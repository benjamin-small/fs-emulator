//! `ExtFs`: a minimal ext2 volume (revision 1, 1 KiB blocks) on an in-memory
//! disk. The superblock and group descriptors are cached and re-read by the
//! corruption gate after a raw write; bitmaps, inodes, directories, and data
//! are read from the disk bytes every time.

use crate::bitmap;
use crate::blockmap;
use crate::dir::{self, DirEntry, FT_DIR};
use crate::group::{self, Geometry, GroupDescriptor, GroupLayout};
use crate::inode::{
    Inode, MODE_DIR, MODE_LOST_FOUND, MODE_TYPE_DIR, MODE_TYPE_FILE, MODE_TYPE_MASK,
};
use crate::superblock::{
    ExtFormatOptions, Superblock, BLOCK_SIZE, EXT2_MAGIC, FIRST_INO, LOST_FOUND_INO, ROOT_INO,
    SUPERBLOCK_OFFSET,
};
use fs_core::path;
use fs_core::{
    Annotation, DateTime, Disk, EntryInfo, Error, FileSystem, OpRecord, Region, RegionKind, Result,
};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

/// Bytes per block, as a `usize` for offsets.
const BS: usize = BLOCK_SIZE as usize;
/// Bits in one bitmap block: the most blocks or inodes a group can track.
const BITS_PER_BITMAP: usize = BS * 8;
/// Data blocks `lost+found` gets at format, as `mke2fs` gives it.
const LOST_FOUND_BLOCKS: u32 = 12;

/// What a block holds, for `block_owners`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockRole {
    Data,
    Directory,
    Indirect,
}

/// The inode (and its path) a block belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockOwner {
    pub inode: u32,
    pub path: String,
    pub role: BlockRole,
}

pub struct ExtFs {
    disk: Disk,
    sb: Superblock,
    gds: Vec<GroupDescriptor>,
    geo: Geometry,
    history: Vec<OpRecord>,
    now: DateTime,
    /// The gate error, or `None` while mounted. `sb`, `gds`, and `geo` keep
    /// their last good values so layout and annotation stay stable.
    corrupt: Option<Error>,
}

/// `now` as the 32-bit seconds ext stores: saturating at both ends.
fn stamp(now: DateTime) -> u32 {
    now.to_unix_seconds().clamp(0, i64::from(u32::MAX)) as u32
}

/// Encode `inode` into its slot in the inode table.
fn put_inode(disk: &mut Disk, geo: &Geometry, ino: u32, inode: &Inode) {
    let (block, offset) = geo.inode_location(ino);
    let mut buf = [0u8; Inode::SIZE];
    inode.encode(&mut buf);
    disk.write(block as usize * BS + offset, &buf);
}

/// The root directory's only block at format: `.`, `..`, and `lost+found`.
fn root_block() -> [u8; BS] {
    let mut block = [0u8; BS];
    let entries = [
        (ROOT_INO, 12u16, &b"."[..]),
        (ROOT_INO, 12, &b".."[..]),
        (LOST_FOUND_INO, (BS - 24) as u16, &b"lost+found"[..]),
    ];
    let mut offset = 0;
    for (inode, rec_len, name) in entries {
        DirEntry {
            inode,
            rec_len,
            name_len: name.len() as u8,
            file_type: FT_DIR,
            name: name.to_vec(),
        }
        .encode(&mut block[offset..]);
        offset += rec_len as usize;
    }
    block
}

/// Decode and check the primary superblock and descriptor table of an
/// image whose physical length is `image.len()`. Shared by `from_image` and
/// the corruption gate.
fn parse_metadata(image: &[u8]) -> Result<(Superblock, Geometry, Vec<GroupDescriptor>)> {
    let sb = Superblock::decode(&image[SUPERBLOCK_OFFSET..SUPERBLOCK_OFFSET + Superblock::LEN]);
    if sb.magic != EXT2_MAGIC {
        return Err(Error::CorruptImage(format!(
            "superblock magic is 0x{:04X}, not 0x{EXT2_MAGIC:04X}",
            sb.magic
        )));
    }
    sb.validate((image.len() / BS) as u64)?;
    let geo = Geometry::from_superblock(&sb)?;
    if sb.inodes_count != geo.inodes_count {
        return Err(Error::CorruptImage(format!(
            "superblock counts {} inodes but {} groups of {} make {}",
            sb.inodes_count, geo.groups, geo.inodes_per_group, geo.inodes_count
        )));
    }
    let table = geo
        .group(0)
        .descriptors_block
        .ok_or_else(|| Error::CorruptImage("group 0 has no group descriptor table".into()))?
        as usize
        * BS;
    let table_len = geo.descriptor_blocks as usize * BS;
    if table + table_len > image.len() {
        return Err(Error::CorruptImage(format!(
            "the group descriptor table runs past the end of the {}-byte image",
            image.len()
        )));
    }
    let gds = group::decode_table(&image[table..table + table_len], geo.groups);
    for (gd, gl) in gds.iter().zip(&geo.groups_layout) {
        let found = (gd.block_bitmap, gd.inode_bitmap, gd.inode_table);
        let expected = (gl.block_bitmap, gl.inode_bitmap, gl.inode_table);
        if found != expected {
            return Err(Error::CorruptImage(format!(
                "group {} descriptor puts its bitmaps and inode table at blocks {}, {}, {} but the layout puts them at {}, {}, {}",
                gl.index, found.0, found.1, found.2, expected.0, expected.1, expected.2
            )));
        }
    }
    Ok((sb, geo, gds))
}

impl ExtFs {
    /// A freshly formatted volume: every superblock copy and descriptor
    /// table, both bitmaps of every group, zeroed inode tables, inodes 1 to
    /// 10 reserved, the root directory, and `lost+found`. Written outside
    /// any operation, so history starts empty.
    pub fn format(options: ExtFormatOptions) -> Result<ExtFs> {
        let geo = Geometry::from_options(&options)?;
        for gl in &geo.groups_layout {
            if gl.first_data > gl.first_block + gl.block_count {
                return Err(Error::InvalidGeometry(format!(
                    "group {} has {} blocks, fewer than its {} metadata blocks",
                    gl.index,
                    gl.block_count,
                    gl.first_data - gl.first_block
                )));
            }
        }
        let g0 = geo.group(0).clone();
        if g0.first_block + g0.block_count - g0.first_data < 1 + LOST_FOUND_BLOCKS {
            return Err(Error::InvalidGeometry(
                "group 0 has no room for the root directory and lost+found".into(),
            ));
        }
        let now = DateTime::default();
        let secs = stamp(now);
        let mut disk = Disk::new(BS, u64::from(geo.total_blocks));

        // Group 0's first data blocks: the root's block, then lost+found's.
        let root_data = g0.first_data;
        let lost_found_data = root_data + 1;
        let mut gds: Vec<GroupDescriptor> = geo
            .groups_layout
            .iter()
            .map(|gl| GroupDescriptor::initial(gl, &geo))
            .collect();
        gds[0].free_blocks_count -= (1 + LOST_FOUND_BLOCKS) as u16;
        gds[0].free_inodes_count -= 1;
        gds[0].used_dirs_count = 2;

        // Bitmaps: metadata blocks and padding set; group 0 also marks the
        // two directories' blocks and inodes 1..=11.
        for gl in &geo.groups_layout {
            let meta = (gl.first_data - gl.first_block) as usize;
            let mut blocks = [0u8; BS];
            bitmap::set_range(&mut blocks, 0, meta);
            bitmap::set_range(&mut blocks, gl.block_count as usize, BITS_PER_BITMAP);
            let mut inodes = [0u8; BS];
            bitmap::set_range(&mut inodes, geo.inodes_per_group as usize, BITS_PER_BITMAP);
            if gl.index == 0 {
                bitmap::set_range(&mut blocks, meta, meta + 1 + LOST_FOUND_BLOCKS as usize);
                bitmap::set_range(&mut inodes, 0, LOST_FOUND_INO as usize);
            }
            disk.write(gl.block_bitmap as usize * BS, &blocks);
            disk.write(gl.inode_bitmap as usize * BS, &inodes);
        }
        // The inode tables are already zero: `Disk::new` is zero-filled.

        let mut root = Inode::new_dir(secs, MODE_DIR);
        root.links_count = 3;
        root.size = BLOCK_SIZE;
        root.blocks = 2;
        root.block[0] = root_data;
        put_inode(&mut disk, &geo, ROOT_INO, &root);
        disk.write(root_data as usize * BS, &root_block());

        let mut lost_found = Inode::new_dir(secs, MODE_LOST_FOUND);
        lost_found.links_count = 2;
        lost_found.size = LOST_FOUND_BLOCKS * BLOCK_SIZE;
        lost_found.blocks = LOST_FOUND_BLOCKS * 2;
        for i in 0..LOST_FOUND_BLOCKS {
            let block = lost_found_data + i;
            lost_found.block[i as usize] = block;
            let bytes = if i == 0 {
                dir::dot_entries_block(LOST_FOUND_INO, ROOT_INO)
            } else {
                dir::empty_block()
            };
            disk.write(block as usize * BS, &bytes);
        }
        put_inode(&mut disk, &geo, LOST_FOUND_INO, &lost_found);

        let mut sb = Superblock::new(&options, &geo, secs);
        sb.free_blocks_count = gds.iter().map(|g| u32::from(g.free_blocks_count)).sum();
        sb.free_inodes_count = gds.iter().map(|g| u32::from(g.free_inodes_count)).sum();
        sb.wtime = secs;
        sb.lastcheck = secs;
        let mut table = vec![0u8; geo.descriptor_blocks as usize * BS];
        group::encode_table(&gds, &mut table);
        for gl in &geo.groups_layout {
            if let (Some(sb_block), Some(table_block)) = (gl.superblock_block, gl.descriptors_block)
            {
                let mut copy = sb.clone();
                copy.block_group_nr = gl.index as u16;
                let mut bytes = [0u8; Superblock::LEN];
                copy.encode(&mut bytes);
                disk.write(sb_block as usize * BS, &bytes);
                disk.write(table_block as usize * BS, &table);
            }
        }
        Ok(ExtFs {
            disk,
            sb,
            gds,
            geo,
            history: Vec::new(),
            now,
            corrupt: None,
        })
    }

    /// Wrap an existing ext2 image. Trailing bytes past `s_blocks_count`
    /// blocks are dropped; a shorter image is `CorruptImage`, a feature this
    /// crate does not implement is `Unsupported`.
    pub fn from_image(mut bytes: Vec<u8>) -> Result<ExtFs> {
        if bytes.len() < SUPERBLOCK_OFFSET + Superblock::LEN {
            return Err(Error::CorruptImage(format!(
                "image is {} bytes, shorter than the {} that hold the boot block and superblock",
                bytes.len(),
                SUPERBLOCK_OFFSET + Superblock::LEN
            )));
        }
        let (sb, geo, gds) = parse_metadata(&bytes)?;
        bytes.truncate(geo.total_blocks as usize * BS);
        let disk = Disk::from_bytes(BS, bytes)?;
        Ok(ExtFs {
            disk,
            sb,
            gds,
            geo,
            history: Vec::new(),
            now: DateTime::default(),
            corrupt: None,
        })
    }

    pub fn superblock(&self) -> &Superblock {
        &self.sb
    }

    pub fn geometry(&self) -> &Geometry {
        &self.geo
    }

    pub fn group_descriptors(&self) -> &[GroupDescriptor] {
        &self.gds
    }

    /// Inode `ino` as the disk holds it now; `NotFound` for 0 or past
    /// `inodes_count`.
    pub fn inode(&self, ino: u32) -> Result<Inode> {
        if ino == 0 || ino > self.geo.inodes_count {
            return Err(Error::NotFound);
        }
        let (block, offset) = self.geo.inode_location(ino);
        Ok(Inode::decode(
            self.disk.read(block as usize * BS + offset, Inode::SIZE),
        ))
    }

    /// `Some(error)` while the primary superblock or descriptor table does
    /// not parse (after a raw write); path operations fail with it until a
    /// later raw write repairs them.
    pub fn corruption(&self) -> Option<&Error> {
        self.corrupt.as_ref()
    }

    pub fn fs_type(&self) -> &'static str {
        "ext2"
    }

    pub fn disk(&self) -> &Disk {
        &self.disk
    }

    pub fn set_now(&mut self, now: DateTime) {
        self.now = now;
    }

    pub fn now(&self) -> DateTime {
        self.now
    }

    pub fn history(&self) -> &[OpRecord] {
        &self.history
    }

    /// Regions per group in disk order, after the boot block (spec section 7).
    pub fn layout(&self) -> Vec<Region> {
        let g = &self.geo;
        let region = |name: String, start: u32, end: u32, kind: RegionKind| Region {
            name,
            sectors: u64::from(start)..u64::from(end),
            kind,
        };
        let mut regions = vec![region("boot block".into(), 0, 1, RegionKind::Boot)];
        for gl in &g.groups_layout {
            let n = gl.index;
            if let Some(block) = gl.superblock_block {
                let name = if n == 0 {
                    "superblock".to_string()
                } else {
                    format!("backup superblock (group {n})")
                };
                regions.push(region(name, block, block + 1, RegionKind::Boot));
            }
            if let Some(block) = gl.descriptors_block {
                regions.push(region(
                    format!("group descriptors (group {n})"),
                    block,
                    block + g.descriptor_blocks,
                    RegionKind::Metadata,
                ));
            }
            regions.push(region(
                format!("block bitmap (group {n})"),
                gl.block_bitmap,
                gl.block_bitmap + 1,
                RegionKind::AllocationTable,
            ));
            regions.push(region(
                format!("inode bitmap (group {n})"),
                gl.inode_bitmap,
                gl.inode_bitmap + 1,
                RegionKind::AllocationTable,
            ));
            regions.push(region(
                format!("inode table (group {n})"),
                gl.inode_table,
                gl.inode_table + g.inode_table_blocks,
                RegionKind::Metadata,
            ));
            regions.push(region(
                format!("data (group {n})"),
                gl.first_data,
                gl.first_block + gl.block_count,
                RegionKind::Data,
            ));
        }
        regions
    }
}

/// Message of the mutation stubs until the write path lands.
const MUTATIONS_PENDING: &str = "ext2 mutations arrive in a later task";

/// ext's 32-bit seconds as a calendar date (UTC).
fn date(secs: u32) -> DateTime {
    DateTime::from_unix_seconds(i64::from(secs))
}

impl ExtFs {
    /// The gate every path-based method passes first.
    fn ensure_mounted(&self) -> Result<()> {
        match &self.corrupt {
            None => Ok(()),
            Some(e) => Err(e.clone()),
        }
    }

    /// Every entry of directory `dir`, in on-disk order, as
    /// `(block, byte offset in the block, entry)`, including unused
    /// (`inode == 0`) entries and `.`/`..`.
    fn scan_dir(&self, dir: &Inode) -> Result<Vec<(u32, usize, DirEntry)>> {
        let mut out = Vec::new();
        for block in blockmap::file_blocks(&self.disk, dir) {
            for (offset, entry) in dir::entries_in_block(self.disk.sector(u64::from(block)))? {
                out.push((block, offset, entry));
            }
        }
        Ok(out)
    }

    /// The inode number `name` names in directory `dir`, by exact byte match.
    fn find_entry(&self, dir: &Inode, name: &str) -> Result<Option<u32>> {
        Ok(self
            .scan_dir(dir)?
            .into_iter()
            .find(|(_, _, e)| e.inode != 0 && e.name == name.as_bytes())
            .map(|(_, _, e)| e.inode))
    }

    /// Walk `parts` from the root: `NotADirectory` when a component before
    /// the last is a file, `NotFound` when one is absent.
    fn walk(&self, parts: &[String]) -> Result<(u32, Inode)> {
        let mut ino = ROOT_INO;
        let mut inode = self.inode(ROOT_INO)?;
        for part in parts {
            if !inode.is_dir() {
                return Err(Error::NotADirectory);
            }
            ino = self.find_entry(&inode, part)?.ok_or(Error::NotFound)?;
            inode = self.inode(ino)?;
        }
        Ok((ino, inode))
    }

    /// The inode a path names, with exact-match components from inode 2.
    fn resolve(&self, path: &str) -> Result<(u32, Inode)> {
        self.walk(&path::parse(path)?)
    }

    /// The directory that would hold `path`'s last component, and that
    /// component. `InvalidPath` for `/`; `NotADirectory` when the parent is
    /// a file. The leaf is not validated here: creators call
    /// `dir::validate_name` on it.
    fn resolve_parent(&self, path: &str) -> Result<(u32, Inode, String)> {
        let (parts, name) = path::split_parent(path)?;
        let (ino, inode) = self.walk(&parts)?;
        if !inode.is_dir() {
            return Err(Error::NotADirectory);
        }
        Ok((ino, inode, name))
    }

    fn entry_info(name: String, inode: &Inode) -> EntryInfo {
        EntryInfo {
            name,
            is_dir: inode.is_dir(),
            size: u64::from(inode.size),
            created: Some(date(inode.ctime)),
            modified: Some(date(inode.mtime)),
            accessed: Some(date(inode.atime)),
        }
    }

    pub fn list_dir(&self, path: &str) -> Result<Vec<EntryInfo>> {
        self.ensure_mounted()?;
        let (_, dir) = self.resolve(path)?;
        if !dir.is_dir() {
            return Err(Error::NotADirectory);
        }
        let mut out = Vec::new();
        for (_, _, entry) in self.scan_dir(&dir)? {
            if entry.inode == 0 || entry.name == b"." || entry.name == b".." {
                continue;
            }
            let inode = self.inode(entry.inode)?;
            out.push(Self::entry_info(
                String::from_utf8_lossy(&entry.name).into_owned(),
                &inode,
            ));
        }
        Ok(out)
    }

    pub fn stat(&self, path: &str) -> Result<EntryInfo> {
        self.ensure_mounted()?;
        let parts = path::parse(path)?;
        let (_, inode) = self.walk(&parts)?;
        let name = parts.last().cloned().unwrap_or_else(|| "/".to_string());
        Ok(Self::entry_info(name, &inode))
    }

    pub fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        self.ensure_mounted()?;
        let (_, inode) = self.resolve(path)?;
        if inode.is_dir() {
            return Err(Error::IsADirectory);
        }
        let size = inode.size as usize;
        let blocks = blockmap::file_blocks(&self.disk, &inode);
        if blocks.len() * BS < size {
            return Err(Error::CorruptImage(format!(
                "inode claims {size} bytes but maps only {} blocks",
                blocks.len()
            )));
        }
        let mut out = Vec::with_capacity(size);
        for block in blocks {
            let take = (size - out.len()).min(BS);
            out.extend_from_slice(self.disk.read(block as usize * BS, take));
        }
        Ok(out)
    }

    /// The stub every mutation returns until the write path lands: the gate
    /// and the path checks run, then `Unsupported`.
    fn mutation_pending(&self, path: &str) -> Result<OpRecord> {
        self.ensure_mounted()?;
        self.resolve_parent(path)?;
        Err(Error::Unsupported(MUTATIONS_PENDING.into()))
    }

    pub fn create_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        self.create_file_op(path, data)
    }

    pub fn write_file(&mut self, path: &str, _data: &[u8]) -> Result<OpRecord> {
        self.mutation_pending(path)
    }

    pub fn delete_file(&mut self, path: &str) -> Result<OpRecord> {
        self.mutation_pending(path)
    }

    pub fn create_dir(&mut self, path: &str) -> Result<OpRecord> {
        self.create_dir_op(path)
    }

    pub fn remove_dir(&mut self, path: &str) -> Result<OpRecord> {
        self.mutation_pending(path)
    }

    /// Which inode and path every directory, data, and indirect block
    /// belongs to, from a walk of the tree from the root. Directories that
    /// do not parse are skipped, so this never fails.
    pub fn block_owners(&self) -> BTreeMap<u32, BlockOwner> {
        let mut owners = BTreeMap::new();
        let mut seen = BTreeSet::from([ROOT_INO]);
        let mut stack = vec![(ROOT_INO, "/".to_string())];
        while let Some((ino, path)) = stack.pop() {
            let Ok(inode) = self.inode(ino) else {
                continue;
            };
            let role = if inode.is_dir() {
                BlockRole::Directory
            } else {
                BlockRole::Data
            };
            let owner = |role| BlockOwner {
                inode: ino,
                path: path.clone(),
                role,
            };
            for block in blockmap::file_blocks(&self.disk, &inode) {
                owners.insert(block, owner(role));
            }
            for (block, _) in blockmap::indirect_blocks(&self.disk, &inode) {
                owners.insert(block, owner(BlockRole::Indirect));
            }
            if !inode.is_dir() {
                continue;
            }
            for (_, _, entry) in self.scan_dir(&inode).unwrap_or_default() {
                if entry.inode == 0 || entry.name == b"." || entry.name == b".." {
                    continue;
                }
                if !seen.insert(entry.inode) {
                    continue;
                }
                let name = String::from_utf8_lossy(&entry.name);
                let child = if path == "/" {
                    format!("/{name}")
                } else {
                    format!("{path}/{name}")
                };
                stack.push((entry.inode, child));
            }
        }
        owners
    }
}

/// Whether the non-empty range `a` shares a byte with `b`.
fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    !a.is_empty() && a.start < b.end && b.start < a.end
}

impl ExtFs {
    /// Run `body` inside a recorded operation: on success the record joins
    /// history; on failure every byte is rolled back and the error returned.
    fn run_op(
        &mut self,
        op: String,
        body: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<OpRecord> {
        self.disk.begin_op(op);
        let result = body(self);
        fs_core::finish_op(&mut self.disk, &mut self.history, result)
    }

    /// The byte ranges of the primary superblock and descriptor table, from
    /// the last good geometry.
    fn primary_metadata(&self) -> [Range<usize>; 2] {
        let table = self.geo.group(0).descriptors_block.unwrap_or(2) as usize * BS;
        [
            SUPERBLOCK_OFFSET..SUPERBLOCK_OFFSET + Superblock::LEN,
            table..table + self.geo.descriptor_blocks as usize * BS,
        ]
    }

    /// Re-read the primary superblock and descriptor table after a raw write
    /// touched them. A parse that succeeds and fits the disk is adopted (the
    /// disk is the truth); otherwise the last good values stay and the
    /// volume is marked corrupt. Never fails: the bytes stay as written.
    fn reparse_metadata(&mut self) {
        match parse_metadata(self.disk.as_bytes()) {
            Ok((sb, geo, gds)) => {
                self.sb = sb;
                self.geo = geo;
                self.gds = gds;
                self.corrupt = None;
            }
            Err(e) => {
                let cause = match e {
                    Error::CorruptImage(cause) => cause,
                    other => other.to_string(),
                };
                self.corrupt = Some(Error::CorruptImage(format!(
                    "superblock or group descriptors no longer parse after a raw write: {cause}"
                )));
            }
        }
    }

    /// Write bytes anywhere on the disk, journaled like every operation. A
    /// write that touches the primary superblock or descriptor table
    /// re-parses them. Works while the volume is corrupt, so a later write
    /// can repair it.
    pub fn write_raw(&mut self, offset: u64, bytes: &[u8]) -> Result<OpRecord> {
        self.run_op(fs_core::raw_write_op(offset, bytes.len()), |fs| {
            let range = fs_core::raw_write(&mut fs.disk, offset, bytes)?;
            if fs.primary_metadata().iter().any(|m| overlaps(&range, m)) {
                fs.reparse_metadata();
            }
            // Nothing fallible may follow: `run_op` rolls the bytes back on
            // an error, but not `sb`/`gds`/`geo`/`corrupt`.
            Ok(())
        })
    }
}

/// One labelled byte range of a block.
fn note(range: Range<usize>, label: impl Into<String>, value: impl Into<String>) -> Annotation {
    Annotation {
        range,
        label: label.into(),
        value: value.into(),
    }
}

/// Little-endian unsigned integer of up to 8 bytes.
fn le(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .rev()
        .fold(0, |acc, &b| (acc << 8) | u64::from(b))
}

/// NUL-trimmed text.
fn text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// The 16 UUID bytes in the usual 8-4-4-4-12 hex form.
fn uuid(bytes: &[u8]) -> String {
    let hex = |r: Range<usize>| {
        bytes[r]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    format!(
        "{}-{}-{}-{}-{}",
        hex(0..4),
        hex(4..6),
        hex(6..8),
        hex(8..10),
        hex(10..16)
    )
}

/// How a superblock field's value is shown.
#[derive(Clone, Copy)]
enum Shown {
    Dec,
    Signed,
    Hex,
    Time,
    Uuid,
    Text,
}

/// Every superblock field of spec section 2: offset, length, label, format.
const SUPERBLOCK_FIELDS: &[(usize, usize, &str, Shown)] = &[
    (0, 4, "inodes count", Shown::Dec),
    (4, 4, "blocks count", Shown::Dec),
    (8, 4, "reserved blocks count", Shown::Dec),
    (12, 4, "free blocks count", Shown::Dec),
    (16, 4, "free inodes count", Shown::Dec),
    (20, 4, "first data block", Shown::Dec),
    (24, 4, "log block size", Shown::Dec),
    (28, 4, "log fragment size", Shown::Dec),
    (32, 4, "blocks per group", Shown::Dec),
    (36, 4, "fragments per group", Shown::Dec),
    (40, 4, "inodes per group", Shown::Dec),
    (44, 4, "last mount time", Shown::Time),
    (48, 4, "last write time", Shown::Time),
    (52, 2, "mount count", Shown::Dec),
    (54, 2, "max mount count", Shown::Signed),
    (56, 2, "magic", Shown::Hex),
    (58, 2, "state", Shown::Dec),
    (60, 2, "errors behaviour", Shown::Dec),
    (62, 2, "minor revision level", Shown::Dec),
    (64, 4, "last check time", Shown::Time),
    (68, 4, "check interval", Shown::Dec),
    (72, 4, "creator OS", Shown::Dec),
    (76, 4, "revision level", Shown::Dec),
    (80, 2, "default reserved uid", Shown::Dec),
    (82, 2, "default reserved gid", Shown::Dec),
    (84, 4, "first inode", Shown::Dec),
    (88, 2, "inode size", Shown::Dec),
    (90, 2, "block group number", Shown::Dec),
    (92, 4, "compatible features", Shown::Hex),
    (96, 4, "incompatible features", Shown::Hex),
    (100, 4, "read-only compatible features", Shown::Hex),
    (104, 16, "UUID", Shown::Uuid),
    (120, 16, "volume name", Shown::Text),
    (136, 64, "last mounted path", Shown::Text),
    (200, 4, "algorithm bitmap", Shown::Dec),
    (204, 1, "preallocate blocks", Shown::Dec),
    (205, 1, "preallocate directory blocks", Shown::Dec),
];

/// One inode slot in words: mode, size, links, blocks, the first direct
/// pointers, and the times as dates.
fn describe_inode(ino: u32, inode: &Inode) -> String {
    if inode.mode == 0 {
        return if ino < FIRST_INO { "reserved" } else { "free" }.to_string();
    }
    let kind = match inode.mode & MODE_TYPE_MASK {
        MODE_TYPE_FILE => "file",
        MODE_TYPE_DIR => "directory",
        _ => "other",
    };
    let direct: Vec<u32> = inode.block[..12]
        .iter()
        .copied()
        .filter(|&b| b != 0)
        .collect();
    let mut pointers = direct
        .iter()
        .take(4)
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    if direct.is_empty() {
        pointers = "none".into();
    } else if direct.len() > 4 {
        pointers.push_str(", …");
    }
    let mut value = format!(
        "mode {:06o} ({kind}), size {}, links {}, blocks {}, direct {pointers}, accessed {}, changed {}, modified {}",
        inode.mode,
        inode.size,
        inode.links_count,
        inode.blocks,
        date(inode.atime),
        date(inode.ctime),
        date(inode.mtime)
    );
    if inode.dtime != 0 {
        value.push_str(&format!(", deleted {}", date(inode.dtime)));
    }
    value
}

impl ExtFs {
    /// Labels for the bytes of one block (the disk's sector is the block):
    /// superblock copies, descriptor tables, bitmaps, and inode tables.
    /// Always describes the live bytes, even while the volume is corrupt.
    pub fn annotate_sector(&self, sector: u64) -> Vec<Annotation> {
        if sector >= self.disk.sector_count() || sector >= u64::from(self.geo.total_blocks) {
            return Vec::new();
        }
        let block = sector as u32;
        if block == 0 {
            return vec![note(0..BS, "boot block", "not used by ext2")];
        }
        let g = &self.geo;
        for gl in &g.groups_layout {
            if gl.superblock_block == Some(block) {
                return self.annotate_superblock(block);
            }
            if let Some(table) = gl.descriptors_block {
                if (table..table + g.descriptor_blocks).contains(&block) {
                    return self.annotate_descriptors(block, block - table);
                }
            }
            if block == gl.block_bitmap {
                return self.annotate_bitmap(block, "block", gl.first_block, gl.block_count);
            }
            if block == gl.inode_bitmap {
                let first = gl.index * g.inodes_per_group + 1;
                return self.annotate_bitmap(block, "inode", first, g.inodes_per_group);
            }
            if (gl.inode_table..gl.inode_table + g.inode_table_blocks).contains(&block) {
                return self.annotate_inode_table(block, gl);
            }
        }
        Vec::new()
    }

    fn block_bytes(&self, block: u32) -> &[u8] {
        self.disk.sector(u64::from(block))
    }

    fn annotate_superblock(&self, block: u32) -> Vec<Annotation> {
        let raw = self.block_bytes(block);
        let mut notes: Vec<Annotation> = SUPERBLOCK_FIELDS
            .iter()
            .map(|&(offset, len, label, shown)| {
                let bytes = &raw[offset..offset + len];
                let value = match shown {
                    Shown::Dec => le(bytes).to_string(),
                    Shown::Signed => (le(bytes) as u16 as i16).to_string(),
                    Shown::Hex => format!("0x{:0width$X}", le(bytes), width = len * 2),
                    Shown::Time => {
                        let secs = le(bytes) as u32;
                        format!("{secs} ({})", date(secs))
                    }
                    Shown::Uuid => uuid(bytes),
                    Shown::Text => text(bytes),
                };
                note(offset..offset + len, label, value)
            })
            .collect();
        notes.push(note(206..BS, "reserved", "unused"));
        notes
    }

    fn annotate_descriptors(&self, block: u32, index_in_table: u32) -> Vec<Annotation> {
        let raw = self.block_bytes(block);
        let per_block = (BS / GroupDescriptor::SIZE) as u32;
        let mut notes = Vec::new();
        for slot in 0..per_block {
            let base = slot as usize * GroupDescriptor::SIZE;
            let g = index_in_table * per_block + slot;
            if g >= self.geo.groups {
                notes.push(note(base..BS, "unused", "past the last group"));
                break;
            }
            let gd = GroupDescriptor::decode(&raw[base..base + GroupDescriptor::SIZE]);
            let fields = [
                (0, 4, "block bitmap", gd.block_bitmap.to_string()),
                (4, 4, "inode bitmap", gd.inode_bitmap.to_string()),
                (8, 4, "inode table", gd.inode_table.to_string()),
                (12, 2, "free blocks", gd.free_blocks_count.to_string()),
                (14, 2, "free inodes", gd.free_inodes_count.to_string()),
                (16, 2, "directories", gd.used_dirs_count.to_string()),
            ];
            for (offset, len, label, value) in fields {
                notes.push(note(
                    base + offset..base + offset + len,
                    format!("group {g} {label}"),
                    value,
                ));
            }
            notes.push(note(
                base + 18..base + GroupDescriptor::SIZE,
                format!("group {g} reserved"),
                "unused",
            ));
        }
        notes
    }

    /// Runs of used and free bits, named by the blocks or inodes they
    /// stand for (`first` is the number bit 0 stands for), then `padding`
    /// for the bits past the group's last one.
    fn annotate_bitmap(&self, block: u32, unit: &str, first: u32, count: u32) -> Vec<Annotation> {
        let raw = self.block_bytes(block);
        let count = (count as usize).min(BITS_PER_BITMAP);
        let mut notes = Vec::new();
        for (used, start, end) in bitmap::runs(raw, count) {
            let (a, b) = (first + start as u32, first + end as u32 - 1);
            let label = if a == b {
                format!("{unit} {a}")
            } else {
                format!("{unit}s {a}..{b}")
            };
            let value = if used { "used" } else { "free" };
            notes.push(note(start / 8..end.div_ceil(8), label, value));
        }
        if count < BITS_PER_BITMAP {
            let all_set = (count..BITS_PER_BITMAP).all(|bit| bitmap::is_set(raw, bit));
            let value = if all_set {
                "set: bits past the end of the group"
            } else {
                "not all set: bits past the end of the group"
            };
            notes.push(note(count / 8..BS, "padding", value));
        }
        notes
    }

    fn annotate_inode_table(&self, block: u32, gl: &GroupLayout) -> Vec<Annotation> {
        let raw = self.block_bytes(block);
        let ipg = self.geo.inodes_per_group;
        let per_block = (BS / Inode::SIZE) as u32;
        let first_index = (block - gl.inode_table) * per_block;
        let mut notes = Vec::new();
        for slot in 0..per_block {
            let index = first_index + slot;
            if index >= ipg {
                break;
            }
            let ino = gl.index * ipg + index + 1;
            let range = slot as usize * Inode::SIZE..(slot as usize + 1) * Inode::SIZE;
            let inode = Inode::decode(&raw[range.clone()]);
            notes.push(note(
                range,
                format!("inode {ino}"),
                describe_inode(ino, &inode),
            ));
        }
        notes
    }
}

impl FileSystem for ExtFs {
    fn fs_type(&self) -> &'static str {
        ExtFs::fs_type(self)
    }
    fn create_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        ExtFs::create_file(self, path, data)
    }
    fn write_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        ExtFs::write_file(self, path, data)
    }
    fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        ExtFs::read_file(self, path)
    }
    fn delete_file(&mut self, path: &str) -> Result<OpRecord> {
        ExtFs::delete_file(self, path)
    }
    fn create_dir(&mut self, path: &str) -> Result<OpRecord> {
        ExtFs::create_dir(self, path)
    }
    fn remove_dir(&mut self, path: &str) -> Result<OpRecord> {
        ExtFs::remove_dir(self, path)
    }
    fn list_dir(&self, path: &str) -> Result<Vec<EntryInfo>> {
        ExtFs::list_dir(self, path)
    }
    fn stat(&self, path: &str) -> Result<EntryInfo> {
        ExtFs::stat(self, path)
    }
    fn set_now(&mut self, now: DateTime) {
        ExtFs::set_now(self, now)
    }
    fn write_raw(&mut self, offset: u64, bytes: &[u8]) -> Result<OpRecord> {
        ExtFs::write_raw(self, offset, bytes)
    }
    fn corruption(&self) -> Option<&Error> {
        ExtFs::corruption(self)
    }
    fn disk(&self) -> &Disk {
        ExtFs::disk(self)
    }
    fn layout(&self) -> Vec<Region> {
        ExtFs::layout(self)
    }
    fn annotate_sector(&self, sector: u64) -> Vec<Annotation> {
        ExtFs::annotate_sector(self, sector)
    }
    fn history(&self) -> &[OpRecord] {
        ExtFs::history(self)
    }
}

/// Allocation-backed operations (spec sections 4 and 5) and the helpers the
/// other mutations share. A child module, so its imports are its own and
/// `ExtFs`'s private fields and methods stay reachable.
mod create {
    use super::{stamp, ExtFs};
    use crate::alloc::{self, AllocCtx};
    use crate::blockmap;
    use crate::dir::{self, DirEntry, FT_DIR, FT_FILE};
    use crate::events::ExtEvent;
    use crate::inode::{Inode, MODE_DIR};
    use crate::superblock::BLOCK_SIZE;
    use fs_core::{path, Error, OpRecord, Result};

    const BS: usize = BLOCK_SIZE as usize;

    /// `/` for the root, otherwise `/a/b`.
    pub(super) fn display_path(parts: &[String]) -> String {
        if parts.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", parts.join("/"))
        }
    }

    /// What `check_new_entry` hands back: where the new entry goes.
    struct NewEntry {
        parent_parts: Vec<String>,
        name: String,
        parent_ino: u32,
        parent: Inode,
    }

    impl ExtFs {
        /// Borrow what an allocation touches.
        pub(super) fn ctx(&mut self) -> AllocCtx<'_> {
            AllocCtx {
                disk: &mut self.disk,
                sb: &mut self.sb,
                gds: &mut self.gds,
                geo: &self.geo,
            }
        }

        /// `now` as ext's 32-bit seconds, saturating at both ends (Task 3's
        /// `stamp`).
        pub(super) fn now_stamp(&self) -> u32 {
            stamp(self.now)
        }

        /// `run_op` for operations that change the cached superblock or
        /// descriptors: `run_op` rolls the disk back on error, and this
        /// restores `sb` and `gds` from before the operation, so the cache
        /// matches the disk again. Every mutation runs through this, never
        /// through bare `run_op`.
        pub(super) fn run_mutation(
            &mut self,
            op: String,
            body: impl FnOnce(&mut Self) -> Result<()>,
        ) -> Result<OpRecord> {
            let saved_sb = self.sb.clone();
            let saved_gds = self.gds.clone();
            let result = self.run_op(op, body);
            if result.is_err() {
                self.sb = saved_sb;
                self.gds = saved_gds;
            }
            result
        }

        /// Encode `inode` into its 128-byte slot and report `InodeWritten`.
        pub(super) fn write_inode(&mut self, ino: u32, inode: &Inode, path: &str) {
            let (block, offset) = self.geo.inode_location(ino);
            let start = block as usize * BS + offset;
            let mut bytes = [0u8; Inode::SIZE];
            inode.encode(&mut bytes);
            self.disk.write(start, &bytes);
            self.disk.event(Box::new(ExtEvent::InodeWritten {
                inode: ino,
                path: path.to_string(),
                range: start..start + Inode::SIZE,
            }));
        }

        /// `blockmap::map_blocks` with the inode's own group as the goal,
        /// then one `IndirectWritten` per pointer block the inode holds (each
        /// was written in full by the mapping). Set `inode.size` first.
        pub(super) fn map_file_blocks(
            &mut self,
            ino: u32,
            inode: &mut Inode,
            data: &[u32],
        ) -> Result<()> {
            let goal = self.geo.group_of_inode(ino);
            blockmap::map_blocks(&mut self.ctx(), inode, data, goal)?;
            for (block, level) in blockmap::indirect_blocks(&self.disk, inode) {
                let start = block as usize * BS;
                self.disk.event(Box::new(ExtEvent::IndirectWritten {
                    inode: ino,
                    level,
                    block,
                    range: start..start + BS,
                }));
            }
            Ok(())
        }

        /// Write `data` into `blocks` (logical order, exactly
        /// `blocks_needed(data.len())` of them), zero-filling the last
        /// block's tail; one `DataWritten` per contiguous run.
        pub(super) fn write_data(&mut self, ino: u32, blocks: &[u32], data: &[u8]) {
            for (index, run) in alloc::contiguous_runs(blocks) {
                let block = blocks[index];
                let start = block as usize * BS;
                let span = run * BS;
                let from = index * BS;
                let chunk = &data[from..(from + span).min(data.len())];
                self.disk.write(start, chunk);
                if chunk.len() < span {
                    self.disk.fill(start + chunk.len(), span - chunk.len(), 0);
                }
                self.disk.event(Box::new(ExtEvent::DataWritten {
                    inode: ino,
                    bytes: chunk.len(),
                    block,
                    range: start..start + span,
                }));
            }
        }

        /// Insert `name` -> `ino` into directory `dir_ino` (spec section 4,
        /// Directories): split the first entry whose slack fits the new
        /// entry; when no block has room, allocate one goal-based (the
        /// directory's group) holding the entry with `rec_len` spanning it.
        /// Updates `dir_inode` in memory (`mtime`, `ctime`; `size`, `blocks`
        /// and pointers when it grows); the caller writes it with
        /// `write_inode`, so `create_dir` writes the parent once.
        pub(super) fn dir_insert(
            &mut self,
            dir_ino: u32,
            dir_inode: &mut Inode,
            name: &str,
            ino: u32,
            file_type: u8,
        ) -> Result<()> {
            let now = self.now_stamp();
            let bytes = name.as_bytes();
            let mut entry = DirEntry {
                inode: ino,
                rec_len: 0,
                name_len: bytes.len() as u8,
                file_type,
                name: bytes.to_vec(),
            };
            let need = usize::from(dir::record_len(bytes.len()));
            let blocks = blockmap::file_blocks(&self.disk, dir_inode);
            let mut slot = None;
            'scan: for &block in &blocks {
                let base = block as usize * BS;
                let entries = dir::entries_in_block(self.disk.read(base, BS))?;
                for (offset, existing) in entries {
                    let used = if existing.inode == 0 {
                        0
                    } else {
                        usize::from(existing.actual_len())
                    };
                    let rec_len = usize::from(existing.rec_len);
                    if rec_len.saturating_sub(used) >= need {
                        if used > 0 {
                            // Shrink the entry to its own length; the rest becomes ours.
                            self.disk
                                .write(base + offset + 4, &(used as u16).to_le_bytes());
                        }
                        slot = Some((block, offset + used, (rec_len - used) as u16));
                        break 'scan;
                    }
                }
            }
            let (block, offset, fresh) = match slot {
                Some((block, offset, rec_len)) => {
                    entry.rec_len = rec_len;
                    (block, offset, false)
                }
                None => {
                    let goal = self.geo.group_of_inode(dir_ino);
                    let new_block = alloc::alloc_blocks(&mut self.ctx(), goal, 1)?[0];
                    let mut all = blocks;
                    all.push(new_block);
                    dir_inode.size += BLOCK_SIZE;
                    self.map_file_blocks(dir_ino, dir_inode, &all)?;
                    entry.rec_len = BLOCK_SIZE as u16;
                    (new_block, 0, true)
                }
            };
            let mut scratch = [0u8; BS];
            entry.encode(&mut scratch);
            let len = usize::from(entry.actual_len());
            let base = block as usize * BS;
            if fresh {
                self.disk.write(base, &scratch);
            } else {
                self.disk.write(base + offset, &scratch[..len]);
            }
            let start = base + offset;
            self.disk.event(Box::new(ExtEvent::DirEntryWritten {
                dir_inode: dir_ino,
                name: name.to_string(),
                block,
                range: start..start + len,
            }));
            dir_inode.mtime = now;
            dir_inode.ctime = now;
            Ok(())
        }

        /// The checks both creates share, in the spec's order: the
        /// corruption gate, `InvalidPath`, `InvalidName`, `NotFound` or
        /// `NotADirectory` on the parent, `AlreadyExists`.
        fn check_new_entry(&self, path: &str) -> Result<NewEntry> {
            self.ensure_mounted()?;
            let (parent_parts, name) = path::split_parent(path)?;
            dir::validate_name(&name)?;
            let (parent_ino, parent, _) = self.resolve_parent(path)?;
            if !parent.is_dir() {
                return Err(Error::NotADirectory);
            }
            match self.resolve(path) {
                Ok(_) => Err(Error::AlreadyExists),
                Err(Error::NotFound) => Ok(NewEntry {
                    parent_parts,
                    name,
                    parent_ino,
                    parent,
                }),
                Err(e) => Err(e),
            }
        }

        /// `create_file` (spec section 4): then `FileTooLarge`, then
        /// `DiskFull` from the counts before anything is written. A parent
        /// that needs a new block for the entry is not counted here; that
        /// `DiskFull` comes from `dir_insert` inside the operation and is
        /// rolled back (spec section 5).
        pub(super) fn create_file_op(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
            let NewEntry {
                parent_parts,
                name,
                parent_ino,
                mut parent,
            } = self.check_new_entry(path)?;
            let max = u64::from(blockmap::MAX_FILE_BLOCKS) * u64::from(BLOCK_SIZE);
            if data.len() as u64 > max {
                return Err(Error::FileTooLarge);
            }
            let data_blocks = blockmap::blocks_needed(data.len() as u64);
            let needed = data_blocks + blockmap::indirect_blocks_needed(data_blocks);
            if self.sb.free_inodes_count == 0 || self.sb.free_blocks_count < needed {
                return Err(Error::DiskFull);
            }
            let parent_path = display_path(&parent_parts);
            let own_path = display_path(&[parent_parts, vec![name.clone()]].concat());
            let now = self.now_stamp();
            let goal = self.geo.group_of_inode(parent_ino);
            self.run_mutation(format!("create_file {path}"), |fs| {
                fs.sb.wtime = now;
                let ino = alloc::alloc_inode(&mut fs.ctx(), goal, false)?;
                let group = fs.geo.group_of_inode(ino);
                let blocks = alloc::alloc_blocks(&mut fs.ctx(), group, data_blocks)?;
                let mut inode = Inode::new_file(now);
                inode.size = data.len() as u32;
                fs.map_file_blocks(ino, &mut inode, &blocks)?;
                fs.write_data(ino, &blocks, data);
                fs.write_inode(ino, &inode, &own_path);
                fs.dir_insert(parent_ino, &mut parent, &name, ino, FT_FILE)?;
                fs.write_inode(parent_ino, &parent, &parent_path);
                Ok(())
            })
        }

        /// `create_dir` (spec section 4): one block holding `.` and `..`,
        /// links 2, the parent's links and the group's used directories up
        /// by one. `DiskFull` from the counts (one inode, one block) before
        /// anything is written; a parent that needs a new block fails
        /// inside the operation and is rolled back, as in `create_file_op`.
        pub(super) fn create_dir_op(&mut self, path: &str) -> Result<OpRecord> {
            let NewEntry {
                parent_parts,
                name,
                parent_ino,
                mut parent,
            } = self.check_new_entry(path)?;
            if self.sb.free_inodes_count == 0 || self.sb.free_blocks_count == 0 {
                return Err(Error::DiskFull);
            }
            let parent_path = display_path(&parent_parts);
            let own_path = display_path(&[parent_parts, vec![name.clone()]].concat());
            let now = self.now_stamp();
            let goal = self.geo.group_of_inode(parent_ino);
            self.run_mutation(format!("create_dir {path}"), |fs| {
                fs.sb.wtime = now;
                let ino = alloc::alloc_inode(&mut fs.ctx(), goal, true)?;
                let group = fs.geo.group_of_inode(ino);
                let block = alloc::alloc_blocks(&mut fs.ctx(), group, 1)?[0];
                let mut inode = Inode::new_dir(now, MODE_DIR);
                inode.links_count = 2;
                inode.size = BLOCK_SIZE;
                fs.map_file_blocks(ino, &mut inode, &[block])?;
                let start = block as usize * BS;
                fs.disk
                    .write(start, &dir::dot_entries_block(ino, parent_ino));
                for (dot, offset) in [(".", 0), ("..", 12)] {
                    fs.disk.event(Box::new(ExtEvent::DirEntryWritten {
                        dir_inode: ino,
                        name: dot.to_string(),
                        block,
                        range: start + offset..start + offset + 12,
                    }));
                }
                fs.write_inode(ino, &inode, &own_path);
                parent.links_count = parent.links_count.saturating_add(1);
                fs.dir_insert(parent_ino, &mut parent, &name, ino, FT_DIR)?;
                fs.write_inode(parent_ino, &parent, &parent_path);
                Ok(())
            })
        }
    }
}
