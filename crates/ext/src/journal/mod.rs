//! The ext3 journal (JBD2 on the reserved inode 8): the on-disk constants,
//! the journal superblock, descriptor, and commit codecs, the escape rule,
//! the ring arithmetic, and the small types the rest of the journal code and
//! the wasm boundary share. Everything here is a pure function over byte
//! slices; every JBD2 field is big-endian. `state` holds the journal of a
//! mounted volume: opening it, the format-time writer, and `JournalInfo`;
//! `txn` turns each mutation into one transaction (spec section 5).

pub mod state;
pub mod txn;

use fs_core::{Error, Result};
use std::fmt;

/// The reserved inode that holds the journal.
pub const JOURNAL_INO: u32 = 8;
pub const JBD2_MAGIC: u32 = 0xC03B_3998;
pub const BLOCKTYPE_DESCRIPTOR: u32 = 1;
pub const BLOCKTYPE_COMMIT: u32 = 2;
pub const BLOCKTYPE_SUPERBLOCK_V1: u32 = 3;
pub const BLOCKTYPE_SUPERBLOCK_V2: u32 = 4;
pub const BLOCKTYPE_REVOKE: u32 = 5;
/// The copy's first four bytes were the magic and are zeroed in the journal.
pub const TAG_ESCAPE: u16 = 0x1;
/// The tag shares the uuid of the tag before it; no uuid follows it.
pub const TAG_SAME_UUID: u16 = 0x2;
pub const TAG_DELETED: u16 = 0x4;
/// The last tag of its descriptor block.
pub const TAG_LAST: u16 = 0x8;
/// Magic, block type, and sequence: the header every journal block but a
/// copy starts with.
pub const HEADER_LEN: usize = 12;
/// `t_blocknr` (be32), `t_checksum` (be16), `t_flags` (be16).
pub const TAG_BYTES: usize = 8;
/// The kernel starts a descriptor with 1012 bytes of space, spends 8 per tag
/// plus 16 for the uuid after the first, and closes the block once fewer
/// than 24 bytes remain: after the 122nd tag.
pub const TAGS_PER_DESCRIPTOR: usize = 122;
pub const MIN_JOURNAL_BLOCKS: u32 = 1024;
pub const MIN_VOLUME_BLOCKS_FOR_JOURNAL: u32 = 2048;
/// `s_feature_compat` bit of the ext superblock.
pub const FEATURE_COMPAT_HAS_JOURNAL: u32 = 0x0004;
/// `s_feature_incompat` bit of the ext superblock.
pub const FEATURE_INCOMPAT_RECOVER: u32 = 0x0004;
/// The journal mode bits of the ext superblock's `s_default_mount_opts`.
pub const DEFM_JMODE_MASK: u32 = 0x0060;
pub const DEFM_JMODE_DATA: u32 = 0x0020;
pub const DEFM_JMODE_ORDERED: u32 = 0x0040;
pub const DEFM_JMODE_WBACK: u32 = 0x0060;
/// `h_commit_sec` (be64) in the commit block.
pub const COMMIT_SEC_OFFSET: usize = 48;
/// `h_commit_nsec` (be32) in the commit block.
pub const COMMIT_NSEC_OFFSET: usize = 56;

/// What the journal carries: metadata only (file data written home first),
/// or metadata and file data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JournalMode {
    #[default]
    Ordered,
    Data,
}

impl JournalMode {
    /// The journal mode bits of `s_default_mount_opts`.
    pub fn mount_opts(self) -> u32 {
        match self {
            JournalMode::Ordered => DEFM_JMODE_ORDERED,
            JournalMode::Data => DEFM_JMODE_DATA,
        }
    }

    /// Read the journal mode bits of `s_default_mount_opts`; the other bits
    /// are ignored. No mode bits means ordered, the kernel's default.
    pub fn from_mount_opts(opts: u32) -> Result<Self> {
        match opts & DEFM_JMODE_MASK {
            DEFM_JMODE_DATA => Ok(JournalMode::Data),
            DEFM_JMODE_WBACK => Err(Error::Unsupported("writeback journaling".into())),
            _ => Ok(JournalMode::Ordered),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            JournalMode::Ordered => "ordered",
            JournalMode::Data => "data",
        }
    }

    /// The inverse of `as_str`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ordered" => Some(JournalMode::Ordered),
            "data" => Some(JournalMode::Data),
            _ => None,
        }
    }
}

/// The journal part of the format options. `blocks: None` takes mke2fs's
/// size for the volume (`default_journal_blocks`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JournalOptions {
    pub blocks: Option<u32>,
    pub mode: JournalMode,
}

/// Where an armed crash stops the next transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashPhase {
    /// After the last copy; the commit block is never written.
    BeforeCommit,
    /// After the commit block, before any checkpoint write.
    AfterCommit,
    /// After the first checkpoint write.
    DuringCheckpoint,
}

impl CrashPhase {
    /// The DTO string.
    pub fn as_str(self) -> &'static str {
        match self {
            CrashPhase::BeforeCommit => "before_commit",
            CrashPhase::AfterCommit => "after_commit",
            CrashPhase::DuringCheckpoint => "during_checkpoint",
        }
    }

    /// The inverse of `as_str`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "before_commit" => Some(CrashPhase::BeforeCommit),
            "after_commit" => Some(CrashPhase::AfterCommit),
            "during_checkpoint" => Some(CrashPhase::DuringCheckpoint),
            _ => None,
        }
    }
}

impl fmt::Display for CrashPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CrashPhase::BeforeCommit => "before commit",
            CrashPhase::AfterCommit => "after commit",
            CrashPhase::DuringCheckpoint => "during checkpoint",
        })
    }
}

/// mke2fs's table: None below 2048 blocks; 1024 below 32,768; 4096 below 262,144; else 8192.
pub fn default_journal_blocks(total_blocks: u32) -> Option<u32> {
    match total_blocks {
        0..2048 => None,
        2048..32_768 => Some(1024),
        32_768..262_144 => Some(4096),
        _ => Some(8192),
    }
}

/// The ring: `x` if `x < maxlen`, else `x - (maxlen - first)`.
pub fn wrap(index: u32, first: u32, maxlen: u32) -> u32 {
    if index < maxlen {
        index
    } else {
        index - (maxlen - first)
    }
}

/// The descriptor blocks a transaction of `tagged` blocks needs:
/// `ceil(tagged / 122)`, 0 for 0.
pub fn descriptor_blocks_needed(tagged: usize) -> usize {
    tagged.div_ceil(TAGS_PER_DESCRIPTOR)
}

fn be32_at(b: &[u8], i: usize) -> u32 {
    u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

fn put_be32(b: &mut [u8], i: usize, v: u32) {
    b[i..i + 4].copy_from_slice(&v.to_be_bytes());
}

/// The journal superblock (journal index 0), version 2. The padding at
/// 0x5C..0xFC is not stored: `encode` writes it as zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalSuperblock {
    pub blocktype: u32,
    pub sequence_hdr: u32,
    pub blocksize: u32,
    pub maxlen: u32,
    pub first: u32,
    pub sequence: u32,
    pub start: u32,
    pub errno: i32,
    pub feature_compat: u32,
    pub feature_incompat: u32,
    pub feature_ro_compat: u32,
    pub uuid: [u8; 16],
    pub nr_users: u32,
    pub dynsuper: u32,
    pub max_transaction: u32,
    pub max_trans_data: u32,
    pub checksum_type: u8,
    pub num_fc_blks: u32,
    pub head: u32,
    pub checksum: u32,
    pub users: [u8; 768],
}

impl JournalSuperblock {
    pub const LEN: usize = 1024;
    /// `s_sequence`, the tid the next transaction takes.
    pub const SEQUENCE_OFFSET: usize = 0x18;
    /// `s_start`, the journal index of the oldest live transaction; 0 when
    /// the journal is empty.
    pub const START_OFFSET: usize = 0x1C;

    /// The values mke2fs writes at format: version 2, 1 KiB blocks, the log
    /// from index 1, sequence 1, empty, one user counted but its slot left
    /// zero.
    pub fn new(maxlen: u32, uuid: [u8; 16]) -> Self {
        JournalSuperblock {
            blocktype: BLOCKTYPE_SUPERBLOCK_V2,
            sequence_hdr: 0,
            blocksize: 1024,
            maxlen,
            first: 1,
            sequence: 1,
            start: 0,
            errno: 0,
            feature_compat: 0,
            feature_incompat: 0,
            feature_ro_compat: 0,
            uuid,
            nr_users: 1,
            dynsuper: 0,
            max_transaction: 0,
            max_trans_data: 0,
            checksum_type: 0,
            num_fc_blks: 0,
            head: 0,
            checksum: 0,
            users: [0; 768],
        }
    }

    /// Read every field of the first `LEN` bytes. Only the magic is checked
    /// here; `validate` checks the rest. Panics if `bytes` is shorter than
    /// `LEN`.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let b = &bytes[..Self::LEN];
        let magic = be32_at(b, 0x00);
        if magic != JBD2_MAGIC {
            return Err(Error::CorruptImage(format!(
                "journal superblock magic is 0x{magic:08X}"
            )));
        }
        let mut uuid = [0u8; 16];
        uuid.copy_from_slice(&b[0x30..0x40]);
        let mut users = [0u8; 768];
        users.copy_from_slice(&b[0x100..0x400]);
        Ok(JournalSuperblock {
            blocktype: be32_at(b, 0x04),
            sequence_hdr: be32_at(b, 0x08),
            blocksize: be32_at(b, 0x0C),
            maxlen: be32_at(b, 0x10),
            first: be32_at(b, 0x14),
            sequence: be32_at(b, Self::SEQUENCE_OFFSET),
            start: be32_at(b, Self::START_OFFSET),
            errno: be32_at(b, 0x20) as i32,
            feature_compat: be32_at(b, 0x24),
            feature_incompat: be32_at(b, 0x28),
            feature_ro_compat: be32_at(b, 0x2C),
            uuid,
            nr_users: be32_at(b, 0x40),
            dynsuper: be32_at(b, 0x44),
            max_transaction: be32_at(b, 0x48),
            max_trans_data: be32_at(b, 0x4C),
            checksum_type: b[0x50],
            num_fc_blks: be32_at(b, 0x54),
            head: be32_at(b, 0x58),
            checksum: be32_at(b, 0xFC),
            users,
        })
    }

    /// Write all `LEN` bytes of `out`, the magic included and the padding
    /// zeroed. Panics if `out` is shorter than `LEN`.
    pub fn encode(&self, out: &mut [u8]) {
        let b = &mut out[..Self::LEN];
        b.fill(0);
        put_be32(b, 0x00, JBD2_MAGIC);
        put_be32(b, 0x04, self.blocktype);
        put_be32(b, 0x08, self.sequence_hdr);
        put_be32(b, 0x0C, self.blocksize);
        put_be32(b, 0x10, self.maxlen);
        put_be32(b, 0x14, self.first);
        put_be32(b, Self::SEQUENCE_OFFSET, self.sequence);
        put_be32(b, Self::START_OFFSET, self.start);
        put_be32(b, 0x20, self.errno as u32);
        put_be32(b, 0x24, self.feature_compat);
        put_be32(b, 0x28, self.feature_incompat);
        put_be32(b, 0x2C, self.feature_ro_compat);
        b[0x30..0x40].copy_from_slice(&self.uuid);
        put_be32(b, 0x40, self.nr_users);
        put_be32(b, 0x44, self.dynsuper);
        put_be32(b, 0x48, self.max_transaction);
        put_be32(b, 0x4C, self.max_trans_data);
        b[0x50] = self.checksum_type;
        put_be32(b, 0x54, self.num_fc_blks);
        put_be32(b, 0x58, self.head);
        put_be32(b, 0xFC, self.checksum);
        b[0x100..0x400].copy_from_slice(&self.users);
    }

    /// Check what this crate can replay and write, for a journal inode that
    /// maps `data_blocks` blocks, and return the first failure: the version
    /// (`Unsupported`), then block size, `s_maxlen`, and `s_first`
    /// (`CorruptImage`), then the feature words (`Unsupported`, the first set
    /// bit by name), then `s_start` (`CorruptImage`).
    pub fn validate(&self, data_blocks: u32) -> Result<()> {
        if self.blocktype != BLOCKTYPE_SUPERBLOCK_V2 {
            return Err(Error::Unsupported(format!(
                "journal superblock version {}",
                self.blocktype
            )));
        }
        if self.blocksize != 1024 {
            return Err(Error::CorruptImage(format!(
                "journal superblock s_blocksize is {}, not 1024",
                self.blocksize
            )));
        }
        if self.maxlen != data_blocks {
            return Err(Error::CorruptImage(format!(
                "journal superblock s_maxlen is {} but the journal inode maps {data_blocks} blocks",
                self.maxlen
            )));
        }
        if self.first != 1 {
            return Err(Error::CorruptImage(format!(
                "journal superblock s_first is {}, not 1",
                self.first
            )));
        }
        if let Some(name) = journal_feature_names(
            self.feature_compat,
            self.feature_incompat,
            self.feature_ro_compat,
        )
        .into_iter()
        .next()
        {
            return Err(Error::Unsupported(format!("journal feature {name}")));
        }
        if self.start != 0 && !(self.first..self.maxlen).contains(&self.start) {
            return Err(Error::CorruptImage(format!(
                "journal superblock s_start is {}, outside the log {}..{}",
                self.start, self.first, self.maxlen
            )));
        }
        Ok(())
    }
}

/// A feature word's bits and their e2fsprogs names.
type FeatureTable = &'static [(u32, &'static str)];

/// e2fsprogs' names of the journal compat feature bits.
const JOURNAL_COMPAT_NAMES: [(u32, &str); 1] = [(0x0001, "journal_checksum")];

/// e2fsprogs' names of the journal incompat feature bits.
const JOURNAL_INCOMPAT_NAMES: [(u32, &str); 6] = [
    (0x0001, "journal_incompat_revoke"),
    (0x0002, "journal_64bit"),
    (0x0004, "journal_async_commit"),
    (0x0008, "journal_checksum_v2"),
    (0x0010, "journal_checksum_v3"),
    (0x0020, "fast_commit"),
];

/// Names for the journal feature words, e2fsprogs spelling, for `validate`
/// and tests: every set bit, compat then incompat then ro_compat, lowest bit
/// first. A bit without a name prints as its word and the bit in hex
/// (`ro_compat 0x00000001`).
pub fn journal_feature_names(compat: u32, incompat: u32, ro_compat: u32) -> Vec<String> {
    let words: [(&str, u32, FeatureTable); 3] = [
        ("compat", compat, &JOURNAL_COMPAT_NAMES),
        ("incompat", incompat, &JOURNAL_INCOMPAT_NAMES),
        ("ro_compat", ro_compat, &[]),
    ];
    let mut names = Vec::new();
    for (word, bits, table) in words {
        for bit in (0..32).map(|i| 1u32 << i).filter(|bit| bits & bit != 0) {
            names.push(match table.iter().find(|(b, _)| *b == bit) {
                Some((_, name)) => (*name).to_string(),
                None => format!("{word} 0x{bit:08x}"),
            });
        }
    }
    names
}

/// The header of a descriptor, commit, or revoke block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockHeader {
    pub blocktype: u32,
    pub sequence: u32,
}

/// The header of `block`; None unless the magic is present.
pub fn read_header(block: &[u8]) -> Option<BlockHeader> {
    let b = block.get(..HEADER_LEN)?;
    (be32_at(b, 0) == JBD2_MAGIC).then(|| BlockHeader {
        blocktype: be32_at(b, 4),
        sequence: be32_at(b, 8),
    })
}

/// Write the magic, `blocktype`, and `sequence` into the first 12 bytes.
pub fn write_header(block: &mut [u8], blocktype: u32, sequence: u32) {
    put_be32(block, 0, JBD2_MAGIC);
    put_be32(block, 4, blocktype);
    put_be32(block, 8, sequence);
}

/// One descriptor tag: the home block and the flags as on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tag {
    pub block: u32,
    pub flags: u16,
}

/// A descriptor block for `sequence` listing `tags` in order. The caller
/// passes only TAG_ESCAPE in flags; SAME_UUID and LAST are set here: the
/// first tag is followed by `uuid`, every later tag sets SAME_UUID, the last
/// sets LAST. Any SAME_UUID or LAST the caller passes is replaced, so
/// re-encoding decoded tags gives the same block. Panics unless
/// `1 <= tags.len() <= TAGS_PER_DESCRIPTOR`.
pub fn encode_descriptor(sequence: u32, uuid: &[u8; 16], tags: &[Tag]) -> [u8; 1024] {
    assert!(
        !tags.is_empty() && tags.len() <= TAGS_PER_DESCRIPTOR,
        "a descriptor holds 1..={TAGS_PER_DESCRIPTOR} tags, not {}",
        tags.len()
    );
    let mut block = [0u8; 1024];
    write_header(&mut block, BLOCKTYPE_DESCRIPTOR, sequence);
    let mut pos = HEADER_LEN;
    for (i, tag) in tags.iter().enumerate() {
        let mut flags = tag.flags & !(TAG_SAME_UUID | TAG_LAST);
        if i > 0 {
            flags |= TAG_SAME_UUID;
        }
        if i + 1 == tags.len() {
            flags |= TAG_LAST;
        }
        put_be32(&mut block, pos, tag.block);
        // t_checksum (be16) at pos + 4 stays zero.
        block[pos + 6..pos + 8].copy_from_slice(&flags.to_be_bytes());
        pos += TAG_BYTES;
        if i == 0 {
            block[pos..pos + 16].copy_from_slice(uuid);
            pos += 16;
        }
    }
    block
}

/// The tags of a descriptor block, stopping at TAG_LAST, with their flags as
/// on disk; a uuid follows every tag without SAME_UUID. The header is not
/// checked. CorruptImage if the tags run past the block without one carrying
/// TAG_LAST.
pub fn decode_descriptor(block: &[u8]) -> Result<Vec<Tag>> {
    let mut tags = Vec::new();
    let mut pos = HEADER_LEN;
    while pos + TAG_BYTES <= block.len() {
        let tag = Tag {
            block: be32_at(block, pos),
            flags: u16::from_be_bytes([block[pos + 6], block[pos + 7]]),
        };
        tags.push(tag);
        if tag.flags & TAG_LAST != 0 {
            return Ok(tags);
        }
        pos += TAG_BYTES;
        if tag.flags & TAG_SAME_UUID == 0 {
            pos += 16;
        }
    }
    Err(Error::CorruptImage(format!(
        "journal descriptor tags run past the end of the block without a last tag ({} read)",
        tags.len()
    )))
}

/// A commit block for `sequence` stamped `commit_sec` (Unix seconds): no
/// checksum, `h_commit_nsec` 0, the rest zero.
pub fn encode_commit(sequence: u32, commit_sec: u64) -> [u8; 1024] {
    let mut block = [0u8; 1024];
    write_header(&mut block, BLOCKTYPE_COMMIT, sequence);
    block[COMMIT_SEC_OFFSET..COMMIT_SEC_OFFSET + 8].copy_from_slice(&commit_sec.to_be_bytes());
    put_be32(&mut block, COMMIT_NSEC_OFFSET, 0);
    block
}

/// The `h_commit_sec` of a commit block; None without the magic or type 2.
pub fn decode_commit(block: &[u8]) -> Option<u64> {
    let header = read_header(block)?;
    if header.blocktype != BLOCKTYPE_COMMIT || block.len() < COMMIT_SEC_OFFSET + 8 {
        return None;
    }
    let mut sec = [0u8; 8];
    sec.copy_from_slice(&block[COMMIT_SEC_OFFSET..COMMIT_SEC_OFFSET + 8]);
    Some(u64::from_be_bytes(sec))
}

/// Whether a copy of `block` must be escaped: its first four bytes are the
/// journal magic, which recovery would take for a journal block.
pub fn needs_escape(block: &[u8]) -> bool {
    block.len() >= 4 && be32_at(block, 0) == JBD2_MAGIC
}

/// Zero the first four bytes.
pub fn escape(block: &mut [u8]) {
    block[..4].fill(0);
}

/// Restore the magic in the first four bytes.
pub fn unescape(block: &mut [u8]) {
    put_be32(block, 0, JBD2_MAGIC);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journal_mode_maps_to_the_default_mount_options() {
        assert_eq!(JournalMode::default(), JournalMode::Ordered);
        assert_eq!(JournalMode::Ordered.mount_opts(), 0x0040);
        assert_eq!(JournalMode::Data.mount_opts(), 0x0020);
        assert_eq!(JournalMode::from_mount_opts(0), Ok(JournalMode::Ordered));
        assert_eq!(
            JournalMode::from_mount_opts(0x0040),
            Ok(JournalMode::Ordered)
        );
        assert_eq!(JournalMode::from_mount_opts(0x0020), Ok(JournalMode::Data));
        // Bits outside the mode mask do not matter.
        assert_eq!(
            JournalMode::from_mount_opts(0x0020 | 0x0004 | 0x0008),
            Ok(JournalMode::Data)
        );
        assert_eq!(
            JournalMode::from_mount_opts(0x0060),
            Err(Error::Unsupported("writeback journaling".into()))
        );
        assert_eq!(
            JournalOptions::default(),
            JournalOptions {
                blocks: None,
                mode: JournalMode::Ordered
            }
        );
    }

    #[test]
    fn journal_mode_and_crash_phase_strings_round_trip() {
        for mode in [JournalMode::Ordered, JournalMode::Data] {
            assert_eq!(JournalMode::parse(mode.as_str()), Some(mode));
        }
        assert_eq!(JournalMode::Ordered.as_str(), "ordered");
        assert_eq!(JournalMode::Data.as_str(), "data");
        assert_eq!(JournalMode::parse("writeback"), None);
        assert_eq!(JournalMode::parse("Ordered"), None);

        let phases = [
            (CrashPhase::BeforeCommit, "before_commit", "before commit"),
            (CrashPhase::AfterCommit, "after_commit", "after commit"),
            (
                CrashPhase::DuringCheckpoint,
                "during_checkpoint",
                "during checkpoint",
            ),
        ];
        for (phase, dto, text) in phases {
            assert_eq!(phase.as_str(), dto);
            assert_eq!(CrashPhase::parse(dto), Some(phase));
            assert_eq!(phase.to_string(), text);
        }
        assert_eq!(CrashPhase::parse("before commit"), None);
        assert_eq!(CrashPhase::parse(""), None);
    }

    #[test]
    fn default_journal_blocks_follows_the_mke2fs_table() {
        assert_eq!(default_journal_blocks(0), None);
        assert_eq!(default_journal_blocks(2047), None);
        assert_eq!(default_journal_blocks(2048), Some(1024));
        assert_eq!(default_journal_blocks(16_384), Some(1024));
        assert_eq!(default_journal_blocks(32_767), Some(1024));
        assert_eq!(default_journal_blocks(32_768), Some(4096));
        assert_eq!(default_journal_blocks(262_143), Some(4096));
        assert_eq!(default_journal_blocks(262_144), Some(8192));
        assert_eq!(
            default_journal_blocks(MIN_VOLUME_BLOCKS_FOR_JOURNAL),
            Some(MIN_JOURNAL_BLOCKS)
        );
    }

    #[test]
    fn wrap_folds_indexes_past_maxlen_back_to_first() {
        assert_eq!(wrap(1, 1, 1024), 1);
        assert_eq!(wrap(1023, 1, 1024), 1023);
        assert_eq!(wrap(1024, 1, 1024), 1);
        assert_eq!(wrap(1025, 1, 1024), 2);
        assert_eq!(wrap(1030, 1, 1024), 7);
    }

    #[test]
    fn descriptor_blocks_needed_is_ceil_of_122() {
        assert_eq!(descriptor_blocks_needed(0), 0);
        assert_eq!(descriptor_blocks_needed(1), 1);
        assert_eq!(descriptor_blocks_needed(122), 1);
        assert_eq!(descriptor_blocks_needed(123), 2);
        assert_eq!(descriptor_blocks_needed(244), 2);
        assert_eq!(descriptor_blocks_needed(245), 3);
    }

    const UUID: [u8; 16] = [
        0xe2, 0xf5, 0xee, 0x00, 0x20, 0x26, 0x49, 0x23, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x01,
    ];

    fn unsupported(sb: &JournalSuperblock, data_blocks: u32) -> String {
        match sb.validate(data_blocks) {
            Err(Error::Unsupported(msg)) => msg,
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    fn corrupt(sb: &JournalSuperblock, data_blocks: u32) -> String {
        match sb.validate(data_blocks) {
            Err(Error::CorruptImage(msg)) => msg,
            other => panic!("expected CorruptImage, got {other:?}"),
        }
    }

    #[test]
    fn new_encodes_the_mke2fs_format_time_bytes() {
        // mke2fs 1.47.4, `-t ext3 -b 1024 -J size=1`: journal index 0.
        let mut expected = [0u8; 0x60];
        let words: [u32; 8] = [
            0xc03b_3998,
            0x0000_0004,
            0x0000_0000,
            0x0000_0400,
            0x0000_0400,
            0x0000_0001,
            0x0000_0001,
            0x0000_0000,
        ];
        for (i, word) in words.iter().enumerate() {
            expected[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        expected[0x30..0x40].copy_from_slice(&UUID);
        expected[0x40..0x44].copy_from_slice(&[0, 0, 0, 1]);

        let mut bytes = [0xAAu8; 1024];
        JournalSuperblock::new(1024, UUID).encode(&mut bytes);
        assert_eq!(&bytes[..0x60], &expected[..]);
        assert!(bytes[0x60..0x400].iter().all(|&b| b == 0));
        assert_eq!(
            &bytes[JournalSuperblock::SEQUENCE_OFFSET..JournalSuperblock::SEQUENCE_OFFSET + 4],
            &[0, 0, 0, 1]
        );
        assert_eq!(
            &bytes[JournalSuperblock::START_OFFSET..JournalSuperblock::START_OFFSET + 4],
            &[0, 0, 0, 0]
        );
    }

    #[test]
    fn superblock_decode_round_trips_encode() {
        let mut users = [0u8; 768];
        users[..16].copy_from_slice(&UUID);
        users[767] = 0x5A;
        let sb = JournalSuperblock {
            blocktype: 3,
            sequence_hdr: 0x0102_0304,
            blocksize: 4096,
            maxlen: 8192,
            first: 2,
            sequence: 0xDEAD_BEEF,
            start: 17,
            errno: -5,
            feature_compat: 0x1,
            feature_incompat: 0x12,
            feature_ro_compat: 0x8000_0000,
            uuid: UUID,
            nr_users: 2,
            dynsuper: 3,
            max_transaction: 256,
            max_trans_data: 128,
            checksum_type: 4,
            num_fc_blks: 64,
            head: 99,
            checksum: 0xCAFE_F00D,
            users,
        };
        let mut bytes = [0xAAu8; 1024];
        sb.encode(&mut bytes);
        assert_eq!(&bytes[0x20..0x24], &(-5i32).to_be_bytes());
        assert_eq!(bytes[0x50], 4);
        assert_eq!(&bytes[0x51..0x54], &[0, 0, 0]);
        assert!(bytes[0x5C..0xFC].iter().all(|&b| b == 0));
        assert_eq!(&bytes[0xFC..0x100], &[0xCA, 0xFE, 0xF0, 0x0D]);
        assert_eq!(JournalSuperblock::decode(&bytes), Ok(sb));

        let fresh = JournalSuperblock::new(1024, UUID);
        fresh.encode(&mut bytes);
        assert_eq!(JournalSuperblock::decode(&bytes), Ok(fresh));
    }

    #[test]
    fn decode_rejects_a_block_without_the_magic() {
        let bytes = [0u8; 1024];
        assert_eq!(
            JournalSuperblock::decode(&bytes),
            Err(Error::CorruptImage(
                "journal superblock magic is 0x00000000".into()
            ))
        );
        let mut bytes = [0u8; 1024];
        JournalSuperblock::new(1024, UUID).encode(&mut bytes);
        bytes[3] = 0x99;
        assert_eq!(
            JournalSuperblock::decode(&bytes),
            Err(Error::CorruptImage(
                "journal superblock magic is 0xC03B3999".into()
            ))
        );
    }

    #[test]
    fn validate_accepts_the_format_time_superblock_and_any_start_in_the_log() {
        let good = JournalSuperblock::new(1024, UUID);
        assert_eq!(good.validate(1024), Ok(()));
        for start in [1, 500, 1023] {
            let sb = JournalSuperblock {
                start,
                ..good.clone()
            };
            assert_eq!(sb.validate(1024), Ok(()));
        }
    }

    #[test]
    fn validate_reports_the_version_then_each_field() {
        let good = JournalSuperblock::new(1024, UUID);
        assert_eq!(
            unsupported(
                &JournalSuperblock {
                    blocktype: BLOCKTYPE_SUPERBLOCK_V1,
                    ..good.clone()
                },
                1024
            ),
            "journal superblock version 3"
        );
        // The version comes before the fields.
        assert_eq!(
            unsupported(
                &JournalSuperblock {
                    blocktype: BLOCKTYPE_SUPERBLOCK_V1,
                    blocksize: 4096,
                    ..good.clone()
                },
                1024
            ),
            "journal superblock version 3"
        );
        let msg = corrupt(
            &JournalSuperblock {
                blocksize: 4096,
                ..good.clone()
            },
            1024,
        );
        assert_eq!(msg, "journal superblock s_blocksize is 4096, not 1024");
        let msg = corrupt(&good, 2048);
        assert_eq!(
            msg,
            "journal superblock s_maxlen is 1024 but the journal inode maps 2048 blocks"
        );
        let msg = corrupt(
            &JournalSuperblock {
                first: 2,
                ..good.clone()
            },
            1024,
        );
        assert_eq!(msg, "journal superblock s_first is 2, not 1");
        // The fields come before the features.
        let msg = corrupt(
            &JournalSuperblock {
                first: 0,
                feature_incompat: 0x2,
                ..good
            },
            1024,
        );
        assert!(msg.contains("s_first"), "{msg}");
    }

    #[test]
    fn validate_names_the_first_journal_feature() {
        let good = JournalSuperblock::new(1024, UUID);
        let with = |compat, incompat, ro_compat| JournalSuperblock {
            feature_compat: compat,
            feature_incompat: incompat,
            feature_ro_compat: ro_compat,
            ..good.clone()
        };
        assert_eq!(
            unsupported(&with(0x1, 0, 0), 1024),
            "journal feature journal_checksum"
        );
        assert_eq!(
            unsupported(&with(0, 0x2, 0), 1024),
            "journal feature journal_64bit"
        );
        assert_eq!(
            unsupported(&with(0, 0x10 | 0x8, 0), 1024),
            "journal feature journal_checksum_v2"
        );
        assert_eq!(
            unsupported(&with(0, 0, 0x1), 1024),
            "journal feature ro_compat 0x00000001"
        );
        // compat before incompat before ro_compat.
        assert_eq!(
            unsupported(&with(0x1, 0x1, 0x1), 1024),
            "journal feature journal_checksum"
        );
        assert_eq!(
            unsupported(&with(0, 0x20, 0x1), 1024),
            "journal feature fast_commit"
        );
        // The features come before the start.
        let msg = unsupported(
            &JournalSuperblock {
                start: 5000,
                ..with(0, 0x1, 0)
            },
            1024,
        );
        assert_eq!(msg, "journal feature journal_incompat_revoke");
    }

    #[test]
    fn validate_rejects_a_start_outside_the_log() {
        let good = JournalSuperblock::new(1024, UUID);
        let msg = corrupt(
            &JournalSuperblock {
                start: 1024,
                ..good.clone()
            },
            1024,
        );
        assert_eq!(
            msg,
            "journal superblock s_start is 1024, outside the log 1..1024"
        );
        let msg = corrupt(
            &JournalSuperblock {
                start: 70_000,
                ..good
            },
            1024,
        );
        assert!(msg.contains("s_start is 70000"), "{msg}");
    }

    #[test]
    fn journal_feature_names_uses_the_e2fsprogs_spelling() {
        assert!(journal_feature_names(0, 0, 0).is_empty());
        assert_eq!(journal_feature_names(0x1, 0, 0), vec!["journal_checksum"]);
        let incompat = [
            (0x01, "journal_incompat_revoke"),
            (0x02, "journal_64bit"),
            (0x04, "journal_async_commit"),
            (0x08, "journal_checksum_v2"),
            (0x10, "journal_checksum_v3"),
            (0x20, "fast_commit"),
        ];
        for (bit, name) in incompat {
            assert_eq!(journal_feature_names(0, bit, 0), vec![name]);
        }
        assert_eq!(
            journal_feature_names(0x2, 0x40, 0x8000_0001),
            vec![
                "compat 0x00000002",
                "incompat 0x00000040",
                "ro_compat 0x00000001",
                "ro_compat 0x80000000",
            ]
        );
        assert_eq!(
            journal_feature_names(0x1, 0x3f, 0),
            vec![
                "journal_checksum",
                "journal_incompat_revoke",
                "journal_64bit",
                "journal_async_commit",
                "journal_checksum_v2",
                "journal_checksum_v3",
                "fast_commit",
            ]
        );
    }

    fn tags(blocks: std::ops::Range<u32>) -> Vec<Tag> {
        blocks.map(|block| Tag { block, flags: 0 }).collect()
    }

    fn be16_at(b: &[u8], i: usize) -> u16 {
        u16::from_be_bytes([b[i], b[i + 1]])
    }

    #[test]
    fn header_round_trips_and_requires_the_magic() {
        let mut block = [0u8; 1024];
        assert_eq!(read_header(&block), None);
        write_header(&mut block, BLOCKTYPE_REVOKE, 42);
        assert_eq!(
            &block[..HEADER_LEN],
            &[0xC0, 0x3B, 0x39, 0x98, 0, 0, 0, 5, 0, 0, 0, 42]
        );
        assert_eq!(
            read_header(&block),
            Some(BlockHeader {
                blocktype: BLOCKTYPE_REVOKE,
                sequence: 42
            })
        );
        assert_eq!(read_header(&block[..HEADER_LEN - 1]), None);
        block[0] = 0;
        assert_eq!(read_header(&block), None);
    }

    #[test]
    fn descriptor_packs_the_uuid_after_the_first_tag() {
        let block = encode_descriptor(
            7,
            &UUID,
            &[
                Tag { block: 5, flags: 0 },
                Tag {
                    block: 69,
                    flags: TAG_ESCAPE,
                },
                Tag {
                    block: 1111,
                    flags: 0,
                },
            ],
        );
        assert_eq!(
            read_header(&block),
            Some(BlockHeader {
                blocktype: BLOCKTYPE_DESCRIPTOR,
                sequence: 7
            })
        );
        // Tag 0 at 12, the uuid at 20, tag 1 at 36, tag 2 at 44.
        assert_eq!(be32_at(&block, 12), 5);
        assert_eq!(be16_at(&block, 16), 0);
        assert_eq!(be16_at(&block, 18), 0);
        assert_eq!(&block[20..36], &UUID);
        assert_eq!(be32_at(&block, 36), 69);
        assert_eq!(be16_at(&block, 40), 0);
        assert_eq!(be16_at(&block, 42), TAG_ESCAPE | TAG_SAME_UUID);
        assert_eq!(be32_at(&block, 44), 1111);
        assert_eq!(be16_at(&block, 50), TAG_SAME_UUID | TAG_LAST);
        assert!(block[52..].iter().all(|&b| b == 0));

        // A single tag is both first and last.
        let one = encode_descriptor(1, &UUID, &tags(82..83));
        assert_eq!(be16_at(&one, 18), TAG_LAST);
        assert_eq!(&one[20..36], &UUID);
        assert!(one[36..].iter().all(|&b| b == 0));
    }

    #[test]
    fn exactly_122_tags_fit_in_a_descriptor() {
        let block = encode_descriptor(3, &UUID, &tags(100..222));
        // 12 header + 8 first tag + 16 uuid + 121 more tags = 1004 bytes.
        let last = HEADER_LEN + TAG_BYTES + 16 + 120 * TAG_BYTES;
        assert_eq!(last + TAG_BYTES, 1004);
        assert_eq!(be32_at(&block, last), 221);
        assert_eq!(be16_at(&block, last + 6), TAG_SAME_UUID | TAG_LAST);
        assert!(block[1004..].iter().all(|&b| b == 0));
        assert_eq!(
            decode_descriptor(&block).unwrap().len(),
            TAGS_PER_DESCRIPTOR
        );
    }

    #[test]
    #[should_panic(expected = "a descriptor holds 1..=122 tags, not 123")]
    fn encode_descriptor_refuses_a_123rd_tag() {
        encode_descriptor(3, &UUID, &tags(0..123));
    }

    #[test]
    fn decode_descriptor_round_trips_encode_descriptor() {
        let input = [
            Tag { block: 5, flags: 0 },
            Tag {
                block: 69,
                flags: TAG_ESCAPE,
            },
            Tag {
                block: 1111,
                flags: 0,
            },
        ];
        let block = encode_descriptor(9, &UUID, &input);
        let decoded = decode_descriptor(&block).unwrap();
        assert_eq!(
            decoded,
            vec![
                Tag { block: 5, flags: 0 },
                Tag {
                    block: 69,
                    flags: TAG_ESCAPE | TAG_SAME_UUID
                },
                Tag {
                    block: 1111,
                    flags: TAG_SAME_UUID | TAG_LAST
                },
            ]
        );
        // The on-disk flags re-encode to the same block.
        assert_eq!(encode_descriptor(9, &UUID, &decoded), block);

        let full = encode_descriptor(4, &UUID, &tags(0..122));
        let decoded = decode_descriptor(&full).unwrap();
        assert_eq!(
            decoded.iter().map(|t| t.block).collect::<Vec<_>>(),
            (0..122).collect::<Vec<_>>()
        );
        assert_eq!(encode_descriptor(4, &UUID, &decoded), full);
    }

    #[test]
    fn decode_descriptor_rejects_a_block_without_a_last_tag() {
        let mut block = encode_descriptor(2, &UUID, &tags(10..12));
        // Clear LAST on the second tag (t_flags at 12 + 8 + 16 + 6 = 42).
        block[43] &= !(TAG_LAST as u8);
        let err = decode_descriptor(&block).unwrap_err();
        assert!(
            matches!(&err, Error::CorruptImage(msg) if msg.contains("without a last tag")),
            "{err:?}"
        );
        // A zeroed body reads as tags that each carry a uuid and never end.
        let mut empty = [0u8; 1024];
        write_header(&mut empty, BLOCKTYPE_DESCRIPTOR, 2);
        assert!(matches!(
            decode_descriptor(&empty),
            Err(Error::CorruptImage(_))
        ));
    }

    #[test]
    fn escape_zeroes_the_magic_and_unescape_restores_it() {
        let mut block = [0x11u8; 1024];
        assert!(!needs_escape(&block));
        block[..4].copy_from_slice(&[0xC0, 0x3B, 0x39, 0x98]);
        assert!(needs_escape(&block));
        let home = block;
        escape(&mut block);
        assert_eq!(&block[..4], &[0, 0, 0, 0]);
        assert_eq!(&block[4..], &home[4..]);
        assert!(!needs_escape(&block));
        unescape(&mut block);
        assert_eq!(block, home);
        // Only the full magic in the first four bytes counts.
        assert!(!needs_escape(&[0xC0, 0x3B, 0x39]));
        assert!(!needs_escape(&[0x00, 0xC0, 0x3B, 0x39, 0x98]));
    }

    #[test]
    fn commit_block_carries_the_commit_time() {
        let block = encode_commit(12, 1_758_672_000);
        assert_eq!(
            &block[..HEADER_LEN],
            &[0xC0, 0x3B, 0x39, 0x98, 0, 0, 0, 2, 0, 0, 0, 12]
        );
        // h_chksum_type, h_chksum_size, padding, and h_chksum[8] are zero.
        assert!(block[HEADER_LEN..COMMIT_SEC_OFFSET].iter().all(|&b| b == 0));
        assert_eq!(
            &block[COMMIT_SEC_OFFSET..COMMIT_SEC_OFFSET + 8],
            &[0, 0, 0, 0, 0x68, 0xD3, 0x34, 0x80]
        );
        assert_eq!(&block[COMMIT_NSEC_OFFSET..COMMIT_NSEC_OFFSET + 4], &[0; 4]);
        assert!(block[COMMIT_NSEC_OFFSET + 4..].iter().all(|&b| b == 0));
        assert_eq!(decode_commit(&block), Some(1_758_672_000));

        assert_eq!(decode_commit(&[0u8; 1024]), None);
        let descriptor = encode_descriptor(12, &UUID, &tags(5..6));
        assert_eq!(decode_commit(&descriptor), None);
    }
}
