//! Plain-data mirrors of the core types, shaped for JavaScript: camelCase
//! fields, tagged unions on `kind`, `Option` as `null`, bytes as `Uint8Array`.
//! Nothing here touches wasm-bindgen, so it all runs under `cargo test`.

use fat::FatVariant;
use fs_core::{Event, RegionKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl From<fs_core::DateTime> for DateTime {
    fn from(d: fs_core::DateTime) -> Self {
        DateTime {
            year: d.year,
            month: d.month,
            day: d.day,
            hour: d.hour,
            minute: d.minute,
            second: d.second,
        }
    }
}

impl From<DateTime> for fs_core::DateTime {
    fn from(d: DateTime) -> Self {
        fs_core::DateTime::new(d.year, d.month, d.day, d.hour, d.minute, d.second)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryInfo {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub created: Option<DateTime>,
    pub modified: Option<DateTime>,
    pub accessed: Option<DateTime>,
}

impl From<fs_core::EntryInfo> for EntryInfo {
    fn from(e: fs_core::EntryInfo) -> Self {
        EntryInfo {
            name: e.name,
            is_dir: e.is_dir,
            size: e.size,
            created: e.created.map(Into::into),
            modified: e.modified.map(Into::into),
            accessed: e.accessed.map(Into::into),
        }
    }
}

/// A byte range; `end` is exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Range {
    pub start: usize,
    pub end: usize,
}

impl From<std::ops::Range<usize>> for Range {
    fn from(r: std::ops::Range<usize>) -> Self {
        Range {
            start: r.start,
            end: r.end,
        }
    }
}

/// A sector range; `end` is exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Range64 {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByteChange {
    pub offset: usize,
    #[serde(with = "serde_bytes")]
    pub before: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub after: Vec<u8>,
}

impl From<&fs_core::ByteChange> for ByteChange {
    fn from(c: &fs_core::ByteChange) -> Self {
        ByteChange {
            offset: c.offset,
            before: c.before.clone(),
            after: c.after.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EventRecord {
    pub kind: String,
    /// The event's `Display` text, e.g. `allocated cluster 5`.
    pub text: String,
    pub region: Option<Range>,
}

impl EventRecord {
    pub fn from_event(e: &dyn Event) -> Self {
        EventRecord {
            kind: e.kind().to_string(),
            text: e.to_string(),
            region: e.region().map(Into::into),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpRecord {
    pub op: String,
    pub changes: Vec<ByteChange>,
    pub events: Vec<EventRecord>,
}

impl From<&fs_core::OpRecord> for OpRecord {
    fn from(r: &fs_core::OpRecord) -> Self {
        OpRecord {
            op: r.op.clone(),
            changes: r.changes.iter().map(Into::into).collect(),
            events: r
                .events
                .iter()
                .map(|e| EventRecord::from_event(e.as_ref()))
                .collect(),
        }
    }
}

pub fn region_kind_name(kind: RegionKind) -> &'static str {
    match kind {
        RegionKind::Boot => "boot",
        RegionKind::Metadata => "metadata",
        RegionKind::AllocationTable => "allocationTable",
        RegionKind::Directory => "directory",
        RegionKind::Data => "data",
        RegionKind::Journal => "journal",
        RegionKind::Reserved => "reserved",
        RegionKind::Other => "other",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub name: String,
    pub sectors: Range64,
    pub kind: String,
}

impl From<fs_core::Region> for Region {
    fn from(r: fs_core::Region) -> Self {
        Region {
            name: r.name,
            sectors: Range64 {
                start: r.sectors.start,
                end: r.sectors.end,
            },
            kind: region_kind_name(r.kind).to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Annotation {
    pub range: Range,
    pub label: String,
    pub value: String,
}

impl From<fs_core::Annotation> for Annotation {
    fn from(a: fs_core::Annotation) -> Self {
        Annotation {
            range: a.range.into(),
            label: a.label,
            value: a.value,
        }
    }
}

/// Every field optional; absent fields take `fat::FormatOptions::default()`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct FormatOptions {
    pub bytes_per_sector: Option<u16>,
    pub sectors_per_cluster: Option<u8>,
    pub total_sectors: Option<u32>,
    pub fat_count: Option<u8>,
    pub root_entries: Option<u16>,
    pub reserved_sectors: Option<u16>,
    pub volume_label: Option<String>,
    pub volume_id: Option<u32>,
    pub enforce_fat16_range: Option<bool>,
}

impl FormatOptions {
    /// The camelCase keys `#[serde(deny_unknown_fields)]` is meant to police.
    /// `serde-wasm-bindgen` deserializes structs from JS objects by looking up
    /// each known field by name rather than iterating the object's own keys,
    /// so `deny_unknown_fields` never actually sees (and can't reject) an
    /// unrecognized key coming from JS. Callers on that boundary must check
    /// the object's keys against this list themselves before deserializing.
    pub const FIELDS: &'static [&'static str] = &[
        "bytesPerSector",
        "sectorsPerCluster",
        "totalSectors",
        "fatCount",
        "rootEntries",
        "reservedSectors",
        "volumeLabel",
        "volumeId",
        "enforceFat16Range",
    ];
}

/// Space-pad or truncate to the 11-byte on-disk label.
pub fn pad_label(label: &str) -> [u8; 11] {
    let mut out = [b' '; 11];
    for (i, b) in label.bytes().take(11).enumerate() {
        out[i] = b;
    }
    out
}

impl From<FormatOptions> for fat::FormatOptions {
    fn from(o: FormatOptions) -> Self {
        let d = fat::FormatOptions::default();
        fat::FormatOptions {
            bytes_per_sector: o.bytes_per_sector.unwrap_or(d.bytes_per_sector),
            sectors_per_cluster: o.sectors_per_cluster.unwrap_or(d.sectors_per_cluster),
            total_sectors: o.total_sectors.unwrap_or(d.total_sectors),
            fat_count: o.fat_count.unwrap_or(d.fat_count),
            root_entries: o.root_entries.unwrap_or(d.root_entries),
            reserved_sectors: o.reserved_sectors.unwrap_or(d.reserved_sectors),
            volume_label: o
                .volume_label
                .map(|l| pad_label(&l))
                .unwrap_or(d.volume_label),
            volume_id: o.volume_id.unwrap_or(d.volume_id),
            enforce_fat16_range: o.enforce_fat16_range.unwrap_or(d.enforce_fat16_range),
        }
    }
}

/// `formatExt2`'s options. Every field optional; absent fields take
/// `ext::ExtFormatOptions::default()` (16,384 blocks, derived inodes per
/// group, empty label, the fixed default UUID).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct ExtFormatOptions {
    pub total_blocks: Option<u32>,
    pub inodes_per_group: Option<u32>,
    pub label: Option<String>,
    pub uuid: Option<String>,
}

impl ExtFormatOptions {
    /// The camelCase keys a JS caller may pass; see `FormatOptions::FIELDS`
    /// for why the boundary checks them by hand.
    pub const FIELDS: &'static [&'static str] = &["totalBlocks", "inodesPerGroup", "label", "uuid"];
}

/// The on-disk UUID from 32 hex digits, bare or hyphenated 8-4-4-4-12
/// (either case). Anything else is an error message for `BadArgument`.
pub fn parse_uuid(text: &str) -> Result<[u8; 16], String> {
    let bad = || format!("uuid \"{text}\" is not 32 hex digits (optionally hyphenated 8-4-4-4-12)");
    let bytes = text.as_bytes();
    let digits: String = if bytes.len() == 36 {
        if [8, 13, 18, 23].iter().any(|&i| bytes[i] != b'-') {
            return Err(bad());
        }
        text.split('-').collect()
    } else {
        text.to_string()
    };
    if digits.len() != 32 || !digits.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(bad());
    }
    let mut out = [0u8; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&digits[2 * i..2 * i + 2], 16).map_err(|_| bad())?;
    }
    Ok(out)
}

/// NUL-pad the label to the 16-byte `s_volume_name`. A longer label is an
/// error message for `BadArgument`, not truncated as FAT's `pad_label` does.
pub fn ext_label(label: &str) -> Result<[u8; 16], String> {
    let bytes = label.as_bytes();
    if bytes.len() > 16 {
        return Err(format!(
            "label \"{label}\" is {} bytes; an ext2 label holds at most 16",
            bytes.len()
        ));
    }
    let mut out = [0u8; 16];
    out[..bytes.len()].copy_from_slice(bytes);
    Ok(out)
}

impl TryFrom<ExtFormatOptions> for ext::ExtFormatOptions {
    type Error = String;

    fn try_from(o: ExtFormatOptions) -> Result<Self, String> {
        let d = ext::ExtFormatOptions::default();
        Ok(ext::ExtFormatOptions {
            total_blocks: o.total_blocks.unwrap_or(d.total_blocks),
            inodes_per_group: o.inodes_per_group.or(d.inodes_per_group),
            label: match o.label {
                Some(l) => ext_label(&l)?,
                None => d.label,
            },
            uuid: match o.uuid {
                Some(u) => parse_uuid(&u)?,
                None => d.uuid,
            },
            journal: None,
        })
    }
}

/// `formatExt3`'s options: `ExtFormatOptions`' four keys plus the
/// journal's. Absent fields take `ext::ExtFormatOptions::ext3()` (mke2fs's
/// journal size for the volume, ordered mode).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Ext3FormatOptions {
    pub total_blocks: Option<u32>,
    pub inodes_per_group: Option<u32>,
    pub label: Option<String>,
    pub uuid: Option<String>,
    pub journal_blocks: Option<u32>,
    pub journal_mode: Option<String>,
}

impl Ext3FormatOptions {
    /// The camelCase keys a JS caller may pass; see `FormatOptions::FIELDS`
    /// for why the boundary checks them by hand.
    pub const FIELDS: &'static [&'static str] = &[
        "totalBlocks",
        "inodesPerGroup",
        "label",
        "uuid",
        "journalBlocks",
        "journalMode",
    ];
}

impl TryFrom<Ext3FormatOptions> for ext::ExtFormatOptions {
    type Error = String;

    fn try_from(o: Ext3FormatOptions) -> Result<Self, String> {
        let mode = match o.journal_mode {
            Some(m) => ext::JournalMode::parse(&m)
                .ok_or_else(|| format!("journal mode \"{m}\" is not \"ordered\" or \"data\""))?,
            None => ext::JournalMode::default(),
        };
        let ext2 = ext::ExtFormatOptions::try_from(ExtFormatOptions {
            total_blocks: o.total_blocks,
            inodes_per_group: o.inodes_per_group,
            label: o.label,
            uuid: o.uuid,
        })?;
        Ok(ext::ExtFormatOptions {
            journal: Some(ext::JournalOptions {
                blocks: o.journal_blocks,
                mode,
            }),
            ..ext2
        })
    }
}

/// `journalInfo()`: the journal's shape and state (spec section 7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalInfo {
    pub inode: u32,
    pub maxlen: u32,
    pub first_block: u32,
    pub sequence: u32,
    pub start: u32,
    pub head: u32,
    /// `"ordered"` or `"data"`.
    pub mode: String,
    pub needs_recovery: bool,
    pub max_transaction: u32,
}

impl From<ext::JournalInfo> for JournalInfo {
    fn from(i: ext::JournalInfo) -> Self {
        JournalInfo {
            inode: i.inode,
            maxlen: i.maxlen,
            first_block: i.first_block,
            sequence: i.sequence,
            start: i.start,
            head: i.head,
            mode: i.mode.as_str().to_string(),
            needs_recovery: i.needs_recovery,
            max_transaction: i.max_transaction,
        }
    }
}

/// One entry of `journalBlocks()`. `kind` is `superblock`, `descriptor`,
/// `copy`, `commit`, `revoke`, or `unused`; `home` and `escaped` are set
/// only for a `copy` and are `null` otherwise, like `tid` on a block that
/// belongs to no transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalBlock {
    pub index: u32,
    pub block: u32,
    pub kind: String,
    pub tid: Option<u32>,
    pub home: Option<u32>,
    pub escaped: Option<bool>,
    pub stale: bool,
}

impl From<ext::JournalBlock> for JournalBlock {
    fn from(b: ext::JournalBlock) -> Self {
        use ext::JournalBlockKind as K;
        let (kind, home, escaped) = match b.kind {
            K::Superblock => ("superblock", None, None),
            K::Descriptor => ("descriptor", None, None),
            K::Copy { home, escaped } => ("copy", Some(home), Some(escaped)),
            K::Commit => ("commit", None, None),
            K::Revoke => ("revoke", None, None),
            K::Unused => ("unused", None, None),
        };
        JournalBlock {
            index: b.index,
            block: b.block,
            kind: kind.to_string(),
            tid: b.tid,
            home,
            escaped,
            stale: b.stale,
        }
    }
}

/// One block group of `extGeometry()`: where its structures live (from
/// `ext::GroupLayout`) and its counters (from the primary group
/// descriptor). `superblockBlock` and `descriptorsBlock` are `null` in a
/// group `sparse_super` gives no backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtGroup {
    pub index: u32,
    pub first_block: u32,
    pub block_count: u32,
    pub superblock_block: Option<u32>,
    pub descriptors_block: Option<u32>,
    pub block_bitmap: u32,
    pub inode_bitmap: u32,
    pub inode_table: u32,
    pub first_data: u32,
    pub free_blocks: u16,
    pub free_inodes: u16,
    pub used_dirs: u16,
}

/// `extGeometry()`: the numbers every ext panel derives from the superblock,
/// and each group's layout and counters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtGeometry {
    pub block_size: u32,
    pub total_blocks: u32,
    pub first_data_block: u32,
    pub blocks_per_group: u32,
    pub inodes_per_group: u32,
    pub inodes_count: u32,
    pub inode_size: u16,
    pub inode_table_blocks: u32,
    pub descriptor_blocks: u32,
    pub groups: Vec<ExtGroup>,
}

impl ExtGeometry {
    /// Merge the layout with the primary descriptors, group by group.
    pub fn new(geo: &ext::Geometry, gds: &[ext::GroupDescriptor]) -> Self {
        let groups = geo
            .groups_layout
            .iter()
            .map(|g| {
                let gd = gds.get(g.index as usize).copied().unwrap_or_default();
                ExtGroup {
                    index: g.index,
                    first_block: g.first_block,
                    block_count: g.block_count,
                    superblock_block: g.superblock_block,
                    descriptors_block: g.descriptors_block,
                    block_bitmap: g.block_bitmap,
                    inode_bitmap: g.inode_bitmap,
                    inode_table: g.inode_table,
                    first_data: g.first_data,
                    free_blocks: gd.free_blocks_count,
                    free_inodes: gd.free_inodes_count,
                    used_dirs: gd.used_dirs_count,
                }
            })
            .collect();
        ExtGeometry {
            block_size: geo.block_size,
            total_blocks: geo.total_blocks,
            first_data_block: geo.first_data_block,
            blocks_per_group: geo.blocks_per_group,
            inodes_per_group: geo.inodes_per_group,
            inodes_count: geo.inodes_count,
            inode_size: ext::INODE_SIZE,
            inode_table_blocks: geo.inode_table_blocks,
            descriptor_blocks: geo.descriptor_blocks,
            groups,
        }
    }
}

/// The 16 UUID bytes as lower-case hex, hyphenated 8-4-4-4-12.
pub fn uuid_text(bytes: &[u8; 16]) -> String {
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// The bytes up to the first NUL, as lossy UTF-8.
pub fn nul_trimmed(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// `extSuperblock()`: the primary superblock's fields the explorer shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtSuperblock {
    pub inodes_count: u32,
    pub blocks_count: u32,
    pub reserved_blocks: u32,
    pub free_blocks: u32,
    pub free_inodes: u32,
    pub first_data_block: u32,
    pub log_block_size: u32,
    pub blocks_per_group: u32,
    pub inodes_per_group: u32,
    pub magic: u16,
    pub state: u16,
    pub rev_level: u32,
    pub first_ino: u32,
    pub inode_size: u16,
    pub feature_compat: u32,
    pub feature_incompat: u32,
    pub feature_ro_compat: u32,
    pub uuid: String,
    pub label: String,
    pub journal_inum: u32,
    pub default_mount_opts: u32,
    pub mtime: u32,
    pub wtime: u32,
    pub mnt_count: u16,
}

impl From<&ext::Superblock> for ExtSuperblock {
    fn from(sb: &ext::Superblock) -> Self {
        ExtSuperblock {
            inodes_count: sb.inodes_count,
            blocks_count: sb.blocks_count,
            reserved_blocks: sb.r_blocks_count,
            free_blocks: sb.free_blocks_count,
            free_inodes: sb.free_inodes_count,
            first_data_block: sb.first_data_block,
            log_block_size: sb.log_block_size,
            blocks_per_group: sb.blocks_per_group,
            inodes_per_group: sb.inodes_per_group,
            magic: sb.magic,
            state: sb.state,
            rev_level: sb.rev_level,
            first_ino: sb.first_ino,
            inode_size: sb.inode_size,
            feature_compat: sb.feature_compat,
            feature_incompat: sb.feature_incompat,
            feature_ro_compat: sb.feature_ro_compat,
            uuid: uuid_text(&sb.uuid),
            label: nul_trimmed(&sb.volume_name),
            journal_inum: sb.journal_inum,
            default_mount_opts: sb.default_mount_opts,
            mtime: sb.mtime,
            wtime: sb.wtime,
            mnt_count: sb.mnt_count,
        }
    }
}

/// `ext::BlockRole` as the string `ExtBlockOwner.role` carries.
pub fn block_role_name(role: ext::BlockRole) -> &'static str {
    match role {
        ext::BlockRole::Data => "data",
        ext::BlockRole::Directory => "directory",
        ext::BlockRole::Indirect => "indirect",
        ext::BlockRole::Journal => "journal",
    }
}

/// One row of `blockOwners()`: the inode and path a block belongs to and
/// what it holds (`data`, `directory`, `indirect`, or `journal`). The
/// journal's rows, its pointer blocks included, carry the path `<journal>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtBlockOwner {
    pub block: u32,
    pub inode: u32,
    pub path: String,
    pub role: String,
}

/// Sorted by block (BTreeMap order).
pub fn ext_owners_to_list(map: &BTreeMap<u32, ext::BlockOwner>) -> Vec<ExtBlockOwner> {
    map.iter()
        .map(|(&block, o)| ExtBlockOwner {
            block,
            inode: o.inode,
            path: o.path.clone(),
            role: block_role_name(o.role).to_string(),
        })
        .collect()
}

/// Where an inode's 128 bytes live: the inode-table block and the absolute
/// byte offset of the slot on the disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtInodeSlot {
    pub block: u32,
    pub offset: u64,
}

/// `extInode(ino)`: the decoded inode and its slot. `blocks` is `i_blocks`,
/// in 512-byte units; `block` is all 15 pointers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtInode {
    pub ino: u32,
    pub mode: u16,
    pub uid: u16,
    pub gid: u16,
    pub size: u32,
    pub links: u16,
    pub blocks: u32,
    pub flags: u32,
    pub atime: u32,
    pub ctime: u32,
    pub mtime: u32,
    pub dtime: u32,
    pub block: Vec<u32>,
    pub slot: ExtInodeSlot,
}

impl ExtInode {
    /// `ino` must be in `1..=geo.inodes_count` (`ExtFs::inode` checks it).
    pub fn new(ino: u32, inode: &ext::Inode, geo: &ext::Geometry) -> Self {
        let (block, in_block) = geo.inode_location(ino);
        ExtInode {
            ino,
            mode: inode.mode,
            uid: inode.uid,
            gid: inode.gid,
            size: inode.size,
            links: inode.links_count,
            blocks: inode.blocks,
            flags: inode.flags,
            atime: inode.atime,
            ctime: inode.ctime,
            mtime: inode.mtime,
            dtime: inode.dtime,
            block: inode.block.to_vec(),
            slot: ExtInodeSlot {
                block,
                offset: u64::from(block) * u64::from(geo.block_size) + in_block as u64,
            },
        }
    }
}

/// One record of `dirEntries(path)`: the block holding it, its absolute
/// byte offset on the disk, and its fields; `name` is lossy UTF-8.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtDirEntry {
    pub block: u32,
    pub offset: u64,
    pub inode: u32,
    pub rec_len: u16,
    pub name_len: u8,
    pub file_type: u8,
    pub name: String,
}

impl ExtDirEntry {
    /// The entry `ExtFs::dir_entries` found at `offset` bytes into `block`.
    pub fn new(block: u32, offset: usize, e: &ext::DirEntry) -> Self {
        ExtDirEntry {
            block,
            offset: u64::from(block) * u64::from(ext::BLOCK_SIZE) + offset as u64,
            inode: e.inode,
            rec_len: e.rec_len,
            name_len: e.name_len,
            file_type: e.file_type,
            name: String::from_utf8_lossy(&e.name).into_owned(),
        }
    }
}

/// One pointer block of `fileBlocks(path)`: level 1 is a single-indirect
/// block or a second-level block under the double, level 2 the
/// double-indirect block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtIndirectBlock {
    pub block: u32,
    pub level: u8,
}

/// `fileBlocks(path)`: the data blocks in logical order and the pointer
/// blocks in the order they are referenced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtFileBlocks {
    pub data: Vec<u32>,
    pub indirect: Vec<ExtIndirectBlock>,
}

impl ExtFileBlocks {
    /// `blockmap::file_blocks` and `blockmap::indirect_blocks` of `inode`.
    pub fn new(disk: &fs_core::Disk, inode: &ext::Inode) -> Self {
        ExtFileBlocks {
            data: ext::blockmap::file_blocks(disk, inode),
            indirect: ext::blockmap::indirect_blocks(disk, inode)
                .into_iter()
                .map(|(block, level)| ExtIndirectBlock { block, level })
                .collect(),
        }
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim_end().to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootSector {
    pub oem_name: String,
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub fat_count: u8,
    pub root_entries: u16,
    pub total_sectors: u32,
    pub media: u8,
    pub sectors_per_fat: u16,
    pub sectors_per_track: u16,
    pub heads: u16,
    pub hidden_sectors: u32,
    pub drive_number: u8,
    pub boot_signature: u8,
    pub volume_id: u32,
    pub volume_label: String,
    pub fs_type: String,
}

impl From<&fat::BootSector> for BootSector {
    fn from(b: &fat::BootSector) -> Self {
        BootSector {
            oem_name: text(&b.oem_name),
            bytes_per_sector: b.bytes_per_sector,
            sectors_per_cluster: b.sectors_per_cluster,
            reserved_sectors: b.reserved_sectors,
            fat_count: b.fat_count,
            root_entries: b.root_entries,
            total_sectors: b.total_sectors(),
            media: b.media,
            sectors_per_fat: b.sectors_per_fat,
            sectors_per_track: b.sectors_per_track,
            heads: b.heads,
            hidden_sectors: b.hidden_sectors,
            drive_number: b.drive_number,
            boot_signature: b.boot_signature,
            volume_id: b.volume_id,
            volume_label: text(&b.volume_label),
            fs_type: text(&b.fs_type),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Geometry {
    pub variant: String,
    pub bytes_per_sector: usize,
    pub sectors_per_cluster: usize,
    pub reserved_sectors: u64,
    pub fat_count: u8,
    pub sectors_per_fat: u64,
    pub root_entries: usize,
    pub root_dir_sectors: u64,
    pub first_root_dir_sector: u64,
    pub first_data_sector: u64,
    pub total_sectors: u64,
    pub cluster_count: u32,
}

impl From<&fat::Geometry> for Geometry {
    fn from(g: &fat::Geometry) -> Self {
        Geometry {
            variant: match g.variant {
                FatVariant::Fat16 => "fat16".to_string(),
            },
            bytes_per_sector: g.bytes_per_sector,
            sectors_per_cluster: g.sectors_per_cluster,
            reserved_sectors: g.reserved_sectors,
            fat_count: g.fat_count,
            sectors_per_fat: g.sectors_per_fat,
            root_entries: g.root_entries,
            root_dir_sectors: g.root_dir_sectors,
            first_root_dir_sector: g.first_root_dir_sector,
            first_data_sector: g.first_data_sector,
            total_sectors: g.total_sectors,
            cluster_count: g.cluster_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FatEntry {
    Free,
    Next { cluster: u32 },
    EndOfChain,
    Bad,
    Reserved,
}

impl From<fat::FatEntry> for FatEntry {
    fn from(e: fat::FatEntry) -> Self {
        match e {
            fat::FatEntry::Free => FatEntry::Free,
            fat::FatEntry::Next(c) => FatEntry::Next { cluster: c },
            fat::FatEntry::EndOfChain => FatEntry::EndOfChain,
            fat::FatEntry::Bad => FatEntry::Bad,
            fat::FatEntry::Reserved => FatEntry::Reserved,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RawEntry {
    Free,
    Deleted {
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    Short {
        name: String,
        attr: u8,
        first_cluster: u32,
        size: u32,
        created: Option<DateTime>,
        modified: Option<DateTime>,
        accessed: Option<DateTime>,
        is_dir: bool,
    },
    Lfn {
        order: u8,
        is_last: bool,
        checksum: u8,
        text: String,
    },
}

impl From<fat::RawEntry> for RawEntry {
    fn from(e: fat::RawEntry) -> Self {
        match e {
            fat::RawEntry::Free => RawEntry::Free,
            fat::RawEntry::Deleted { bytes } => RawEntry::Deleted {
                bytes: bytes.to_vec(),
            },
            fat::RawEntry::Short(s) => RawEntry::Short {
                name: s.display_name(),
                attr: s.attr,
                first_cluster: s.first_cluster(),
                size: s.size,
                created: fat::dir_entry::unpack(s.create_date, s.create_time).map(Into::into),
                modified: fat::dir_entry::unpack(s.write_date, s.write_time).map(Into::into),
                accessed: fat::dir_entry::unpack(s.access_date, 0).map(Into::into),
                is_dir: s.is_dir(),
            },
            fat::RawEntry::Lfn(l) => RawEntry::Lfn {
                order: l.order(),
                is_last: l.is_last(),
                checksum: l.checksum,
                text: fat::name::from_ucs2(&l.chars()),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterOwner {
    pub cluster: u32,
    pub path: String,
    pub is_dir: bool,
    pub first_cluster: u32,
}

/// Sorted by cluster (BTreeMap order).
pub fn owners_to_list(map: &BTreeMap<u32, fat::ClusterOwner>) -> Vec<ClusterOwner> {
    map.iter()
        .map(|(&cluster, o)| ClusterOwner {
            cluster,
            path: o.path.clone(),
            is_dir: o.is_dir,
            first_cluster: o.first_cluster,
        })
        .collect()
}

pub fn owners_from_list(list: Vec<ClusterOwner>) -> BTreeMap<u32, fat::ClusterOwner> {
    list.into_iter()
        .map(|o| {
            (
                o.cluster,
                fat::ClusterOwner {
                    path: o.path,
                    is_dir: o.is_dir,
                    first_cluster: o.first_cluster,
                },
            )
        })
        .collect()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use fat::{FatFs, FormatOptions as FatFormat};
    use std::collections::BTreeMap;
    use std::fmt;
    use std::ops::Range as StdRange;

    #[derive(Debug, Clone)]
    struct Dummy;
    impl fmt::Display for Dummy {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "did a thing")
        }
    }
    impl Event for Dummy {
        fn kind(&self) -> &'static str {
            "dummy"
        }
        fn region(&self) -> Option<StdRange<usize>> {
            Some(10..12)
        }
        fn clone_box(&self) -> Box<dyn Event> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn datetime_round_trips() {
        let core = fs_core::DateTime::new(2026, 9, 21, 1, 2, 3);
        let dto = DateTime::from(core);
        assert_eq!(
            dto,
            DateTime {
                year: 2026,
                month: 9,
                day: 21,
                hour: 1,
                minute: 2,
                second: 3
            }
        );
        assert_eq!(fs_core::DateTime::from(dto), core);
    }

    #[test]
    fn entry_info_maps_every_field() {
        let core = fs_core::EntryInfo {
            name: "A.TXT".into(),
            is_dir: false,
            size: 7,
            created: Some(fs_core::DateTime::default()),
            modified: None,
            accessed: None,
        };
        let dto = EntryInfo::from(core);
        assert_eq!(dto.name, "A.TXT");
        assert!(!dto.is_dir);
        assert_eq!(dto.size, 7);
        assert_eq!(
            dto.created,
            Some(DateTime::from(fs_core::DateTime::default()))
        );
        assert_eq!(dto.modified, None);
    }

    #[test]
    fn op_record_maps_changes_and_events() {
        let mut core = fs_core::OpRecord::new("create_file /A");
        core.changes.push(fs_core::ByteChange {
            offset: 5,
            before: vec![0, 0],
            after: vec![1, 2],
        });
        core.events.push(Box::new(Dummy));
        let dto = OpRecord::from(&core);
        assert_eq!(dto.op, "create_file /A");
        assert_eq!(
            dto.changes,
            vec![ByteChange {
                offset: 5,
                before: vec![0, 0],
                after: vec![1, 2]
            }]
        );
        assert_eq!(
            dto.events,
            vec![EventRecord {
                kind: "dummy".into(),
                text: "did a thing".into(),
                region: Some(Range { start: 10, end: 12 })
            }]
        );
    }

    #[test]
    fn region_and_annotation_map() {
        let r = Region::from(fs_core::Region {
            name: "FAT 0".into(),
            sectors: 1..33,
            kind: RegionKind::AllocationTable,
        });
        assert_eq!(
            r,
            Region {
                name: "FAT 0".into(),
                sectors: Range64 { start: 1, end: 33 },
                kind: "allocationTable".into()
            }
        );
        assert_eq!(region_kind_name(RegionKind::Boot), "boot");
        assert_eq!(region_kind_name(RegionKind::Other), "other");
        assert_eq!(region_kind_name(RegionKind::Journal), "journal");
        let a = Annotation::from(fs_core::Annotation {
            range: 11..13,
            label: "bytes per sector".into(),
            value: "512".into(),
        });
        assert_eq!(a.range, Range { start: 11, end: 13 });
        assert_eq!(a.label, "bytes per sector");
    }

    #[test]
    fn serde_attributes_rename_to_camel_case() {
        // serde's derive emits field names at compile time; check them through a
        // minimal Serializer that records struct field keys.
        use serde::ser::{Serialize, SerializeStruct, Serializer};
        struct KeyCollector(Vec<&'static str>);
        struct KeyErr;
        impl std::fmt::Display for KeyErr {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "err")
            }
        }
        impl std::fmt::Debug for KeyErr {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "err")
            }
        }
        impl std::error::Error for KeyErr {}
        impl serde::ser::Error for KeyErr {
            fn custom<T: std::fmt::Display>(_: T) -> Self {
                KeyErr
            }
        }
        impl SerializeStruct for &mut KeyCollector {
            type Ok = ();
            type Error = KeyErr;
            fn serialize_field<T: ?Sized + Serialize>(
                &mut self,
                key: &'static str,
                _: &T,
            ) -> Result<(), KeyErr> {
                self.0.push(key);
                Ok(())
            }
            fn end(self) -> Result<(), KeyErr> {
                Ok(())
            }
        }
        macro_rules! unsupported {
            ($($name:ident: $t:ty),*) => { $(fn $name(self, _: $t) -> Result<(), KeyErr> { Err(KeyErr) })* };
        }
        impl Serializer for &mut KeyCollector {
            type Ok = ();
            type Error = KeyErr;
            type SerializeSeq = serde::ser::Impossible<(), KeyErr>;
            type SerializeTuple = serde::ser::Impossible<(), KeyErr>;
            type SerializeTupleStruct = serde::ser::Impossible<(), KeyErr>;
            type SerializeTupleVariant = serde::ser::Impossible<(), KeyErr>;
            type SerializeMap = serde::ser::Impossible<(), KeyErr>;
            type SerializeStruct = Self;
            type SerializeStructVariant = serde::ser::Impossible<(), KeyErr>;
            unsupported!(serialize_bool: bool, serialize_i8: i8, serialize_i16: i16, serialize_i32: i32, serialize_i64: i64,
                serialize_u8: u8, serialize_u16: u16, serialize_u32: u32, serialize_u64: u64, serialize_f32: f32,
                serialize_f64: f64, serialize_char: char, serialize_str: &str, serialize_bytes: &[u8]);
            fn serialize_none(self) -> Result<(), KeyErr> {
                Err(KeyErr)
            }
            fn serialize_some<T: ?Sized + Serialize>(self, _: &T) -> Result<(), KeyErr> {
                Err(KeyErr)
            }
            fn serialize_unit(self) -> Result<(), KeyErr> {
                Err(KeyErr)
            }
            fn serialize_unit_struct(self, _: &'static str) -> Result<(), KeyErr> {
                Err(KeyErr)
            }
            fn serialize_unit_variant(
                self,
                _: &'static str,
                _: u32,
                _: &'static str,
            ) -> Result<(), KeyErr> {
                Err(KeyErr)
            }
            fn serialize_newtype_struct<T: ?Sized + Serialize>(
                self,
                _: &'static str,
                _: &T,
            ) -> Result<(), KeyErr> {
                Err(KeyErr)
            }
            fn serialize_newtype_variant<T: ?Sized + Serialize>(
                self,
                _: &'static str,
                _: u32,
                _: &'static str,
                _: &T,
            ) -> Result<(), KeyErr> {
                Err(KeyErr)
            }
            fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, KeyErr> {
                Err(KeyErr)
            }
            fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, KeyErr> {
                Err(KeyErr)
            }
            fn serialize_tuple_struct(
                self,
                _: &'static str,
                _: usize,
            ) -> Result<Self::SerializeTupleStruct, KeyErr> {
                Err(KeyErr)
            }
            fn serialize_tuple_variant(
                self,
                _: &'static str,
                _: u32,
                _: &'static str,
                _: usize,
            ) -> Result<Self::SerializeTupleVariant, KeyErr> {
                Err(KeyErr)
            }
            fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, KeyErr> {
                Err(KeyErr)
            }
            fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, KeyErr> {
                Ok(self)
            }
            fn serialize_struct_variant(
                self,
                _: &'static str,
                _: u32,
                _: &'static str,
                _: usize,
            ) -> Result<Self::SerializeStructVariant, KeyErr> {
                Err(KeyErr)
            }
        }
        let dto = EntryInfo {
            name: "x".into(),
            is_dir: true,
            size: 0,
            created: None,
            modified: None,
            accessed: None,
        };
        let mut keys = KeyCollector(Vec::new());
        dto.serialize(&mut keys).unwrap();
        assert_eq!(
            keys.0,
            vec!["name", "isDir", "size", "created", "modified", "accessed"]
        );
    }

    #[test]
    fn format_options_default_to_the_core_defaults() {
        let dto = FormatOptions::default();
        assert_eq!(FatFormat::from(dto), FatFormat::default());
        let dto = FormatOptions {
            total_sectors: Some(8192),
            sectors_per_cluster: Some(1),
            volume_label: Some("teach".into()),
            enforce_fat16_range: Some(false),
            ..Default::default()
        };
        let core = FatFormat::from(dto);
        assert_eq!(core.total_sectors, 8192);
        assert_eq!(core.sectors_per_cluster, 1);
        assert_eq!(&core.volume_label, b"teach      ");
        assert!(!core.enforce_fat16_range);
        assert_eq!(core.bytes_per_sector, 512);
    }

    #[test]
    fn labels_are_padded_and_truncated() {
        assert_eq!(pad_label(""), *b"           ");
        assert_eq!(pad_label("A"), *b"A          ");
        assert_eq!(pad_label("TWELVE CHARS"), *b"TWELVE CHAR");
    }

    #[test]
    fn ext_format_options_default_to_the_core_defaults() {
        let core = ext::ExtFormatOptions::try_from(ExtFormatOptions::default()).unwrap();
        assert_eq!(core, ext::ExtFormatOptions::default());
        let dto = ExtFormatOptions {
            total_blocks: Some(8192),
            inodes_per_group: Some(64),
            label: Some("teach".into()),
            uuid: Some("01234567-89AB-cdef-0123-456789abcdef".into()),
        };
        let core = ext::ExtFormatOptions::try_from(dto).unwrap();
        assert_eq!(core.total_blocks, 8192);
        assert_eq!(core.inodes_per_group, Some(64));
        assert_eq!(&core.label, b"teach\0\0\0\0\0\0\0\0\0\0\0");
        assert_eq!(
            core.uuid,
            [
                0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab,
                0xcd, 0xef
            ]
        );
        assert_eq!(
            ExtFormatOptions::FIELDS,
            &["totalBlocks", "inodesPerGroup", "label", "uuid"]
        );
    }

    #[test]
    fn ext3_format_options_add_the_journal_to_the_ext2_keys() {
        let core = ext::ExtFormatOptions::try_from(Ext3FormatOptions::default()).unwrap();
        assert_eq!(core, ext::ExtFormatOptions::ext3());
        let dto = Ext3FormatOptions {
            total_blocks: Some(8192),
            inodes_per_group: Some(64),
            label: Some("journal".into()),
            uuid: Some("0123456789abcdef0123456789abcdef".into()),
            journal_blocks: Some(2048),
            journal_mode: Some("data".into()),
        };
        let core = ext::ExtFormatOptions::try_from(dto).unwrap();
        assert_eq!(core.total_blocks, 8192);
        assert_eq!(core.inodes_per_group, Some(64));
        assert_eq!(&core.label, b"journal\0\0\0\0\0\0\0\0\0");
        assert_eq!(core.uuid[..2], [0x01, 0x23]);
        assert_eq!(
            core.journal,
            Some(ext::JournalOptions {
                blocks: Some(2048),
                mode: ext::JournalMode::Data
            })
        );
        let ordered = Ext3FormatOptions {
            journal_mode: Some("ordered".into()),
            ..Default::default()
        };
        assert_eq!(
            ext::ExtFormatOptions::try_from(ordered).unwrap().journal,
            Some(ext::JournalOptions::default())
        );
        assert_eq!(
            Ext3FormatOptions::FIELDS,
            &[
                "totalBlocks",
                "inodesPerGroup",
                "label",
                "uuid",
                "journalBlocks",
                "journalMode"
            ]
        );
        // The ext2 options never carry a journal.
        let ext2 = ext::ExtFormatOptions::try_from(ExtFormatOptions::default()).unwrap();
        assert_eq!(ext2.journal, None);
    }

    #[test]
    fn ext3_format_options_refuse_another_mode_and_keep_the_ext2_checks() {
        for mode in ["writeback", "Ordered", "journal_data", ""] {
            let err = ext::ExtFormatOptions::try_from(Ext3FormatOptions {
                journal_mode: Some(mode.into()),
                ..Default::default()
            })
            .unwrap_err();
            assert_eq!(
                err,
                format!("journal mode \"{mode}\" is not \"ordered\" or \"data\"")
            );
        }
        let err = ext::ExtFormatOptions::try_from(Ext3FormatOptions {
            uuid: Some("xyz".into()),
            ..Default::default()
        })
        .unwrap_err();
        assert_eq!(
            err,
            "uuid \"xyz\" is not 32 hex digits (optionally hyphenated 8-4-4-4-12)"
        );
        assert!(ext::ExtFormatOptions::try_from(Ext3FormatOptions {
            label: Some("x".repeat(17)),
            ..Default::default()
        })
        .is_err());
    }

    #[test]
    fn journal_info_and_blocks_map_a_formatted_journal() {
        let fs = ext::ExtFs::format(ext::ExtFormatOptions::ext3()).unwrap();
        assert_eq!(
            JournalInfo::from(fs.journal_info().unwrap()),
            JournalInfo {
                inode: 8,
                maxlen: 1024,
                first_block: 82,
                sequence: 1,
                start: 0,
                head: 1,
                mode: "ordered".into(),
                needs_recovery: false,
                max_transaction: 256,
            }
        );
        let blocks: Vec<JournalBlock> = fs.journal_blocks().into_iter().map(Into::into).collect();
        assert_eq!(blocks.len(), 1024);
        assert_eq!(
            blocks[0],
            JournalBlock {
                index: 0,
                block: 82,
                kind: "superblock".into(),
                tid: None,
                home: None,
                escaped: None,
                stale: false,
            }
        );
        assert!(blocks[1..].iter().all(|b| b.kind == "unused"));
        assert_eq!(blocks[1023].block, 1105);
    }

    #[test]
    fn journal_block_kinds_are_strings_and_only_a_copy_carries_home() {
        use ext::JournalBlockKind as K;
        let core = |kind, tid| ext::JournalBlock {
            index: 3,
            block: 85,
            kind,
            tid,
            stale: true,
        };
        assert_eq!(
            JournalBlock::from(core(
                K::Copy {
                    home: 69,
                    escaped: true
                },
                Some(2)
            )),
            JournalBlock {
                index: 3,
                block: 85,
                kind: "copy".into(),
                tid: Some(2),
                home: Some(69),
                escaped: Some(true),
                stale: true,
            }
        );
        for (kind, name) in [
            (K::Superblock, "superblock"),
            (K::Descriptor, "descriptor"),
            (K::Commit, "commit"),
            (K::Revoke, "revoke"),
            (K::Unused, "unused"),
        ] {
            let dto = JournalBlock::from(core(kind, Some(7)));
            assert_eq!(dto.kind, name);
            assert_eq!(dto.tid, Some(7));
            assert_eq!((dto.home, dto.escaped), (None, None));
        }
    }

    #[test]
    fn ext_uuids_parse_bare_or_hyphenated_and_nothing_else() {
        assert_eq!(
            parse_uuid("e2f5ee00-2026-4923-8000-000000000001"),
            Ok(ext::DEFAULT_UUID)
        );
        assert_eq!(
            parse_uuid("E2F5EE00202649238000000000000001"),
            Ok(ext::DEFAULT_UUID)
        );
        for bad in [
            "",
            "not-a-uuid",
            "e2f5ee0020264923800000000000000",
            "e2f5ee00202649238000000000000001ff",
            "e2f5ee0020264923800000000000000g",
            "e2f5ee002-026-4923-8000-000000000001",
            "e2f5ee00-2026-4923-8000-00000000-001",
            "+2f5ee00202649238000000000000001",
            "e2f5ee00-2026-4923-8000-0000000000é",
        ] {
            let err = parse_uuid(bad).unwrap_err();
            assert!(err.starts_with("uuid \""), "{bad}: {err}");
        }
        let err = ext::ExtFormatOptions::try_from(ExtFormatOptions {
            uuid: Some("xyz".into()),
            ..Default::default()
        })
        .unwrap_err();
        assert_eq!(
            err,
            "uuid \"xyz\" is not 32 hex digits (optionally hyphenated 8-4-4-4-12)"
        );
    }

    #[test]
    fn ext_labels_hold_sixteen_bytes() {
        assert_eq!(ext_label(""), Ok([0u8; 16]));
        assert_eq!(ext_label("sixteen-bytes!!!"), Ok(*b"sixteen-bytes!!!"));
        assert_eq!(
            ext_label("seventeen-bytes!!"),
            Err("label \"seventeen-bytes!!\" is 17 bytes; an ext2 label holds at most 16".into())
        );
        // Bytes, not characters: nine two-byte characters are 18 bytes.
        assert!(ext_label("ééééééééé").is_err());
        assert!(ext::ExtFormatOptions::try_from(ExtFormatOptions {
            label: Some("x".repeat(17)),
            ..Default::default()
        })
        .is_err());
    }

    #[test]
    fn boot_sector_and_geometry_map() {
        let fs = FatFs::format(FatFormat::default()).unwrap();
        let bs = BootSector::from(fs.boot_sector());
        assert_eq!(bs.oem_name, "FAT16EMU");
        assert_eq!(bs.bytes_per_sector, 512);
        assert_eq!(bs.total_sectors, 32768);
        assert_eq!(bs.fs_type, "FAT16");
        assert_eq!(bs.volume_label, "NO NAME");
        let g = Geometry::from(fs.geometry());
        assert_eq!(g.variant, "fat16");
        assert_eq!(g.cluster_count, 8167);
        assert_eq!(g.first_data_sector, 97);
    }

    #[test]
    fn fat_entry_and_raw_entry_map() {
        assert_eq!(FatEntry::from(fat::FatEntry::Free), FatEntry::Free);
        assert_eq!(
            FatEntry::from(fat::FatEntry::Next(9)),
            FatEntry::Next { cluster: 9 }
        );
        assert_eq!(
            FatEntry::from(fat::FatEntry::EndOfChain),
            FatEntry::EndOfChain
        );
        let mut fs = FatFs::format(FatFormat::default()).unwrap();
        fs.create_file("/My File.txt", b"abc").unwrap();
        fs.create_file("/B", b"").unwrap();
        fs.delete_file("/B").unwrap();
        let raw: Vec<RawEntry> = fs
            .raw_dir_entries("/")
            .unwrap()
            .into_iter()
            .map(Into::into)
            .collect();
        assert!(
            matches!(&raw[0], RawEntry::Lfn { order: 1, is_last: true, text, .. } if text == "My File.txt")
        );
        assert!(
            matches!(&raw[1], RawEntry::Short { name, size: 3, is_dir: false, first_cluster: 2, .. } if name == "MYFILE~1.TXT")
        );
        assert!(matches!(&raw[2], RawEntry::Deleted { bytes } if bytes.len() == 32));
        assert_eq!(raw[3], RawEntry::Free);
    }

    #[test]
    fn cluster_owner_lists_round_trip_sorted() {
        let mut fs = FatFs::format(FatFormat::default()).unwrap();
        fs.create_dir("/D").unwrap();
        fs.create_file("/D/F", &[0u8; 5000]).unwrap();
        let map = fs.cluster_owners();
        let list = owners_to_list(&map);
        assert_eq!(list.len(), 4);
        assert!(list.windows(2).all(|w| w[0].cluster < w[1].cluster));
        assert_eq!(
            list[0],
            ClusterOwner {
                cluster: 2,
                path: "/D".into(),
                is_dir: true,
                first_cluster: 2
            }
        );
        assert_eq!(list[1].path, "/D/F");
        assert_eq!(list[1].first_cluster, 3);
        let back: BTreeMap<u32, fat::ClusterOwner> = owners_from_list(list);
        assert_eq!(back, map);
    }

    #[test]
    fn ext_uuids_print_hyphenated_and_labels_stop_at_the_first_nul() {
        assert_eq!(
            uuid_text(&ext::DEFAULT_UUID),
            "e2f5ee00-2026-4923-8000-000000000001"
        );
        assert_eq!(
            uuid_text(&[0xAB; 16]),
            "abababab-abab-abab-abab-abababababab"
        );
        assert_eq!(nul_trimmed(b"teach\0\0\0"), "teach");
        assert_eq!(nul_trimmed(b"a\0b"), "a");
        assert_eq!(nul_trimmed(b""), "");
        assert_eq!(nul_trimmed(b"sixteen-bytes!!!"), "sixteen-bytes!!!");
        assert_eq!(nul_trimmed(&[0xFF, b'x', 0]), "\u{FFFD}x");
    }

    #[test]
    fn ext_geometry_and_superblock_map_the_default_disk() {
        let fs = ext::ExtFs::format(ext::ExtFormatOptions::ext3()).unwrap();
        let geo = ExtGeometry::new(fs.geometry(), fs.group_descriptors());
        assert_eq!(
            (geo.total_blocks, geo.inodes_count, geo.inode_size),
            (16384, 1024, 128)
        );
        assert_eq!(
            geo.groups[0],
            ExtGroup {
                index: 0,
                first_block: 1,
                block_count: 8192,
                superblock_block: Some(1),
                descriptors_block: Some(2),
                block_bitmap: 3,
                inode_bitmap: 4,
                inode_table: 5,
                first_data: 69,
                free_blocks: 7082,
                free_inodes: 501,
                used_dirs: 2,
            }
        );
        let sb = ExtSuperblock::from(fs.superblock());
        assert_eq!((sb.free_blocks, sb.free_inodes), (15205, 1013));
        assert_eq!(sb.uuid, "e2f5ee00-2026-4923-8000-000000000001");
        assert_eq!((sb.label.as_str(), sb.journal_inum), ("", 8));
    }

    #[test]
    fn ext_owner_rows_carry_role_strings_in_block_order() {
        assert_eq!(block_role_name(ext::BlockRole::Data), "data");
        assert_eq!(block_role_name(ext::BlockRole::Directory), "directory");
        assert_eq!(block_role_name(ext::BlockRole::Indirect), "indirect");
        assert_eq!(block_role_name(ext::BlockRole::Journal), "journal");
        let fs = ext::ExtFs::format(ext::ExtFormatOptions::ext3()).unwrap();
        let list = ext_owners_to_list(&fs.block_owners());
        assert_eq!(list.len(), 13 + 1024 + 5);
        assert!(list.windows(2).all(|w| w[0].block < w[1].block));
        assert_eq!(
            list[0],
            ExtBlockOwner {
                block: 69,
                inode: 2,
                path: "/".into(),
                role: "directory".into()
            }
        );
        assert_eq!(
            list.last().unwrap(),
            &ExtBlockOwner {
                block: 1110,
                inode: 8,
                path: "<journal>".into(),
                role: "indirect".into()
            }
        );
    }

    #[test]
    fn ext_inode_slot_offsets_are_absolute() {
        let fs = ext::ExtFs::format(ext::ExtFormatOptions::ext3()).unwrap();
        let root = ExtInode::new(2, &fs.inode(2).unwrap(), fs.geometry());
        assert_eq!(
            root.slot,
            ExtInodeSlot {
                block: 5,
                offset: 5 * 1024 + 0x80
            }
        );
        assert_eq!((root.mode, root.links, root.block.len()), (0o40755, 3, 15));
        assert_eq!(root.block[0], 69);
        let lf = ExtInode::new(11, &fs.inode(11).unwrap(), fs.geometry());
        assert_eq!(lf.slot.offset, 6 * 1024 + 0x100);
        assert_eq!((lf.size, lf.blocks), (12288, 24));
    }

    #[test]
    fn ext_dir_entries_and_file_blocks_map_the_default_disk() {
        let mut fs = ext::ExtFs::format(ext::ExtFormatOptions::ext3()).unwrap();
        fs.create_file("/bigger.txt", &[b'x'; 13312]).unwrap();
        let rows: Vec<ExtDirEntry> = fs
            .dir_entries("/")
            .unwrap()
            .iter()
            .map(|(block, offset, e)| ExtDirEntry::new(*block, *offset, e))
            .collect();
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows[3],
            ExtDirEntry {
                block: 69,
                offset: 69 * 1024 + 44,
                inode: 12,
                rec_len: 980,
                name_len: 10,
                file_type: 1,
                name: "bigger.txt".into(),
            }
        );
        let inode = fs.inode(fs.lookup("/bigger.txt").unwrap()).unwrap();
        let blocks = ExtFileBlocks::new(fs.disk(), &inode);
        assert_eq!(blocks.data, (1111..=1123).collect::<Vec<u32>>());
        assert_eq!(
            blocks.indirect,
            vec![ExtIndirectBlock {
                block: 1124,
                level: 1
            }]
        );
    }
}
