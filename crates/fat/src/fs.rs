//! `FatFs`: a FAT16 volume on an in-memory disk. The disk bytes are the only
//! state; every method re-reads what it needs from them.

use crate::boot_sector::{BootSector, FormatOptions, Geometry};
use crate::dir::{self, DirLocation};
use crate::dir_entry::{self, attr, LfnEntry, ShortEntry, DELETED, ENTRY_SIZE, FREE};
use crate::events::{EntryKind, FatEvent};
use crate::name;
use crate::table;
use crate::table::FatEntry;
use fs_core::path;
use fs_core::{
    Annotation, DateTime, Disk, EntryInfo, Error, FileSystem, OpRecord, Region, RegionKind, Result,
};
use std::collections::{BTreeMap, HashSet};
use std::ops::Range;

pub struct FatFs {
    disk: Disk,
    boot: BootSector,
    geo: Geometry,
    now: DateTime,
    history: Vec<OpRecord>,
}

impl FatFs {
    /// A freshly formatted, empty volume.
    pub fn format(opts: FormatOptions) -> Result<FatFs> {
        let boot = BootSector::from_options(&opts)?;
        let geo = boot.geometry()?;
        let mut disk = Disk::new(geo.bytes_per_sector, geo.total_sectors);
        disk.write(0, &boot.to_bytes());
        for fat in 0..geo.fat_count {
            // Entry 0 holds the media descriptor (0xFFF8); entry 1 is reserved (0xFFFF).
            disk.write(geo.fat_offset(fat), &[0xF8, 0xFF, 0xFF, 0xFF]);
        }
        Ok(FatFs {
            disk,
            boot,
            geo,
            now: DateTime::default(),
            history: Vec::new(),
        })
    }

    /// Wrap an existing FAT16 image. Extra trailing bytes are dropped; a
    /// shorter image than the boot sector describes is rejected.
    pub fn from_image(mut bytes: Vec<u8>) -> Result<FatFs> {
        if bytes.len() < 512 {
            return Err(Error::CorruptImage(
                "image is shorter than one sector".into(),
            ));
        }
        let boot = BootSector::parse(&bytes[..512])?;
        let geo = boot.geometry()?;
        let expected = geo.total_sectors as usize * geo.bytes_per_sector;
        if bytes.len() < expected {
            return Err(Error::CorruptImage(format!(
                "image is {} bytes but the boot sector describes {expected}",
                bytes.len()
            )));
        }
        bytes.truncate(expected);
        let disk = Disk::from_bytes(geo.bytes_per_sector, bytes)?;
        Ok(FatFs {
            disk,
            boot,
            geo,
            now: DateTime::default(),
            history: Vec::new(),
        })
    }

    pub fn boot_sector(&self) -> &BootSector {
        &self.boot
    }

    pub fn geometry(&self) -> &Geometry {
        &self.geo
    }

    pub fn fs_type(&self) -> &'static str {
        "FAT16"
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

    pub fn layout(&self) -> Vec<Region> {
        let g = &self.geo;
        let mut regions = vec![Region {
            name: "reserved (boot sector)".into(),
            sectors: 0..g.reserved_sectors,
            kind: RegionKind::Boot,
        }];
        for fat in 0..g.fat_count {
            let start = g.reserved_sectors + fat as u64 * g.sectors_per_fat;
            regions.push(Region {
                name: format!("FAT {fat}"),
                sectors: start..start + g.sectors_per_fat,
                kind: RegionKind::AllocationTable,
            });
        }
        regions.push(Region {
            name: "root directory".into(),
            sectors: g.first_root_dir_sector..g.first_data_sector,
            kind: RegionKind::Directory,
        });
        regions.push(Region {
            name: "data".into(),
            sectors: g.first_data_sector..g.total_sectors,
            kind: RegionKind::Data,
        });
        regions
    }

    /// Run `body` inside a recorded operation. On success the record is
    /// appended to history and returned. On failure the operation is rolled
    /// back byte-for-byte (every recorded change is restored in reverse
    /// order) and leaves no history entry; the error is then returned.
    fn run_op(
        &mut self,
        op: String,
        body: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<OpRecord> {
        self.disk.begin_op(op);
        let result = body(self);
        let record = self.disk.end_op();
        if let Err(e) = result {
            // The op is closed, so these restores are not re-journaled.
            for change in record.changes.iter().rev() {
                self.disk.write(change.offset, &change.before);
            }
            return Err(e);
        }
        self.history.push(record.clone());
        Ok(record)
    }

    /// `/` for the root, otherwise `/A/B` from components.
    fn dir_display_name(parts: &[String]) -> String {
        if parts.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", parts.join("/"))
        }
    }

    /// All in-use entries of a directory in slot order, with long names
    /// attached. Includes `.` and `..`; excludes the volume label.
    fn scan_dir(&self, loc: DirLocation) -> Result<Vec<Located>> {
        let mut out = Vec::new();
        let mut pending: Vec<(usize, LfnEntry)> = Vec::new();
        for slot in dir::slots(&self.disk, &self.geo, loc)? {
            let bytes = self.disk.read(slot.offset, ENTRY_SIZE);
            match bytes[0] {
                FREE => break,
                DELETED => {
                    pending.clear();
                    continue;
                }
                _ => {}
            }
            if attr::is_lfn(bytes[11]) {
                let lfn = LfnEntry::parse(bytes);
                if lfn.is_last() {
                    pending.clear();
                    pending.push((slot.index, lfn));
                } else if let Some((_, prev)) = pending.last() {
                    if prev.order() == lfn.order() + 1 && prev.checksum == lfn.checksum {
                        pending.push((slot.index, lfn));
                    } else {
                        pending.clear();
                    }
                }
                continue;
            }
            let entry = ShortEntry::parse(bytes);
            if entry.is_volume_label() {
                pending.clear();
                continue;
            }
            let long_name = assemble_lfn(&pending, &entry.name);
            let lfn_slots = if long_name.is_some() {
                pending.iter().map(|(i, _)| *i).collect()
            } else {
                Vec::new()
            };
            pending.clear();
            out.push(Located {
                dir: loc,
                slot: slot.index,
                lfn_slots,
                entry,
                long_name,
            });
        }
        Ok(out)
    }

    /// Case-insensitive lookup against both the long and the short name.
    fn find_in_dir(&self, loc: DirLocation, name: &str) -> Result<Option<Located>> {
        Ok(self.scan_dir(loc)?.into_iter().find(|l| {
            !l.entry.is_dot_entry()
                && (names_match(&l.entry.display_name(), name)
                    || l.long_name
                        .as_deref()
                        .is_some_and(|ln| names_match(ln, name)))
        }))
    }

    /// Walk directory components from the root.
    fn resolve_dir(&self, parts: &[String]) -> Result<DirLocation> {
        let mut loc = DirLocation::Root;
        for part in parts {
            let found = self.find_in_dir(loc, part)?.ok_or(Error::NotFound)?;
            if !found.entry.is_dir() {
                return Err(Error::NotADirectory);
            }
            loc = DirLocation::from_cluster(found.entry.first_cluster());
        }
        Ok(loc)
    }

    /// `Ok(None)` for the root, `Err(NotFound)` if the path is absent.
    fn resolve(&self, path: &str) -> Result<Option<Located>> {
        let parts = path::parse(path)?;
        let Some((name, parent)) = parts.split_last() else {
            return Ok(None);
        };
        let loc = self.resolve_dir(parent)?;
        self.find_in_dir(loc, name)?
            .ok_or(Error::NotFound)
            .map(Some)
    }

    fn read_chain_data(&self, first: u32, size: usize) -> Result<Vec<u8>> {
        // `size` comes straight off disk; a corrupt entry claiming e.g. 4 GiB
        // must not be used to pre-allocate that much memory.
        let mut out = Vec::with_capacity(size.min(self.disk.len()));
        if size == 0 {
            return Ok(out);
        }
        for cluster in table::chain(&self.disk, &self.geo, first)? {
            if out.len() >= size {
                break;
            }
            let take = (size - out.len()).min(self.geo.cluster_size());
            out.extend_from_slice(self.disk.read(self.geo.cluster_offset(cluster), take));
        }
        if out.len() < size {
            return Err(Error::CorruptImage(format!(
                "file claims {size} bytes but its cluster chain holds only {}",
                out.len()
            )));
        }
        Ok(out)
    }

    pub fn list_dir(&self, path: &str) -> Result<Vec<EntryInfo>> {
        let parts = path::parse(path)?;
        let loc = self.resolve_dir(&parts)?;
        Ok(self
            .scan_dir(loc)?
            .into_iter()
            .filter(|l| !l.entry.is_dot_entry())
            .map(|l| l.info())
            .collect())
    }

    pub fn stat(&self, path: &str) -> Result<EntryInfo> {
        match self.resolve(path)? {
            Some(located) => Ok(located.info()),
            None => Ok(EntryInfo {
                name: "/".into(),
                is_dir: true,
                size: 0,
                created: None,
                modified: None,
                accessed: None,
            }),
        }
    }

    pub fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        let located = self.resolve(path)?.ok_or(Error::IsADirectory)?;
        if located.entry.is_dir() {
            return Err(Error::IsADirectory);
        }
        self.read_chain_data(located.entry.first_cluster(), located.entry.size as usize)
    }

    pub fn create_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        let (parent_parts, name) = path::split_parent(path)?;
        name::validate_long_name(&name)?;
        let parent = self.resolve_dir(&parent_parts)?;
        if self.find_in_dir(parent, &name)?.is_some() {
            return Err(Error::AlreadyExists);
        }
        let size = u32::try_from(data.len()).map_err(|_| Error::FileTooLarge)?;
        let parent_name = Self::dir_display_name(&parent_parts);
        self.run_op(format!("create_file {path}"), |fs| {
            let clusters = fs.write_data(data)?;
            let first = clusters.first().copied().unwrap_or(0);
            fs.write_new_entry(parent, &parent_name, &name, attr::ARCHIVE, first, size)?;
            Ok(())
        })
    }

    pub fn create_dir(&mut self, path: &str) -> Result<OpRecord> {
        let (parent_parts, name) = path::split_parent(path)?;
        name::validate_long_name(&name)?;
        let parent = self.resolve_dir(&parent_parts)?;
        if self.find_in_dir(parent, &name)?.is_some() {
            return Err(Error::AlreadyExists);
        }
        let parent_name = Self::dir_display_name(&parent_parts);
        let own_name = Self::dir_display_name(&[parent_parts.clone(), vec![name.clone()]].concat());
        self.run_op(format!("create_dir {path}"), |fs| {
            let cluster = table::allocate_chain(&mut fs.disk, &fs.geo, 1)?[0];
            let base = fs.geo.cluster_offset(cluster);
            let cs = fs.geo.cluster_size();
            fs.disk.fill(base, cs, 0);
            let parent_cluster = match parent {
                DirLocation::Root => 0,
                DirLocation::Cluster(c) => c,
            };
            let mut dot = ShortEntry::new(*b".          ", attr::DIRECTORY, &fs.now);
            dot.set_first_cluster(cluster);
            let mut dotdot = ShortEntry::new(*b"..         ", attr::DIRECTORY, &fs.now);
            dotdot.set_first_cluster(parent_cluster);
            for (slot, entry) in [(0, dot), (1, dotdot)] {
                let off = base + slot * ENTRY_SIZE;
                fs.disk.write(off, &entry.to_bytes());
                fs.disk.event(Box::new(FatEvent::DirEntryWritten {
                    dir: own_name.clone(),
                    slot,
                    kind: EntryKind::Short,
                    range: off..off + ENTRY_SIZE,
                }));
            }
            fs.write_new_entry(parent, &parent_name, &name, attr::DIRECTORY, cluster, 0)?;
            Ok(())
        })
    }

    pub fn delete_file(&mut self, path: &str) -> Result<OpRecord> {
        let located = self.resolve(path)?.ok_or(Error::InvalidPath)?;
        if located.entry.is_dir() {
            return Err(Error::IsADirectory);
        }
        let dir_name = Self::parent_display_name(path)?;
        self.run_op(format!("delete_file {path}"), |fs| {
            fs.remove_entry(&located, &dir_name)
        })
    }

    pub fn remove_dir(&mut self, path: &str) -> Result<OpRecord> {
        let located = self.resolve(path)?.ok_or(Error::InvalidPath)?;
        if !located.entry.is_dir() {
            return Err(Error::NotADirectory);
        }
        let contents = self.scan_dir(DirLocation::Cluster(located.entry.first_cluster()))?;
        if contents.iter().any(|l| !l.entry.is_dot_entry()) {
            return Err(Error::DirectoryNotEmpty);
        }
        let dir_name = Self::parent_display_name(path)?;
        self.run_op(format!("remove_dir {path}"), |fs| {
            fs.remove_entry(&located, &dir_name)
        })
    }

    pub fn write_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        let located = self.resolve(path)?.ok_or(Error::InvalidPath)?;
        if located.entry.is_dir() {
            return Err(Error::IsADirectory);
        }
        let size = u32::try_from(data.len()).map_err(|_| Error::FileTooLarge)?;
        let needed = data.len().div_ceil(self.geo.cluster_size()) as u32;
        let old_first = located.entry.first_cluster();
        let old_len = if old_first == 0 {
            0
        } else {
            table::chain(&self.disk, &self.geo, old_first)?.len() as u32
        };
        if table::count_free(&self.disk, &self.geo) + old_len < needed {
            return Err(Error::DiskFull);
        }
        let dir_name = Self::parent_display_name(path)?;
        self.run_op(format!("write_file {path}"), |fs| {
            if old_first != 0 {
                table::free_chain(&mut fs.disk, &fs.geo, old_first)?;
            }
            let clusters = fs.write_data(data)?;
            let mut entry = located.entry.clone();
            entry.set_first_cluster(clusters.first().copied().unwrap_or(0));
            entry.size = size;
            entry.write_time = dir_entry::pack_time(&fs.now);
            entry.write_date = dir_entry::pack_date(&fs.now);
            entry.access_date = dir_entry::pack_date(&fs.now);
            let slots = dir::slots(&fs.disk, &fs.geo, located.dir)?;
            let slot = slots[located.slot];
            fs.disk.write(slot.offset, &entry.to_bytes());
            fs.disk.event(Box::new(FatEvent::DirEntryWritten {
                dir: dir_name.clone(),
                slot: slot.index,
                kind: EntryKind::Short,
                range: slot.offset..slot.offset + ENTRY_SIZE,
            }));
            Ok(())
        })
    }

    /// Mark the entry and its LFN entries deleted and free its chain. Data
    /// bytes are left in place, exactly as a real driver leaves them.
    fn remove_entry(&mut self, located: &Located, dir_name: &str) -> Result<()> {
        let slots = dir::slots(&self.disk, &self.geo, located.dir)?;
        for &i in located
            .lfn_slots
            .iter()
            .chain(std::iter::once(&located.slot))
        {
            let off = slots[i].offset;
            self.disk.write(off, &[DELETED]);
            self.disk.event(Box::new(FatEvent::DirEntryDeleted {
                dir: dir_name.to_string(),
                slot: i,
                range: off..off + ENTRY_SIZE,
            }));
        }
        let first = located.entry.first_cluster();
        if first != 0 {
            table::free_chain(&mut self.disk, &self.geo, first)?;
        }
        Ok(())
    }

    fn parent_display_name(path: &str) -> Result<String> {
        let (parts, _) = path::split_parent(path)?;
        Ok(Self::dir_display_name(&parts))
    }

    /// Allocate a chain for `data` and write it cluster by cluster. The tail
    /// of the last cluster is zeroed so cluster contents are deterministic.
    fn write_data(&mut self, data: &[u8]) -> Result<Vec<u32>> {
        let cs = self.geo.cluster_size();
        let needed = data.len().div_ceil(cs) as u32;
        let clusters = table::allocate_chain(&mut self.disk, &self.geo, needed)?;
        for (i, &cluster) in clusters.iter().enumerate() {
            let chunk = &data[i * cs..((i + 1) * cs).min(data.len())];
            let off = self.geo.cluster_offset(cluster);
            self.disk.write(off, chunk);
            if chunk.len() < cs {
                self.disk.fill(off + chunk.len(), cs - chunk.len(), 0);
            }
            self.disk.event(Box::new(FatEvent::DataWritten {
                cluster,
                bytes: chunk.len(),
                range: off..off + cs,
            }));
        }
        Ok(clusters)
    }

    /// Write the LFN entries (if the name needs them) and the short entry
    /// into the first free run of slots, growing the directory as needed.
    fn write_new_entry(
        &mut self,
        parent: DirLocation,
        parent_name: &str,
        name: &str,
        attributes: u8,
        first_cluster: u32,
        size: u32,
    ) -> Result<()> {
        let (short, chunks) = match name::to_short_name_bytes(name) {
            Some(short) => (short, Vec::new()),
            None => {
                let taken: HashSet<[u8; 11]> = self
                    .scan_dir(parent)?
                    .iter()
                    .map(|l| l.entry.name)
                    .collect();
                let short = name::generate_short_name(name, &|c| taken.contains(c))?;
                (short, name::lfn_chunks(name))
            }
        };
        let needed = chunks.len() + 1;
        let start = loop {
            if let Some(i) = dir::find_free_run(&self.disk, &self.geo, parent, needed)? {
                break i;
            }
            dir::grow(&mut self.disk, &self.geo, parent, parent_name)?;
        };
        let slots = dir::slots(&self.disk, &self.geo, parent)?;
        let checksum = dir_entry::lfn_checksum(&short);
        let n = chunks.len();
        for (i, chunk) in chunks.iter().enumerate().rev() {
            let entry = LfnEntry::new((i + 1) as u8, i == n - 1, checksum, chunk);
            let slot = slots[start + (n - 1 - i)];
            self.disk.write(slot.offset, &entry.to_bytes());
            self.disk.event(Box::new(FatEvent::DirEntryWritten {
                dir: parent_name.to_string(),
                slot: slot.index,
                kind: EntryKind::Lfn,
                range: slot.offset..slot.offset + ENTRY_SIZE,
            }));
        }
        let mut entry = ShortEntry::new(short, attributes, &self.now);
        entry.set_first_cluster(first_cluster);
        entry.size = size;
        let slot = slots[start + n];
        self.disk.write(slot.offset, &entry.to_bytes());
        self.disk.event(Box::new(FatEvent::DirEntryWritten {
            dir: parent_name.to_string(),
            slot: slot.index,
            kind: EntryKind::Short,
            range: slot.offset..slot.offset + ENTRY_SIZE,
        }));
        Ok(())
    }

    /// Every entry of one FAT copy, indexed by cluster number. Returns an
    /// empty vector for a FAT index that does not exist, rather than
    /// running off the buffer.
    pub fn fat_entries(&self, fat: u8) -> Vec<FatEntry> {
        if fat >= self.geo.fat_count {
            return Vec::new();
        }
        (0..self.geo.cluster_count + 2)
            .map(|c| {
                if c < 2 {
                    FatEntry::Reserved
                } else {
                    table::read_entry_from(&self.disk, &self.geo, fat, c)
                }
            })
            .collect()
    }

    pub fn cluster_chain(&self, start: u32) -> Result<Vec<u32>> {
        table::chain(&self.disk, &self.geo, start)
    }

    /// Every slot of a directory, including free, deleted and LFN slots.
    pub fn raw_dir_entries(&self, path: &str) -> Result<Vec<RawEntry>> {
        let parts = path::parse(path)?;
        let loc = self.resolve_dir(&parts)?;
        Ok(dir::slots(&self.disk, &self.geo, loc)?
            .into_iter()
            .map(|s| RawEntry::parse(self.disk.read(s.offset, ENTRY_SIZE)))
            .collect())
    }

    /// Which file or directory each allocated cluster belongs to. One full
    /// directory-tree walk; call once and reuse across `annotate_sector_with`.
    pub fn cluster_owners(&self) -> BTreeMap<u32, ClusterOwner> {
        let mut owners = BTreeMap::new();
        let mut visited = HashSet::new();
        let mut stack = vec![(DirLocation::Root, String::new())];
        while let Some((loc, prefix)) = stack.pop() {
            let Ok(entries) = self.scan_dir(loc) else {
                continue;
            };
            for l in entries.into_iter().filter(|l| !l.entry.is_dot_entry()) {
                let path = format!("{prefix}/{}", l.name());
                let first = l.entry.first_cluster();
                if first != 0 {
                    if let Ok(chain) = table::chain(&self.disk, &self.geo, first) {
                        for c in chain {
                            owners.insert(
                                c,
                                ClusterOwner {
                                    path: path.clone(),
                                    is_dir: l.entry.is_dir(),
                                    first_cluster: first,
                                },
                            );
                        }
                    }
                    if l.entry.is_dir() && visited.insert(first) {
                        stack.push((DirLocation::Cluster(first), path));
                    }
                }
            }
        }
        owners
    }

    pub fn annotate_sector(&self, sector: u64) -> Vec<Annotation> {
        self.annotate_sector_with(sector, &self.cluster_owners())
    }

    /// Same as `annotate_sector`, but reuses a cluster-owner map computed
    /// once (with `cluster_owners`) instead of recomputing it for every
    /// sector.
    pub fn annotate_sector_with(
        &self,
        sector: u64,
        owners: &BTreeMap<u32, ClusterOwner>,
    ) -> Vec<Annotation> {
        let g = &self.geo;
        if sector >= g.total_sectors {
            return Vec::new();
        }
        if sector == 0 {
            return self.annotate_boot_sector();
        }
        if sector < g.reserved_sectors {
            return vec![annotation(
                0..g.bytes_per_sector,
                "reserved",
                "unused reserved sector",
            )];
        }
        if sector < g.first_root_dir_sector {
            return self.annotate_fat_sector(sector);
        }
        if sector < g.first_data_sector {
            let entries_per_sector = g.bytes_per_sector / ENTRY_SIZE;
            let first_slot = (sector - g.first_root_dir_sector) as usize * entries_per_sector;
            return self.annotate_dir_sector(sector, first_slot);
        }
        let Some(cluster) = g.cluster_of_sector(sector) else {
            return vec![annotation(
                0..g.bytes_per_sector,
                "unused",
                "sector beyond the last cluster",
            )];
        };
        match owners.get(&cluster) {
            Some(owner) if owner.is_dir => {
                let entries_per_sector = g.bytes_per_sector / ENTRY_SIZE;
                let cluster_size_slots = g.cluster_size() / ENTRY_SIZE;
                let pos = table::chain(&self.disk, g, owner.first_cluster)
                    .ok()
                    .and_then(|c| c.iter().position(|&x| x == cluster))
                    .unwrap_or(0);
                let first_sector_of_cluster =
                    g.first_data_sector + (cluster - 2) as u64 * g.sectors_per_cluster as u64;
                let first_slot = pos * cluster_size_slots
                    + (sector - first_sector_of_cluster) as usize * entries_per_sector;
                self.annotate_dir_sector(sector, first_slot)
            }
            Some(owner) => vec![annotation(
                0..g.bytes_per_sector,
                format!("cluster {cluster}"),
                format!("data of {}", owner.path),
            )],
            None => {
                let state = match table::read_entry(&self.disk, g, cluster) {
                    FatEntry::Free => "free cluster".to_string(),
                    other => format!("cluster marked {other} but owned by no file"),
                };
                vec![annotation(
                    0..g.bytes_per_sector,
                    format!("cluster {cluster}"),
                    state,
                )]
            }
        }
    }

    fn annotate_boot_sector(&self) -> Vec<Annotation> {
        let b = &self.boot;
        let jump = self.disk.read(0, 3);
        let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).to_string();
        vec![
            annotation(
                0..3,
                "jump instruction",
                format!("{:02X} {:02X} {:02X}", jump[0], jump[1], jump[2]),
            ),
            annotation(3..11, "OEM name", text(&b.oem_name)),
            annotation(11..13, "bytes per sector", b.bytes_per_sector.to_string()),
            annotation(
                13..14,
                "sectors per cluster",
                b.sectors_per_cluster.to_string(),
            ),
            annotation(14..16, "reserved sectors", b.reserved_sectors.to_string()),
            annotation(16..17, "FAT count", b.fat_count.to_string()),
            annotation(17..19, "root directory entries", b.root_entries.to_string()),
            annotation(
                19..21,
                "total sectors (16-bit)",
                b.total_sectors_16.to_string(),
            ),
            annotation(21..22, "media descriptor", format!("0x{:02X}", b.media)),
            annotation(22..24, "sectors per FAT", b.sectors_per_fat.to_string()),
            annotation(24..26, "sectors per track", b.sectors_per_track.to_string()),
            annotation(26..28, "heads", b.heads.to_string()),
            annotation(28..32, "hidden sectors", b.hidden_sectors.to_string()),
            annotation(
                32..36,
                "total sectors (32-bit)",
                b.total_sectors_32.to_string(),
            ),
            annotation(36..37, "drive number", format!("0x{:02X}", b.drive_number)),
            annotation(37..38, "reserved", "0"),
            annotation(
                38..39,
                "extended boot signature",
                format!("0x{:02X}", b.boot_signature),
            ),
            annotation(39..43, "volume ID", format!("0x{:08X}", b.volume_id)),
            annotation(43..54, "volume label", text(&b.volume_label)),
            annotation(54..62, "filesystem type", text(&b.fs_type)),
            annotation(62..510, "boot code", "unused"),
            annotation(510..512, "boot signature", "55 AA"),
        ]
    }

    fn annotate_fat_sector(&self, sector: u64) -> Vec<Annotation> {
        let g = &self.geo;
        let fat = ((sector - g.reserved_sectors) / g.sectors_per_fat) as u8;
        let sector_in_fat = (sector - g.reserved_sectors) % g.sectors_per_fat;
        let entries_per_sector = g.bytes_per_sector / 2;
        let first_cluster = sector_in_fat as usize * entries_per_sector;
        (0..entries_per_sector)
            .map(|i| {
                let cluster = (first_cluster + i) as u32;
                let range = i * 2..i * 2 + 2;
                if cluster >= g.cluster_count + 2 {
                    return annotation(
                        range,
                        format!("entry {cluster}"),
                        "unused (beyond last cluster)",
                    );
                }
                let label = match cluster {
                    0 => "cluster 0 (media descriptor)".to_string(),
                    1 => "cluster 1 (reserved)".to_string(),
                    _ => format!("cluster {cluster}"),
                };
                let value = if cluster < 2 {
                    let raw = self.disk.read(g.fat_entry_offset(fat, cluster), 2);
                    format!("0x{:04X}", u16::from_le_bytes([raw[0], raw[1]]))
                } else {
                    table::read_entry_from(&self.disk, g, fat, cluster).to_string()
                };
                annotation(range, label, value)
            })
            .collect()
    }

    fn annotate_dir_sector(&self, sector: u64, first_slot: usize) -> Vec<Annotation> {
        let g = &self.geo;
        let base = sector as usize * g.bytes_per_sector;
        (0..g.bytes_per_sector / ENTRY_SIZE)
            .map(|i| {
                let off = base + i * ENTRY_SIZE;
                let raw = RawEntry::parse(self.disk.read(off, ENTRY_SIZE));
                annotation(
                    i * ENTRY_SIZE..(i + 1) * ENTRY_SIZE,
                    format!("slot {}", first_slot + i),
                    raw.describe(),
                )
            })
            .collect()
    }
}

/// Which file or directory a data cluster belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterOwner {
    /// Absolute path, e.g. `/DOCS/NOTES.TXT`.
    pub path: String,
    pub is_dir: bool,
    /// First cluster of the owner's chain.
    pub first_cluster: u32,
}

/// A directory entry found on disk, with where it lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Located {
    pub dir: DirLocation,
    /// Slot index of the short entry.
    pub slot: usize,
    /// Slot indices of the long-name entries that precede it, in disk order.
    pub lfn_slots: Vec<usize>,
    pub entry: ShortEntry,
    pub long_name: Option<String>,
}

impl Located {
    pub(crate) fn name(&self) -> String {
        self.long_name
            .clone()
            .unwrap_or_else(|| self.entry.display_name())
    }

    pub(crate) fn info(&self) -> EntryInfo {
        EntryInfo {
            name: self.name(),
            is_dir: self.entry.is_dir(),
            size: self.entry.size as u64,
            created: dir_entry::unpack(self.entry.create_date, self.entry.create_time),
            modified: dir_entry::unpack(self.entry.write_date, self.entry.write_time),
            accessed: dir_entry::unpack(self.entry.access_date, 0),
        }
    }
}

/// One directory slot exactly as it is on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawEntry {
    Free,
    Deleted { bytes: [u8; 32] },
    Short(ShortEntry),
    Lfn(LfnEntry),
}

impl RawEntry {
    pub fn parse(bytes: &[u8]) -> RawEntry {
        match bytes[0] {
            FREE => RawEntry::Free,
            DELETED => {
                let mut copy = [0u8; ENTRY_SIZE];
                copy.copy_from_slice(&bytes[..ENTRY_SIZE]);
                RawEntry::Deleted { bytes: copy }
            }
            _ if attr::is_lfn(bytes[11]) => RawEntry::Lfn(LfnEntry::parse(bytes)),
            _ => RawEntry::Short(ShortEntry::parse(bytes)),
        }
    }

    /// One line describing the slot, for annotations.
    pub fn describe(&self) -> String {
        match self {
            RawEntry::Free => "free".into(),
            RawEntry::Deleted { bytes } => {
                let mut name = *b"?          ";
                name[1..].copy_from_slice(&bytes[1..11]);
                let mut e = ShortEntry::parse(bytes);
                e.name = name;
                format!("deleted (was {})", e.display_name())
            }
            RawEntry::Short(e) => {
                let kind = if e.is_dir() {
                    "directory"
                } else if e.is_volume_label() {
                    "volume label"
                } else {
                    "file"
                };
                format!(
                    "{} {} attr=0x{:02X} first_cluster={} size={}",
                    kind,
                    e.display_name(),
                    e.attr,
                    e.first_cluster(),
                    e.size
                )
            }
            RawEntry::Lfn(e) => format!(
                "LFN part {}{} checksum=0x{:02X} \"{}\"",
                e.order(),
                if e.is_last() { " (last)" } else { "" },
                e.checksum,
                name::from_ucs2(&e.chars())
            ),
        }
    }
}

fn annotation(
    range: Range<usize>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> Annotation {
    Annotation {
        range,
        label: label.into(),
        value: value.into(),
    }
}

/// Reassemble a long name from the LFN entries collected before a short
/// entry, or `None` if they are incomplete or do not match its checksum.
fn assemble_lfn(pending: &[(usize, LfnEntry)], short_name: &[u8; 11]) -> Option<String> {
    let (_, last) = pending.last()?;
    let (_, first) = &pending[0];
    if last.order() != 1 || !first.is_last() || first.order() as usize != pending.len() {
        return None;
    }
    let sum = dir_entry::lfn_checksum(short_name);
    if pending.iter().any(|(_, e)| e.checksum != sum) {
        return None;
    }
    let mut units = Vec::with_capacity(pending.len() * 13);
    for (_, e) in pending.iter().rev() {
        units.extend(e.chars());
    }
    Some(name::from_ucs2(&units))
}

fn names_match(a: &str, b: &str) -> bool {
    if a.is_ascii() && b.is_ascii() {
        a.eq_ignore_ascii_case(b)
    } else {
        a.to_uppercase() == b.to_uppercase()
    }
}

impl FileSystem for FatFs {
    fn fs_type(&self) -> &'static str {
        FatFs::fs_type(self)
    }
    fn create_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        FatFs::create_file(self, path, data)
    }
    fn write_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        FatFs::write_file(self, path, data)
    }
    fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        FatFs::read_file(self, path)
    }
    fn delete_file(&mut self, path: &str) -> Result<OpRecord> {
        FatFs::delete_file(self, path)
    }
    fn create_dir(&mut self, path: &str) -> Result<OpRecord> {
        FatFs::create_dir(self, path)
    }
    fn remove_dir(&mut self, path: &str) -> Result<OpRecord> {
        FatFs::remove_dir(self, path)
    }
    fn list_dir(&self, path: &str) -> Result<Vec<EntryInfo>> {
        FatFs::list_dir(self, path)
    }
    fn stat(&self, path: &str) -> Result<EntryInfo> {
        FatFs::stat(self, path)
    }
    fn set_now(&mut self, now: DateTime) {
        FatFs::set_now(self, now)
    }
    fn disk(&self) -> &Disk {
        FatFs::disk(self)
    }
    fn layout(&self) -> Vec<Region> {
        FatFs::layout(self)
    }
    fn annotate_sector(&self, sector: u64) -> Vec<Annotation> {
        FatFs::annotate_sector(self, sector)
    }
    fn history(&self) -> &[OpRecord] {
        FatFs::history(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table;
    use crate::table::FatEntry;
    use fs_core::{Error, RegionKind};

    fn tiny_fs() -> FatFs {
        FatFs::format(FormatOptions {
            total_sectors: 128,
            sectors_per_cluster: 1,
            root_entries: 16,
            enforce_fat16_range: false,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn format_writes_boot_sector_and_fat_headers() {
        let fs = FatFs::format(FormatOptions::default()).unwrap();
        assert_eq!(fs.fs_type(), "FAT16");
        assert_eq!(fs.disk().len(), 32768 * 512);
        assert_eq!(
            BootSector::parse(fs.disk().sector(0)).unwrap(),
            *fs.boot_sector()
        );
        for fat in 0..2 {
            let off = fs.geometry().fat_offset(fat);
            assert_eq!(fs.disk().read(off, 4), &[0xF8, 0xFF, 0xFF, 0xFF]);
        }
        assert_eq!(
            table::count_free(fs.disk(), fs.geometry()),
            fs.geometry().cluster_count
        );
        assert!(fs.history().is_empty());
        assert_eq!(fs.now(), DateTime::default());
    }

    #[test]
    fn layout_regions_are_contiguous_and_cover_the_disk() {
        let fs = FatFs::format(FormatOptions::default()).unwrap();
        let regions = fs.layout();
        let names: Vec<&str> = regions.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "reserved (boot sector)",
                "FAT 0",
                "FAT 1",
                "root directory",
                "data"
            ]
        );
        assert_eq!(regions[0].kind, RegionKind::Boot);
        assert_eq!(regions[1].kind, RegionKind::AllocationTable);
        assert_eq!(regions[3].kind, RegionKind::Directory);
        assert_eq!(regions[4].kind, RegionKind::Data);
        let mut next = 0;
        for r in &regions {
            assert_eq!(r.sectors.start, next);
            next = r.sectors.end;
        }
        assert_eq!(next, 32768);
    }

    #[test]
    fn from_image_round_trips_and_validates() {
        let fs = FatFs::format(FormatOptions::default()).unwrap();
        let image = fs.disk().as_bytes().to_vec();
        let again = FatFs::from_image(image.clone()).unwrap();
        assert_eq!(again.geometry(), fs.geometry());
        assert_eq!(again.disk().as_bytes(), fs.disk().as_bytes());

        let mut bad = image.clone();
        bad[510] = 0;
        assert!(matches!(
            FatFs::from_image(bad),
            Err(Error::CorruptImage(_))
        ));
        assert!(matches!(
            FatFs::from_image(vec![0; 100]),
            Err(Error::CorruptImage(_))
        ));
        let short = image[..image.len() - 512].to_vec();
        assert!(matches!(
            FatFs::from_image(short),
            Err(Error::CorruptImage(_))
        ));

        let mut tiny_fat = image.clone();
        tiny_fat[22..24].copy_from_slice(&1u16.to_le_bytes());
        assert!(matches!(
            FatFs::from_image(tiny_fat),
            Err(Error::CorruptImage(_))
        ));
    }

    #[test]
    fn set_now_is_stored() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let t = DateTime::new(2026, 9, 21, 1, 2, 3);
        fs.set_now(t);
        assert_eq!(fs.now(), t);
    }

    use crate::dir_entry::{attr, lfn_checksum, LfnEntry, ShortEntry, DELETED, ENTRY_SIZE};
    use crate::name;

    fn write_root_slot(fs: &mut FatFs, slot: usize, bytes: &[u8; 32]) {
        let off = fs.geo.root_dir_offset() + slot * ENTRY_SIZE;
        fs.disk.write(off, bytes);
    }

    fn short(name: &[u8; 11], attributes: u8) -> ShortEntry {
        ShortEntry::new(*name, attributes, &DateTime::new(2026, 9, 21, 10, 0, 0))
    }

    #[test]
    fn empty_root_lists_nothing_and_stat_root_works() {
        let fs = FatFs::format(FormatOptions::default()).unwrap();
        assert_eq!(fs.list_dir("/").unwrap(), Vec::new());
        let root = fs.stat("/").unwrap();
        assert_eq!(root.name, "/");
        assert!(root.is_dir);
        assert_eq!(fs.stat("/NOPE"), Err(Error::NotFound));
        assert_eq!(fs.list_dir("/NOPE"), Err(Error::NotFound));
        assert_eq!(fs.list_dir("relative"), Err(Error::InvalidPath));
    }

    #[test]
    fn short_entries_are_listed_with_display_names_and_timestamps() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let mut e = short(b"README  TXT", attr::ARCHIVE);
        e.size = 5;
        write_root_slot(&mut fs, 0, &e.to_bytes());
        write_root_slot(
            &mut fs,
            1,
            &short(b"LABEL      ", attr::VOLUME_ID).to_bytes(),
        );
        write_root_slot(
            &mut fs,
            2,
            &short(b"DOCS       ", attr::DIRECTORY).to_bytes(),
        );
        let list = fs.list_dir("/").unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "README.TXT");
        assert_eq!(list[0].size, 5);
        assert!(!list[0].is_dir);
        assert_eq!(list[0].modified, Some(DateTime::new(2026, 9, 21, 10, 0, 0)));
        assert_eq!(list[0].accessed, Some(DateTime::new(2026, 9, 21, 0, 0, 0)));
        assert_eq!(list[1].name, "DOCS");
        assert!(list[1].is_dir);
        assert_eq!(fs.stat("/readme.txt").unwrap().name, "README.TXT");
    }

    #[test]
    fn long_names_are_reassembled_when_valid() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let short_name = *b"MYFILE~1TXT";
        let sum = lfn_checksum(&short_name);
        let chunks = name::lfn_chunks("My File.txt");
        write_root_slot(
            &mut fs,
            0,
            &LfnEntry::new(1, true, sum, &chunks[0]).to_bytes(),
        );
        write_root_slot(&mut fs, 1, &short(&short_name, attr::ARCHIVE).to_bytes());
        // A two-entry name: 14 A's, written highest order first.
        let long = "A".repeat(14);
        let short2 = *b"AAAAAA~1   ";
        let sum2 = lfn_checksum(&short2);
        let chunks2 = name::lfn_chunks(&long);
        write_root_slot(
            &mut fs,
            2,
            &LfnEntry::new(2, true, sum2, &chunks2[1]).to_bytes(),
        );
        write_root_slot(
            &mut fs,
            3,
            &LfnEntry::new(1, false, sum2, &chunks2[0]).to_bytes(),
        );
        write_root_slot(&mut fs, 4, &short(&short2, attr::ARCHIVE).to_bytes());
        let list = fs.list_dir("/").unwrap();
        assert_eq!(list[0].name, "My File.txt");
        assert_eq!(list[1].name, long);
        assert_eq!(fs.stat("/my file.TXT").unwrap().name, "My File.txt");
        assert_eq!(fs.stat("/MYFILE~1.TXT").unwrap().name, "My File.txt");
    }

    #[test]
    fn orphaned_or_mismatched_lfn_entries_are_ignored() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let chunks = name::lfn_chunks("Wrong.txt");
        // Checksum does not match the short name that follows.
        write_root_slot(
            &mut fs,
            0,
            &LfnEntry::new(1, true, 0x11, &chunks[0]).to_bytes(),
        );
        write_root_slot(&mut fs, 1, &short(b"REAL    TXT", attr::ARCHIVE).to_bytes());
        // A deleted LFN entry followed by a short entry.
        let mut deleted =
            LfnEntry::new(1, true, lfn_checksum(b"OTHER   TXT"), &chunks[0]).to_bytes();
        deleted[0] = DELETED;
        write_root_slot(&mut fs, 2, &deleted);
        write_root_slot(&mut fs, 3, &short(b"OTHER   TXT", attr::ARCHIVE).to_bytes());
        let names: Vec<String> = fs
            .list_dir("/")
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, vec!["REAL.TXT", "OTHER.TXT"]);
    }

    #[test]
    fn read_file_follows_the_chain_and_respects_size() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let clusters = table::allocate_chain(&mut fs.disk, &fs.geo, 2).unwrap();
        let cs = fs.geo.cluster_size();
        fs.disk.fill(fs.geo.cluster_offset(clusters[0]), cs, b'x');
        fs.disk.fill(fs.geo.cluster_offset(clusters[1]), cs, b'y');
        let mut e = short(b"DATA    BIN", attr::ARCHIVE);
        e.set_first_cluster(clusters[0]);
        e.size = (cs + 3) as u32;
        write_root_slot(&mut fs, 0, &e.to_bytes());
        let mut d = short(b"SUB        ", attr::DIRECTORY);
        d.set_first_cluster(clusters[1]);
        write_root_slot(&mut fs, 1, &d.to_bytes());
        write_root_slot(&mut fs, 2, &short(b"EMPTY   TXT", attr::ARCHIVE).to_bytes());

        let data = fs.read_file("/DATA.BIN").unwrap();
        assert_eq!(data.len(), cs + 3);
        assert!(data[..cs].iter().all(|&b| b == b'x'));
        assert_eq!(&data[cs..], b"yyy");
        assert_eq!(fs.read_file("/EMPTY.TXT").unwrap(), Vec::<u8>::new());
        assert_eq!(fs.read_file("/SUB"), Err(Error::IsADirectory));
        assert_eq!(fs.read_file("/"), Err(Error::IsADirectory));
        assert_eq!(fs.read_file("/MISSING"), Err(Error::NotFound));
        assert_eq!(fs.list_dir("/DATA.BIN"), Err(Error::NotADirectory));
    }

    #[test]
    fn nonzero_size_with_no_cluster_is_corrupt() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let mut e = short(b"BROKEN  BIN", attr::ARCHIVE);
        e.size = 10;
        write_root_slot(&mut fs, 0, &e.to_bytes());
        assert!(matches!(
            fs.read_file("/BROKEN.BIN"),
            Err(Error::CorruptImage(_))
        ));
    }

    #[test]
    fn absurd_size_does_not_preallocate() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let mut e = short(b"BROKEN  BIN", attr::ARCHIVE);
        e.size = u32::MAX;
        write_root_slot(&mut fs, 0, &e.to_bytes());
        assert!(matches!(
            fs.read_file("/BROKEN.BIN"),
            Err(Error::CorruptImage(_))
        ));
    }

    #[test]
    fn nested_directory_is_resolved_through_its_cluster() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let c = table::allocate_chain(&mut fs.disk, &fs.geo, 1).unwrap()[0];
        let mut d = short(b"SUB        ", attr::DIRECTORY);
        d.set_first_cluster(c);
        write_root_slot(&mut fs, 0, &d.to_bytes());
        let base = fs.geo.cluster_offset(c);
        let mut dot = short(b".          ", attr::DIRECTORY);
        dot.set_first_cluster(c);
        fs.disk.write(base, &dot.to_bytes());
        fs.disk.write(
            base + ENTRY_SIZE,
            &short(b"..         ", attr::DIRECTORY).to_bytes(),
        );
        fs.disk.write(
            base + 2 * ENTRY_SIZE,
            &short(b"INNER   TXT", attr::ARCHIVE).to_bytes(),
        );
        let names: Vec<String> = fs
            .list_dir("/SUB")
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, vec!["INNER.TXT"]);
        assert_eq!(fs.stat("/sub/inner.txt").unwrap().name, "INNER.TXT");
    }

    #[test]
    fn create_file_round_trips_and_records_the_operation() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        fs.set_now(DateTime::new(2026, 9, 21, 8, 30, 0));
        let cs = fs.geo.cluster_size();
        let data: Vec<u8> = (0..cs * 2 + 10).map(|i| i as u8).collect();
        let rec = fs.create_file("/HELLO.TXT", &data).unwrap();
        assert_eq!(rec.op, "create_file /HELLO.TXT");
        assert!(!rec.changes.is_empty());
        let kinds = rec.event_kinds();
        assert_eq!(
            kinds.iter().filter(|k| **k == "cluster_allocated").count(),
            3
        );
        assert_eq!(kinds.iter().filter(|k| **k == "data_written").count(), 3);
        assert_eq!(
            kinds.iter().filter(|k| **k == "dir_entry_written").count(),
            1
        );
        assert_eq!(fs.read_file("/hello.txt").unwrap(), data);
        let info = fs.stat("/HELLO.TXT").unwrap();
        assert_eq!(info.size, data.len() as u64);
        assert_eq!(info.created, Some(DateTime::new(2026, 9, 21, 8, 30, 0)));
        assert_eq!(fs.history().len(), 1);
        assert_eq!(
            fs.create_file("/hello.txt", b"x").unwrap_err(),
            Error::AlreadyExists
        );
        assert_eq!(fs.history().len(), 1);
    }

    #[test]
    fn empty_file_has_no_clusters() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let rec = fs.create_file("/EMPTY", b"").unwrap();
        assert!(!rec.event_kinds().contains(&"cluster_allocated"));
        assert_eq!(fs.read_file("/EMPTY").unwrap(), Vec::<u8>::new());
        assert_eq!(table::count_free(&fs.disk, &fs.geo), fs.geo.cluster_count);
    }

    #[test]
    fn exact_cluster_multiple_uses_no_extra_cluster() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let cs = fs.geo.cluster_size();
        fs.create_file("/TWO.BIN", &vec![7u8; cs * 2]).unwrap();
        assert_eq!(
            table::count_free(&fs.disk, &fs.geo),
            fs.geo.cluster_count - 2
        );
        assert_eq!(fs.read_file("/TWO.BIN").unwrap().len(), cs * 2);
    }

    #[test]
    fn long_names_get_lfn_entries_and_tilde_aliases() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let rec = fs.create_file("/My First File.txt", b"1").unwrap();
        assert_eq!(
            rec.event_kinds()
                .iter()
                .filter(|k| **k == "dir_entry_written")
                .count(),
            3
        );
        fs.create_file("/My Second File.txt", b"2").unwrap();
        let list = fs.list_dir("/").unwrap();
        assert_eq!(list[0].name, "My First File.txt");
        assert_eq!(list[1].name, "My Second File.txt");
        assert_eq!(fs.stat("/MYFIRS~1.TXT").unwrap().name, "My First File.txt");
        assert_eq!(fs.stat("/MYSECO~1.TXT").unwrap().name, "My Second File.txt");
        fs.create_file("/my first file.doc", b"3").unwrap();
        assert_eq!(fs.stat("/MYFIRS~1.DOC").unwrap().name, "my first file.doc");
        fs.create_file("/My First File (copy).txt", b"4").unwrap();
        assert_eq!(
            fs.stat("/MYFIRS~2.TXT").unwrap().name,
            "My First File (copy).txt"
        );
        assert_eq!(fs.read_file("/MY FIRST FILE (COPY).TXT").unwrap(), b"4");
        assert_eq!(
            fs.create_file("/bad*name", b"").unwrap_err(),
            Error::InvalidName
        );
        assert_eq!(fs.create_file("/", b"").unwrap_err(), Error::InvalidPath);
    }

    #[test]
    fn create_file_writes_the_standard_lfn_layout() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let name = "A very long file name that needs several LFN entries.txt"; // 56 chars -> 5 LFN entries
        fs.create_file(&format!("/{name}"), b"x").unwrap();
        let base = fs.geo.root_dir_offset();
        let short = *b"AVERYL~1TXT";
        let sum = dir_entry::lfn_checksum(&short);
        let units = name::to_ucs2(name);
        for i in 0..5 {
            let slot = fs.disk.read(base + i * ENTRY_SIZE, ENTRY_SIZE);
            let order = 5 - i as u8;
            let expected_seq = if i == 0 { order | 0x40 } else { order };
            assert_eq!(slot[0], expected_seq, "slot {i} sequence");
            assert_eq!(slot[11], attr::LFN);
            assert_eq!(slot[13], sum);
            assert_eq!(&slot[26..28], &[0, 0]);
            let entry = LfnEntry::parse(slot);
            let chars = entry.chars();
            let start = (order as usize - 1) * 13;
            for (j, c) in chars.iter().enumerate() {
                let expected = match units.get(start + j) {
                    Some(u) => *u,
                    None if start + j == units.len() => 0x0000,
                    None => 0xFFFF,
                };
                assert_eq!(*c, expected, "slot {i} char {j}");
            }
        }
        let short_slot = fs.disk.read(base + 5 * ENTRY_SIZE, ENTRY_SIZE);
        assert_eq!(&short_slot[0..11], &short);
    }

    #[test]
    fn create_dir_writes_dot_entries_and_nests() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let rec = fs.create_dir("/DOCS").unwrap();
        assert_eq!(
            rec.event_kinds()
                .iter()
                .filter(|k| **k == "dir_entry_written")
                .count(),
            3
        );
        assert!(fs.stat("/DOCS").unwrap().is_dir);
        assert_eq!(fs.list_dir("/DOCS").unwrap(), Vec::new());
        fs.create_dir("/DOCS/NOTES").unwrap();
        fs.create_file("/DOCS/NOTES/a.txt", b"hi").unwrap();
        assert_eq!(fs.read_file("/docs/notes/A.TXT").unwrap(), b"hi");
        assert_eq!(fs.list_dir("/DOCS").unwrap()[0].name, "NOTES");
        let docs = fs.resolve("/DOCS").unwrap().unwrap();
        let notes = fs.resolve("/DOCS/NOTES").unwrap().unwrap();
        let base = fs.geo.cluster_offset(notes.entry.first_cluster());
        let dot = ShortEntry::parse(fs.disk.read(base, ENTRY_SIZE));
        let dotdot = ShortEntry::parse(fs.disk.read(base + ENTRY_SIZE, ENTRY_SIZE));
        assert_eq!(dot.name, *b".          ");
        assert_eq!(dot.first_cluster(), notes.entry.first_cluster());
        assert_eq!(dotdot.name, *b"..         ");
        assert_eq!(dotdot.first_cluster(), docs.entry.first_cluster());
        let docs_base = fs.geo.cluster_offset(docs.entry.first_cluster());
        assert_eq!(
            ShortEntry::parse(fs.disk.read(docs_base + ENTRY_SIZE, ENTRY_SIZE)).first_cluster(),
            0
        );
        assert_eq!(fs.create_dir("/DOCS").unwrap_err(), Error::AlreadyExists);
        assert_eq!(
            fs.create_file("/DOCS/NOTES/a.txt/x", b"").unwrap_err(),
            Error::NotADirectory
        );
        assert_eq!(fs.create_file("/NOPE/x", b"").unwrap_err(), Error::NotFound);
    }

    #[test]
    fn subdirectory_grows_when_its_cluster_fills() {
        let mut fs = tiny_fs();
        fs.create_dir("/SUB").unwrap();
        let free_before = table::count_free(&fs.disk, &fs.geo);
        // 16 slots per 512-byte cluster; two are taken by . and ..
        let mut grew = false;
        for i in 0..20 {
            let rec = fs.create_file(&format!("/SUB/F{i}"), b"").unwrap();
            grew |= rec.event_kinds().contains(&"directory_grown");
        }
        assert!(grew);
        assert_eq!(fs.list_dir("/SUB").unwrap().len(), 20);
        assert_eq!(table::count_free(&fs.disk, &fs.geo), free_before - 1);
    }

    #[test]
    fn root_directory_full_and_disk_full_leave_nothing_behind() {
        let mut fs = tiny_fs();
        for i in 0..16 {
            fs.create_file(&format!("/F{i}"), b"").unwrap();
        }
        assert_eq!(
            fs.create_file("/F16", b"").unwrap_err(),
            Error::DirectoryFull
        );
        assert_eq!(
            fs.create_file("/Long name needs two slots", b"")
                .unwrap_err(),
            Error::DirectoryFull
        );
        assert_eq!(fs.list_dir("/").unwrap().len(), 16);
        assert_eq!(fs.history().len(), 16);

        let mut fs = tiny_fs();
        let free = table::count_free(&fs.disk, &fs.geo) as usize;
        let cs = fs.geo.cluster_size();
        assert_eq!(
            fs.create_file("/BIG", &vec![1u8; (free + 1) * cs])
                .unwrap_err(),
            Error::DiskFull
        );
        assert_eq!(fs.list_dir("/").unwrap(), Vec::new());
        assert_eq!(table::count_free(&fs.disk, &fs.geo) as usize, free);
        fs.create_file("/FITS", &vec![1u8; free * cs]).unwrap();
        assert_eq!(table::count_free(&fs.disk, &fs.geo), 0);
        assert_eq!(fs.create_dir("/D").unwrap_err(), Error::DiskFull);
        assert_eq!(fs.history().len(), 1);
    }

    #[test]
    fn disk_full_during_directory_growth_rolls_back_the_grown_cluster() {
        let mut fs = FatFs::format(FormatOptions {
            total_sectors: 200,
            sectors_per_cluster: 1,
            root_entries: 16,
            enforce_fat16_range: false,
            ..Default::default()
        })
        .unwrap();
        fs.create_dir("/D").unwrap();
        // Fill the disk except one cluster.
        let cs = fs.geo.cluster_size();
        let free = table::count_free(&fs.disk, &fs.geo) as usize;
        fs.create_file("/FILL", &vec![0u8; (free - 1) * cs])
            .unwrap();
        // Occupy the free slots in /D's single cluster (16 slots: . .. + 14).
        for i in 0..14 {
            fs.create_file(&format!("/D/F{i}"), b"").unwrap();
        }
        let chain_before = fs
            .cluster_chain(fs.resolve("/D").unwrap().unwrap().entry.first_cluster())
            .unwrap();
        let free_before = table::count_free(&fs.disk, &fs.geo);
        let image_before = fs.disk.as_bytes().to_vec();
        let history_before = fs.history().len();
        // 200-character name -> 16 LFN entries + 1 short = 17 slots: needs two grows, only one cluster is free.
        let long = "L".repeat(200);
        assert_eq!(
            fs.create_file(&format!("/D/{long}"), b"").unwrap_err(),
            Error::DiskFull
        );
        assert_eq!(table::count_free(&fs.disk, &fs.geo), free_before);
        assert_eq!(fs.cluster_chain(chain_before[0]).unwrap(), chain_before);
        assert_eq!(fs.history().len(), history_before);
        assert_eq!(fs.disk.as_bytes(), &image_before[..]);
    }

    #[test]
    fn create_in_full_root_with_data_frees_the_clusters_again() {
        let mut fs = tiny_fs();
        for i in 0..16 {
            fs.create_file(&format!("/F{i}"), b"").unwrap();
        }
        let free = table::count_free(&fs.disk, &fs.geo);
        assert_eq!(
            fs.create_file("/X", b"data").unwrap_err(),
            Error::DirectoryFull
        );
        assert_eq!(table::count_free(&fs.disk, &fs.geo), free);
    }

    #[test]
    fn delete_marks_entries_frees_chain_and_leaves_data() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        fs.create_file("/My File.txt", b"keep me around").unwrap();
        let located = fs.resolve("/My File.txt").unwrap().unwrap();
        let first = located.entry.first_cluster();
        let data_off = fs.geo.cluster_offset(first);
        let rec = fs.delete_file("/my file.txt").unwrap();
        assert_eq!(rec.op, "delete_file /my file.txt");
        let kinds = rec.event_kinds();
        assert_eq!(
            kinds.iter().filter(|k| **k == "dir_entry_deleted").count(),
            2
        );
        assert_eq!(kinds.iter().filter(|k| **k == "cluster_freed").count(), 1);
        assert_eq!(fs.stat("/My File.txt"), Err(Error::NotFound));
        assert_eq!(fs.list_dir("/").unwrap(), Vec::new());
        let slots = dir::slots(&fs.disk, &fs.geo, DirLocation::Root).unwrap();
        assert_eq!(fs.disk.read(slots[0].offset, 1)[0], DELETED);
        assert_eq!(fs.disk.read(slots[1].offset, 1)[0], DELETED);
        assert_eq!(&fs.disk.read(slots[1].offset + 1, 10), b"YFILE~1TXT");
        assert_eq!(fs.disk.read(data_off, 14), b"keep me around");
        assert_eq!(table::read_entry(&fs.disk, &fs.geo, first), FatEntry::Free);
        assert_eq!(fs.delete_file("/My File.txt").unwrap_err(), Error::NotFound);
        assert_eq!(fs.delete_file("/").unwrap_err(), Error::InvalidPath);
        fs.create_dir("/D").unwrap();
        assert_eq!(fs.delete_file("/D").unwrap_err(), Error::IsADirectory);
    }

    #[test]
    fn deleted_slots_are_reused() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        fs.create_file("/A", b"").unwrap();
        fs.create_file("/B", b"").unwrap();
        fs.delete_file("/A").unwrap();
        fs.create_file("/C", b"").unwrap();
        let names: Vec<String> = fs
            .list_dir("/")
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, vec!["C", "B"]);
    }

    #[test]
    fn remove_dir_requires_empty() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        fs.create_dir("/D").unwrap();
        fs.create_file("/D/X", b"1").unwrap();
        assert_eq!(fs.remove_dir("/D").unwrap_err(), Error::DirectoryNotEmpty);
        fs.delete_file("/D/X").unwrap();
        let free = table::count_free(&fs.disk, &fs.geo);
        fs.remove_dir("/D").unwrap();
        assert_eq!(table::count_free(&fs.disk, &fs.geo), free + 1);
        assert_eq!(fs.stat("/D"), Err(Error::NotFound));
        fs.create_file("/F", b"").unwrap();
        assert_eq!(fs.remove_dir("/F").unwrap_err(), Error::NotADirectory);
        assert_eq!(fs.remove_dir("/").unwrap_err(), Error::InvalidPath);
    }

    #[test]
    fn write_file_replaces_data_and_reuses_the_entry() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let cs = fs.geo.cluster_size();
        fs.create_file("/F.TXT", &vec![1u8; cs * 3]).unwrap();
        let free = table::count_free(&fs.disk, &fs.geo);
        let slot_before = fs.resolve("/F.TXT").unwrap().unwrap().slot;
        fs.set_now(DateTime::new(2027, 1, 2, 3, 4, 6));
        let rec = fs.write_file("/F.TXT", b"small").unwrap();
        assert_eq!(
            rec.event_kinds()
                .iter()
                .filter(|k| **k == "cluster_freed")
                .count(),
            3
        );
        assert_eq!(
            rec.event_kinds()
                .iter()
                .filter(|k| **k == "cluster_allocated")
                .count(),
            1
        );
        assert_eq!(fs.read_file("/F.TXT").unwrap(), b"small");
        assert_eq!(table::count_free(&fs.disk, &fs.geo), free + 2);
        let after = fs.resolve("/F.TXT").unwrap().unwrap();
        assert_eq!(after.slot, slot_before);
        let info = after.info();
        assert_eq!(info.modified, Some(DateTime::new(2027, 1, 2, 3, 4, 6)));
        assert_eq!(info.created, Some(DateTime::default()));
        fs.write_file("/F.TXT", &vec![2u8; cs * 5]).unwrap();
        assert_eq!(fs.read_file("/F.TXT").unwrap().len(), cs * 5);
        assert_eq!(table::count_free(&fs.disk, &fs.geo), free - 2);
        fs.write_file("/F.TXT", b"").unwrap();
        assert_eq!(
            fs.resolve("/F.TXT").unwrap().unwrap().entry.first_cluster(),
            0
        );
        assert_eq!(fs.write_file("/NOPE", b"").unwrap_err(), Error::NotFound);
        fs.create_dir("/D").unwrap();
        assert_eq!(fs.write_file("/D", b"").unwrap_err(), Error::IsADirectory);
    }

    #[test]
    fn write_file_can_grow_into_its_own_old_clusters() {
        let mut fs = tiny_fs();
        let free = table::count_free(&fs.disk, &fs.geo) as usize;
        let cs = fs.geo.cluster_size();
        fs.create_file("/F", &vec![1u8; (free - 1) * cs]).unwrap();
        fs.write_file("/F", &vec![2u8; free * cs]).unwrap();
        assert_eq!(table::count_free(&fs.disk, &fs.geo), 0);
        assert_eq!(
            fs.write_file("/F", &vec![3u8; (free + 1) * cs])
                .unwrap_err(),
            Error::DiskFull
        );
        assert_eq!(fs.read_file("/F").unwrap()[0], 2);
    }

    use fs_core::FileSystem;

    #[test]
    fn fat_entries_and_cluster_chain() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let cs = fs.geo.cluster_size();
        fs.create_file("/F", &vec![0u8; cs * 3]).unwrap();
        let entries = fs.fat_entries(0);
        assert_eq!(entries.len() as u32, fs.geo.cluster_count + 2);
        assert_eq!(entries[0], FatEntry::Reserved);
        assert_eq!(entries[1], FatEntry::Reserved);
        assert_eq!(entries[2], FatEntry::Next(3));
        assert_eq!(entries[3], FatEntry::Next(4));
        assert_eq!(entries[4], FatEntry::EndOfChain);
        assert_eq!(entries[5], FatEntry::Free);
        assert_eq!(fs.fat_entries(1), entries);
        assert_eq!(fs.cluster_chain(2).unwrap(), vec![2, 3, 4]);
        assert!(matches!(fs.cluster_chain(0), Err(Error::CorruptImage(_))));
        assert!(fs.fat_entries(2).is_empty());
    }

    #[test]
    fn raw_dir_entries_show_every_slot() {
        let mut fs = tiny_fs();
        fs.create_file("/My File.txt", b"").unwrap();
        fs.create_file("/B", b"").unwrap();
        fs.delete_file("/B").unwrap();
        let raw = fs.raw_dir_entries("/").unwrap();
        assert_eq!(raw.len(), 16);
        assert!(matches!(&raw[0], RawEntry::Lfn(e) if e.order() == 1));
        assert!(matches!(&raw[1], RawEntry::Short(e) if e.name == *b"MYFILE~1TXT"));
        assert!(matches!(&raw[2], RawEntry::Deleted { bytes } if &bytes[1..11] == b"          "));
        assert_eq!(raw[3], RawEntry::Free);
        assert_eq!(
            fs.raw_dir_entries("/My File.txt"),
            Err(Error::NotADirectory)
        );
    }

    #[test]
    fn annotations_cover_boot_fat_directory_and_data_sectors() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        fs.create_dir("/DOCS").unwrap();
        fs.create_file("/DOCS/N.TXT", b"note").unwrap();
        let g = fs.geo.clone();

        let boot = fs.annotate_sector(0);
        assert!(boot
            .iter()
            .any(|a| a.label == "bytes per sector" && a.value == "512" && a.range == (11..13)));
        assert!(boot
            .iter()
            .any(|a| a.label == "boot signature" && a.range == (510..512)));

        let fat = fs.annotate_sector(g.reserved_sectors);
        assert_eq!(fat.len(), 256);
        assert_eq!(fat[0].range, 0..2);
        assert_eq!(fat[0].label, "cluster 0 (media descriptor)");
        assert_eq!(fat[2].label, "cluster 2");
        assert_eq!(fat[2].value, "end of chain");
        assert_eq!(fat[3].value, "end of chain");
        assert_eq!(fat[4].value, "free");

        let root = fs.annotate_sector(g.first_root_dir_sector);
        assert_eq!(root.len(), 16);
        assert_eq!(root[0].range, 0..32);
        assert!(root[0].value.contains("DOCS") && root[0].value.contains("directory"));
        assert_eq!(root[1].value, "free");

        let docs_cluster = fs.resolve("/DOCS").unwrap().unwrap().entry.first_cluster();
        let docs_sector =
            g.first_data_sector + (docs_cluster as u64 - 2) * g.sectors_per_cluster as u64;
        let docs = fs.annotate_sector(docs_sector);
        assert!(docs[0].value.contains(".") && docs[2].value.contains("N.TXT"));

        let n_cluster = fs
            .resolve("/DOCS/N.TXT")
            .unwrap()
            .unwrap()
            .entry
            .first_cluster();
        let n_sector = g.first_data_sector + (n_cluster as u64 - 2) * g.sectors_per_cluster as u64;
        let data = fs.annotate_sector(n_sector);
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].range, 0..512);
        assert_eq!(data[0].label, format!("cluster {n_cluster}"));
        assert_eq!(data[0].value, "data of /DOCS/N.TXT");

        let unused = fs.annotate_sector(n_sector + 8);
        assert_eq!(unused[0].value, "free cluster");
        assert!(fs.annotate_sector(g.total_sectors).is_empty());
    }

    #[test]
    fn annotate_sector_terminates_on_a_self_referencing_directory() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        fs.create_dir("/LOOP").unwrap();
        fs.create_file("/DATA.BIN", b"x").unwrap();
        let loop_cluster = fs.resolve("/LOOP").unwrap().unwrap().entry.first_cluster();
        // Corrupt the image: an entry inside /LOOP that points back at /LOOP itself.
        let base = fs.geo.cluster_offset(loop_cluster);
        let mut evil = ShortEntry::new(*b"EVIL       ", attr::DIRECTORY, &DateTime::default());
        evil.set_first_cluster(loop_cluster);
        fs.disk.write(base + 2 * ENTRY_SIZE, &evil.to_bytes());
        let data_cluster = fs
            .resolve("/DATA.BIN")
            .unwrap()
            .unwrap()
            .entry
            .first_cluster();
        let g = fs.geo.clone();
        let sector = g.first_data_sector + (data_cluster as u64 - 2) * g.sectors_per_cluster as u64;
        let ann = fs.annotate_sector(sector);
        assert_eq!(ann[0].value, "data of /DATA.BIN");
    }

    #[test]
    fn cluster_owners_maps_every_cluster_to_its_path() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        fs.create_dir("/DOCS").unwrap();
        let cs = fs.geo.cluster_size();
        fs.create_file("/DOCS/N.TXT", &vec![0u8; cs * 2 + 1])
            .unwrap();
        let owners = fs.cluster_owners();
        assert_eq!(owners.len(), 4);
        let docs_cluster = fs.resolve("/DOCS").unwrap().unwrap().entry.first_cluster();
        assert_eq!(
            owners.get(&docs_cluster),
            Some(&ClusterOwner {
                path: "/DOCS".to_string(),
                is_dir: true,
                first_cluster: docs_cluster,
            })
        );
        let n_first = fs
            .resolve("/DOCS/N.TXT")
            .unwrap()
            .unwrap()
            .entry
            .first_cluster();
        let n_chain = table::chain(&fs.disk, &fs.geo, n_first).unwrap();
        assert_eq!(n_chain.len(), 3);
        for c in n_chain {
            assert_eq!(
                owners.get(&c),
                Some(&ClusterOwner {
                    path: "/DOCS/N.TXT".to_string(),
                    is_dir: false,
                    first_cluster: n_first,
                })
            );
        }
    }

    #[test]
    fn annotate_sector_with_matches_annotate_sector() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        fs.create_dir("/DOCS").unwrap();
        let cs = fs.geo.cluster_size();
        fs.create_file("/DOCS/N.TXT", &vec![0u8; cs * 2 + 1])
            .unwrap();
        let owners = fs.cluster_owners();
        let g = fs.geo.clone();
        let docs_cluster = fs.resolve("/DOCS").unwrap().unwrap().entry.first_cluster();
        let docs_sector =
            g.first_data_sector + (docs_cluster as u64 - 2) * g.sectors_per_cluster as u64;
        let n_first = fs
            .resolve("/DOCS/N.TXT")
            .unwrap()
            .unwrap()
            .entry
            .first_cluster();
        let n_sector = g.first_data_sector + (n_first as u64 - 2) * g.sectors_per_cluster as u64;
        for s in [
            0,
            g.reserved_sectors,
            g.first_root_dir_sector,
            docs_sector,
            n_sector,
        ] {
            assert_eq!(fs.annotate_sector_with(s, &owners), fs.annotate_sector(s));
        }
    }

    #[test]
    fn root_directory_slots_are_numbered_across_sectors() {
        let fs = FatFs::format(FormatOptions::default()).unwrap();
        let g = fs.geo.clone();
        assert_eq!(
            fs.annotate_sector(g.first_root_dir_sector + 1)[0].label,
            "slot 16"
        );
    }

    #[test]
    fn subdirectory_slots_are_numbered_across_clusters() {
        let mut fs = tiny_fs();
        fs.create_dir("/SUB").unwrap();
        for i in 0..20 {
            fs.create_file(&format!("/SUB/F{i}"), b"").unwrap();
        }
        let sub_first = fs.resolve("/SUB").unwrap().unwrap().entry.first_cluster();
        let chain = table::chain(&fs.disk, &fs.geo, sub_first).unwrap();
        assert_eq!(chain.len(), 2);
        let g = fs.geo.clone();
        let second_sector =
            g.first_data_sector + (chain[1] as u64 - 2) * g.sectors_per_cluster as u64;
        let ann = fs.annotate_sector(second_sector);
        assert_eq!(ann[0].label, "slot 16");
        assert!(ann[0].value.contains("F14"), "value was {}", ann[0].value);
    }

    #[test]
    fn names_match_is_case_insensitive_for_ascii_and_unicode() {
        assert!(names_match("readme.txt", "README.TXT"));
        assert!(!names_match("readme.txt", "README.TX"));
        assert!(names_match("héllo", "HÉLLO"));
        assert!(!names_match("héllo", "hello"));
    }

    #[test]
    fn works_through_the_trait_object() {
        let mut fs: Box<dyn FileSystem> =
            Box::new(FatFs::format(FormatOptions::default()).unwrap());
        assert_eq!(fs.fs_type(), "FAT16");
        fs.create_dir("/A").unwrap();
        fs.create_file("/A/b.txt", b"via trait").unwrap();
        fs.write_file("/A/b.txt", b"again").unwrap();
        assert_eq!(fs.read_file("/A/B.TXT").unwrap(), b"again");
        assert_eq!(fs.list_dir("/A").unwrap()[0].name, "B.TXT");
        fs.delete_file("/A/b.txt").unwrap();
        fs.remove_dir("/A").unwrap();
        assert_eq!(fs.history().len(), 5);
        assert_eq!(fs.layout().len(), 5);
        assert_eq!(fs.disk().sector_count(), 32768);
        fs.set_now(DateTime::new(2000, 1, 1, 0, 0, 0));
        assert!(!fs.annotate_sector(0).is_empty());
    }
}
