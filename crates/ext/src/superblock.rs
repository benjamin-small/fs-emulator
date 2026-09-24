//! The ext2 superblock (1,024 bytes at byte 1,024 of the disk and at the
//! start of every backup group), the format options, and the rules that
//! decide which superblocks this crate can mount.

use crate::group::Geometry;
use fs_core::{Error, Result};

pub const EXT2_MAGIC: u16 = 0xEF53;
pub const BLOCK_SIZE: u32 = 1024;
pub const INODE_SIZE: u16 = 128;
pub const BLOCKS_PER_GROUP: u32 = 8192;
pub const FIRST_INO: u32 = 11;
pub const ROOT_INO: u32 = 2;
pub const LOST_FOUND_INO: u32 = 11;
/// Inodes `1..=RESERVED_INODES` are reserved.
pub const RESERVED_INODES: u32 = 10;
pub const FEATURE_INCOMPAT_FILETYPE: u32 = 0x0002;
pub const FEATURE_RO_COMPAT_SPARSE_SUPER: u32 = 0x0001;
/// Byte offset of the primary superblock.
pub const SUPERBLOCK_OFFSET: usize = 1024;
pub const MIN_BLOCKS: u32 = 64;
/// 256 MiB at 1 KiB blocks, the wasm cap.
pub const MAX_BLOCKS: u32 = 262_144;
/// `e2f5ee00-2026-4923-8000-000000000001`.
pub const DEFAULT_UUID: [u8; 16] = [
    0xe2, 0xf5, 0xee, 0x00, 0x20, 0x26, 0x49, 0x23, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
];

/// Names of the incompat feature bits, for `Unsupported` messages.
const INCOMPAT_NAMES: [(u32, &str); 16] = [
    (0x0001, "compression"),
    (0x0002, "filetype"),
    (0x0004, "needs_recovery"),
    (0x0008, "journal_dev"),
    (0x0010, "meta_bg"),
    (0x0040, "extent"),
    (0x0080, "64bit"),
    (0x0100, "mmp"),
    (0x0200, "flex_bg"),
    (0x0400, "ea_inode"),
    (0x1000, "dirdata"),
    (0x2000, "metadata_csum_seed"),
    (0x4000, "large_dir"),
    (0x8000, "inline_data"),
    (0x1_0000, "encrypt"),
    (0x2_0000, "casefold"),
];

/// Names of the ro_compat feature bits, for `Unsupported` messages.
const RO_COMPAT_NAMES: [(u32, &str); 12] = [
    (0x0001, "sparse_super"),
    (0x0002, "large_file"),
    (0x0004, "btree_dir"),
    (0x0008, "huge_file"),
    (0x0010, "uninit_bg"),
    (0x0020, "dir_nlink"),
    (0x0040, "extra_isize"),
    (0x0100, "quota"),
    (0x0200, "bigalloc"),
    (0x0400, "metadata_csum"),
    (0x2000, "project"),
    (0x8000, "verity"),
];

/// Space-separated names of every set bit in `bits`; unnamed bits print as hex.
fn feature_names(bits: u32, names: &[(u32, &str)]) -> String {
    (0..32)
        .map(|i| 1u32 << i)
        .filter(|bit| bits & bit != 0)
        .map(|bit| match names.iter().find(|(b, _)| *b == bit) {
            Some((_, name)) => (*name).to_string(),
            None => format!("0x{bit:x}"),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `inodes_per_group` is a multiple of 8 in `16..=8192`.
pub(crate) fn inodes_per_group_is_valid(ipg: u32) -> bool {
    (16..=8192).contains(&ipg) && ipg.is_multiple_of(8)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtFormatOptions {
    pub total_blocks: u32,
    /// `None` derives one inode per 16 KiB (`default_inodes_per_group`).
    pub inodes_per_group: Option<u32>,
    /// The volume name, NUL-padded.
    pub label: [u8; 16],
    pub uuid: [u8; 16],
}

impl Default for ExtFormatOptions {
    fn default() -> Self {
        Self {
            total_blocks: 16_384,
            inodes_per_group: None,
            label: [0; 16],
            uuid: DEFAULT_UUID,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Superblock {
    pub inodes_count: u32,
    pub blocks_count: u32,
    pub r_blocks_count: u32,
    pub free_blocks_count: u32,
    pub free_inodes_count: u32,
    pub first_data_block: u32,
    pub log_block_size: u32,
    pub log_frag_size: u32,
    pub blocks_per_group: u32,
    pub frags_per_group: u32,
    pub inodes_per_group: u32,
    pub mtime: u32,
    pub wtime: u32,
    pub mnt_count: u16,
    pub max_mnt_count: i16,
    pub magic: u16,
    pub state: u16,
    pub errors: u16,
    pub minor_rev_level: u16,
    pub lastcheck: u32,
    pub checkinterval: u32,
    pub creator_os: u32,
    pub rev_level: u32,
    pub def_resuid: u16,
    pub def_resgid: u16,
    pub first_ino: u32,
    pub inode_size: u16,
    pub block_group_nr: u16,
    pub feature_compat: u32,
    pub feature_incompat: u32,
    pub feature_ro_compat: u32,
    pub uuid: [u8; 16],
    pub volume_name: [u8; 16],
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

fn put_u16(b: &mut [u8], i: usize, v: u16) {
    b[i..i + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_u32(b: &mut [u8], i: usize, v: u32) {
    b[i..i + 4].copy_from_slice(&v.to_le_bytes());
}

impl Superblock {
    pub const LEN: usize = 1024;

    /// Read every field of the first `LEN` bytes without validating any of
    /// them. Panics if `bytes` is shorter than `LEN`.
    pub fn decode(bytes: &[u8]) -> Superblock {
        let b = &bytes[..Self::LEN];
        let mut uuid = [0u8; 16];
        uuid.copy_from_slice(&b[104..120]);
        let mut volume_name = [0u8; 16];
        volume_name.copy_from_slice(&b[120..136]);
        Superblock {
            inodes_count: u32_at(b, 0),
            blocks_count: u32_at(b, 4),
            r_blocks_count: u32_at(b, 8),
            free_blocks_count: u32_at(b, 12),
            free_inodes_count: u32_at(b, 16),
            first_data_block: u32_at(b, 20),
            log_block_size: u32_at(b, 24),
            log_frag_size: u32_at(b, 28),
            blocks_per_group: u32_at(b, 32),
            frags_per_group: u32_at(b, 36),
            inodes_per_group: u32_at(b, 40),
            mtime: u32_at(b, 44),
            wtime: u32_at(b, 48),
            mnt_count: u16_at(b, 52),
            max_mnt_count: u16_at(b, 54) as i16,
            magic: u16_at(b, 56),
            state: u16_at(b, 58),
            errors: u16_at(b, 60),
            minor_rev_level: u16_at(b, 62),
            lastcheck: u32_at(b, 64),
            checkinterval: u32_at(b, 68),
            creator_os: u32_at(b, 72),
            rev_level: u32_at(b, 76),
            def_resuid: u16_at(b, 80),
            def_resgid: u16_at(b, 82),
            first_ino: u32_at(b, 84),
            inode_size: u16_at(b, 88),
            block_group_nr: u16_at(b, 90),
            feature_compat: u32_at(b, 92),
            feature_incompat: u32_at(b, 96),
            feature_ro_compat: u32_at(b, 100),
            uuid,
            volume_name,
        }
    }

    /// Write every field into the first `LEN` bytes of `out`; the fields
    /// this crate does not model (`s_last_mounted` onwards, bytes 136..1024)
    /// are zeroed. Panics if `out` is shorter than `LEN`.
    pub fn encode(&self, out: &mut [u8]) {
        let b = &mut out[..Self::LEN];
        b.fill(0);
        put_u32(b, 0, self.inodes_count);
        put_u32(b, 4, self.blocks_count);
        put_u32(b, 8, self.r_blocks_count);
        put_u32(b, 12, self.free_blocks_count);
        put_u32(b, 16, self.free_inodes_count);
        put_u32(b, 20, self.first_data_block);
        put_u32(b, 24, self.log_block_size);
        put_u32(b, 28, self.log_frag_size);
        put_u32(b, 32, self.blocks_per_group);
        put_u32(b, 36, self.frags_per_group);
        put_u32(b, 40, self.inodes_per_group);
        put_u32(b, 44, self.mtime);
        put_u32(b, 48, self.wtime);
        put_u16(b, 52, self.mnt_count);
        put_u16(b, 54, self.max_mnt_count as u16);
        put_u16(b, 56, self.magic);
        put_u16(b, 58, self.state);
        put_u16(b, 60, self.errors);
        put_u16(b, 62, self.minor_rev_level);
        put_u32(b, 64, self.lastcheck);
        put_u32(b, 68, self.checkinterval);
        put_u32(b, 72, self.creator_os);
        put_u32(b, 76, self.rev_level);
        put_u16(b, 80, self.def_resuid);
        put_u16(b, 82, self.def_resgid);
        put_u32(b, 84, self.first_ino);
        put_u16(b, 88, self.inode_size);
        put_u16(b, 90, self.block_group_nr);
        put_u32(b, 92, self.feature_compat);
        put_u32(b, 96, self.feature_incompat);
        put_u32(b, 100, self.feature_ro_compat);
        b[104..120].copy_from_slice(&self.uuid);
        b[120..136].copy_from_slice(&self.volume_name);
    }

    /// The primary superblock of a fresh volume. The free counts are the
    /// sums of every group's `GroupDescriptor::initial` (all non-metadata
    /// blocks, every inode past the reserved ones); `format` subtracts what
    /// the root directory and `lost+found` then take.
    pub fn new(options: &ExtFormatOptions, geo: &Geometry, now: u32) -> Superblock {
        let metadata: u32 = (0..geo.groups)
            .map(|g| geo.metadata_blocks_in_group(g))
            .sum();
        Superblock {
            inodes_count: geo.inodes_count,
            blocks_count: geo.total_blocks,
            r_blocks_count: 0,
            free_blocks_count: geo.total_blocks - geo.first_data_block - metadata,
            free_inodes_count: geo.inodes_count - RESERVED_INODES,
            first_data_block: geo.first_data_block,
            log_block_size: 0,
            log_frag_size: 0,
            blocks_per_group: geo.blocks_per_group,
            frags_per_group: geo.blocks_per_group,
            inodes_per_group: geo.inodes_per_group,
            mtime: 0,
            wtime: now,
            mnt_count: 0,
            max_mnt_count: -1,
            magic: EXT2_MAGIC,
            state: 1,
            errors: 1,
            minor_rev_level: 0,
            lastcheck: now,
            checkinterval: 0,
            creator_os: 0,
            rev_level: 1,
            def_resuid: 0,
            def_resgid: 0,
            first_ino: FIRST_INO,
            inode_size: INODE_SIZE,
            block_group_nr: 0,
            feature_compat: 0,
            feature_incompat: FEATURE_INCOMPAT_FILETYPE,
            feature_ro_compat: FEATURE_RO_COMPAT_SPARSE_SUPER,
            uuid: options.uuid,
            volume_name: options.label,
        }
    }

    /// Check that this crate can mount the superblock of a disk holding
    /// `disk_blocks` 1 KiB blocks. The magic is checked first, then the
    /// revision, block size, inode size, and features (`Unsupported`), then
    /// the geometry (`CorruptImage`).
    pub fn validate(&self, disk_blocks: u64) -> Result<()> {
        if self.magic != EXT2_MAGIC {
            return Err(Error::CorruptImage(format!(
                "superblock magic is 0x{:04X}, not 0xEF53",
                self.magic
            )));
        }
        if self.rev_level != 1 {
            return Err(Error::Unsupported(format!(
                "ext2 revision {} (only revision 1 is supported)",
                self.rev_level
            )));
        }
        if self.log_block_size != 0 {
            return Err(Error::Unsupported(format!(
                "block size 1024 << {} (only 1 KiB blocks are supported)",
                self.log_block_size
            )));
        }
        if self.inode_size != INODE_SIZE {
            return Err(Error::Unsupported(format!(
                "inode size {} (only 128-byte inodes are supported)",
                self.inode_size
            )));
        }
        if self.feature_compat != 0 {
            return Err(Error::Unsupported(format!(
                "compat features are not supported (0x{:08X})",
                self.feature_compat
            )));
        }
        let incompat = self.feature_incompat & !FEATURE_INCOMPAT_FILETYPE;
        if incompat != 0 {
            return Err(Error::Unsupported(format!(
                "incompatible feature {}",
                feature_names(incompat, &INCOMPAT_NAMES)
            )));
        }
        if self.feature_incompat != FEATURE_INCOMPAT_FILETYPE {
            return Err(Error::Unsupported(
                "the filetype feature is required".into(),
            ));
        }
        let ro_compat = self.feature_ro_compat & !FEATURE_RO_COMPAT_SPARSE_SUPER;
        if ro_compat != 0 {
            return Err(Error::Unsupported(format!(
                "read-only-compatible feature {}",
                feature_names(ro_compat, &RO_COMPAT_NAMES)
            )));
        }
        if self.feature_ro_compat != FEATURE_RO_COMPAT_SPARSE_SUPER {
            return Err(Error::Unsupported(
                "the sparse_super feature is required".into(),
            ));
        }
        if self.first_data_block != 1 {
            return Err(Error::CorruptImage(format!(
                "first data block is {}, not 1",
                self.first_data_block
            )));
        }
        if self.blocks_per_group != BLOCKS_PER_GROUP {
            return Err(Error::CorruptImage(format!(
                "{} blocks per group, not {BLOCKS_PER_GROUP}",
                self.blocks_per_group
            )));
        }
        if !inodes_per_group_is_valid(self.inodes_per_group) {
            return Err(Error::CorruptImage(format!(
                "{} inodes per group is not a multiple of 8 in 16..=8192",
                self.inodes_per_group
            )));
        }
        if self.blocks_count as u64 > disk_blocks {
            return Err(Error::CorruptImage(format!(
                "superblock counts {} blocks but the image holds {disk_blocks}",
                self.blocks_count
            )));
        }
        Ok(())
    }

    /// The volume name with trailing NULs removed, lossily decoded as UTF-8.
    pub fn label(&self) -> String {
        let end = self
            .volume_name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.volume_name.len());
        String::from_utf8_lossy(&self.volume_name[..end]).into_owned()
    }
}

/// Whether a group carries a superblock and descriptor table under
/// `sparse_super`: groups 0 and 1 and every power of 3, 5, and 7.
pub fn has_superblock(group: u32) -> bool {
    fn is_power_of(mut n: u32, base: u32) -> bool {
        if n < base {
            return false;
        }
        while n.is_multiple_of(base) {
            n /= base;
        }
        n == 1
    }
    group <= 1 || is_power_of(group, 3) || is_power_of(group, 5) || is_power_of(group, 7)
}

/// mke2fs's ratio of one inode per 16 KiB, per group:
/// `clamp(round_up(total_blocks * 1024 / 16_384 / groups, 8), 16, 8_192)`.
pub fn default_inodes_per_group(total_blocks: u32, groups: u32) -> u32 {
    let per_group = total_blocks as u64 * BLOCK_SIZE as u64 / 16_384 / groups.max(1) as u64;
    (per_group.div_ceil(8) * 8).clamp(16, 8192) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::Geometry;
    use fs_core::Error;

    fn default_superblock() -> Superblock {
        let options = ExtFormatOptions::default();
        let geo = Geometry::from_options(&options).unwrap();
        Superblock::new(&options, &geo, 315_532_800)
    }

    fn unsupported_message(sb: &Superblock) -> String {
        match sb.validate(16_384) {
            Err(Error::Unsupported(msg)) => msg,
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    #[test]
    fn new_fills_the_default_disk_fields() {
        let sb = default_superblock();
        assert_eq!(sb.inodes_count, 1024);
        assert_eq!(sb.blocks_count, 16_384);
        assert_eq!(sb.free_blocks_count, 8124 + 8123);
        assert_eq!(sb.free_inodes_count, 502 + 512);
        assert_eq!(sb.first_data_block, 1);
        assert_eq!(sb.blocks_per_group, 8192);
        assert_eq!(sb.frags_per_group, 8192);
        assert_eq!(sb.inodes_per_group, 512);
        assert_eq!(
            (sb.mtime, sb.wtime, sb.lastcheck),
            (0, 315_532_800, 315_532_800)
        );
        assert_eq!(sb.max_mnt_count, -1);
        assert_eq!(sb.magic, 0xEF53);
        assert_eq!((sb.state, sb.errors, sb.rev_level), (1, 1, 1));
        assert_eq!((sb.first_ino, sb.inode_size), (11, 128));
        assert_eq!(sb.feature_compat, 0);
        assert_eq!(sb.feature_incompat, 0x0002);
        assert_eq!(sb.feature_ro_compat, 0x0001);
        assert_eq!(sb.uuid, DEFAULT_UUID);
        assert_eq!(sb.volume_name, [0; 16]);
        assert_eq!(sb.label(), "");
        assert_eq!(sb.validate(16_384), Ok(()));
    }

    #[test]
    fn encode_writes_the_spec_offsets_and_decode_round_trips() {
        let mut sb = default_superblock();
        sb.volume_name[..5].copy_from_slice(b"hello");
        sb.block_group_nr = 1;
        let mut bytes = [0xAAu8; 1024];
        sb.encode(&mut bytes);
        assert_eq!(&bytes[0..4], &1024u32.to_le_bytes());
        assert_eq!(&bytes[4..8], &16_384u32.to_le_bytes());
        assert_eq!(&bytes[20..24], &1u32.to_le_bytes());
        assert_eq!(&bytes[40..44], &512u32.to_le_bytes());
        assert_eq!(&bytes[54..56], &[0xFF, 0xFF]);
        assert_eq!(&bytes[56..58], &[0x53, 0xEF]);
        assert_eq!(&bytes[76..80], &1u32.to_le_bytes());
        assert_eq!(&bytes[84..88], &11u32.to_le_bytes());
        assert_eq!(&bytes[88..90], &128u16.to_le_bytes());
        assert_eq!(&bytes[90..92], &1u16.to_le_bytes());
        assert_eq!(&bytes[96..100], &2u32.to_le_bytes());
        assert_eq!(&bytes[100..104], &1u32.to_le_bytes());
        assert_eq!(&bytes[104..108], &[0xe2, 0xf5, 0xee, 0x00]);
        assert_eq!(&bytes[120..125], b"hello");
        assert!(bytes[136..].iter().all(|&b| b == 0));
        let back = Superblock::decode(&bytes);
        assert_eq!(back, sb);
        assert_eq!(back.label(), "hello");
    }

    #[test]
    fn validate_reports_each_corrupt_image_reason() {
        let corrupt = |sb: &Superblock, disk_blocks: u64| {
            matches!(sb.validate(disk_blocks), Err(Error::CorruptImage(_)))
        };
        let good = default_superblock();
        assert!(corrupt(
            &Superblock {
                magic: 0x1234,
                ..good.clone()
            },
            16_384
        ));
        assert!(corrupt(&good, 16_383));
        assert!(corrupt(
            &Superblock {
                first_data_block: 0,
                ..good.clone()
            },
            16_384
        ));
        assert!(corrupt(
            &Superblock {
                blocks_per_group: 32_768,
                ..good.clone()
            },
            16_384
        ));
        assert!(corrupt(
            &Superblock {
                inodes_per_group: 12,
                ..good.clone()
            },
            16_384
        ));
        assert!(corrupt(
            &Superblock {
                inodes_per_group: 8,
                ..good.clone()
            },
            16_384
        ));
        assert!(corrupt(
            &Superblock {
                inodes_per_group: 8200,
                ..good.clone()
            },
            16_384
        ));
        assert_eq!(good.validate(20_000), Ok(()));
    }

    #[test]
    fn validate_reports_each_unsupported_reason_by_name() {
        let good = default_superblock();
        let msg = unsupported_message(&Superblock {
            rev_level: 0,
            ..good.clone()
        });
        assert!(msg.contains("revision 0"), "{msg}");
        let msg = unsupported_message(&Superblock {
            log_block_size: 2,
            ..good.clone()
        });
        assert!(msg.contains("block size"), "{msg}");
        let msg = unsupported_message(&Superblock {
            inode_size: 256,
            ..good.clone()
        });
        assert!(msg.contains("inode size 256"), "{msg}");
        let msg = unsupported_message(&Superblock {
            feature_incompat: FEATURE_INCOMPAT_FILETYPE | 0x0040,
            ..good.clone()
        });
        assert!(msg.contains("extent"), "{msg}");
        let msg = unsupported_message(&Superblock {
            feature_incompat: FEATURE_INCOMPAT_FILETYPE | 0x4000_0000,
            ..good.clone()
        });
        assert!(msg.contains("0x40000000"), "{msg}");
        let msg = unsupported_message(&Superblock {
            feature_ro_compat: FEATURE_RO_COMPAT_SPARSE_SUPER | 0x0002,
            ..good.clone()
        });
        assert!(msg.contains("large_file"), "{msg}");
        // Unsupported is reported before the geometry of a 4 KiB-block image.
        let msg = unsupported_message(&Superblock {
            log_block_size: 2,
            first_data_block: 0,
            blocks_per_group: 32_768,
            ..good.clone()
        });
        assert!(msg.contains("block size"), "{msg}");
        // A superblock without FILETYPE or SPARSE_SUPER is rejected: the
        // derived layout depends on both being set exactly.
        let bare = Superblock {
            feature_incompat: 0,
            feature_ro_compat: 0,
            ..good
        };
        assert!(matches!(bare.validate(16_384), Err(Error::Unsupported(_))));
    }

    #[test]
    fn validate_requires_exactly_the_filetype_incompat_feature() {
        let good = default_superblock();
        let msg = unsupported_message(&Superblock {
            feature_incompat: 0,
            ..good
        });
        assert_eq!(msg, "the filetype feature is required");
    }

    #[test]
    fn validate_requires_exactly_the_sparse_super_ro_compat_feature() {
        let good = default_superblock();
        let msg = unsupported_message(&Superblock {
            feature_ro_compat: 0,
            ..good
        });
        assert_eq!(msg, "the sparse_super feature is required");
    }

    #[test]
    fn validate_rejects_any_compat_feature() {
        let good = default_superblock();
        let msg = unsupported_message(&Superblock {
            feature_compat: 0x0004,
            ..good
        });
        assert_eq!(msg, "compat features are not supported (0x00000004)");
    }

    #[test]
    fn has_superblock_follows_sparse_super_for_groups_0_to_30() {
        let with: Vec<u32> = (0..=30).filter(|&g| has_superblock(g)).collect();
        assert_eq!(with, vec![0, 1, 3, 5, 7, 9, 25, 27]);
        assert!(has_superblock(49) && has_superblock(81) && has_superblock(125));
        assert!(!has_superblock(15) && !has_superblock(45));
    }

    #[test]
    fn default_inodes_per_group_is_one_per_16_kib_rounded_and_clamped() {
        assert_eq!(default_inodes_per_group(16_384, 2), 512);
        assert_eq!(default_inodes_per_group(262_144, 32), 512);
        assert_eq!(default_inodes_per_group(64, 1), 16);
        assert_eq!(default_inodes_per_group(8_200, 2), 256);
        assert_eq!(default_inodes_per_group(1_100, 1), 72);
    }

    #[test]
    fn options_default_to_the_16_mib_disk() {
        let o = ExtFormatOptions::default();
        assert_eq!(o.total_blocks, 16_384);
        assert_eq!(o.inodes_per_group, None);
        assert_eq!(o.label, [0; 16]);
        assert_eq!(o.uuid, DEFAULT_UUID);
    }
}
