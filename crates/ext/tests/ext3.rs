//! ext3: the journal on inode 8 (spec `2026-09-24-ext3-journal-design.md`).

mod common;

/// Task 3: format, load, validation, layout, owners, and the gate.
mod format {
    use super::common::ext3;
    use ext::blockmap;
    use ext::{
        BlockRole, ExtFormatOptions, ExtFs, JournalInfo, JournalMode, JournalOptions,
        JournalSuperblock, Superblock, DEFAULT_UUID,
    };
    use fs_core::{Error, RegionKind};

    /// 1980-01-01 00:00:00 UTC, the default `now`, as ext stores it.
    const EPOCH_1980: u32 = 315_532_800;
    /// The journal superblock (journal index 0) on the default disk.
    const JSB: usize = 82 * 1024;
    /// Inode 8's slot: the inode table starts at block 5, 128 bytes each.
    const INODE_8: usize = 5 * 1024 + 7 * 128;
    const BOTH: [JournalMode; 2] = [JournalMode::Ordered, JournalMode::Data];
    /// FNV-1a (64-bit) of the default ext2 image.
    const EXT2_IMAGE_FNV: u64 = 1_437_322_056_522_707_274;

    fn put_u32(image: &mut [u8], offset: usize, value: u32) {
        image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_be32(image: &mut [u8], offset: usize, value: u32) {
        image[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }

    fn with_journal(total_blocks: u32, blocks: Option<u32>) -> fs_core::Result<ExtFs> {
        ExtFs::format(ExtFormatOptions {
            total_blocks,
            journal: Some(JournalOptions {
                blocks,
                mode: JournalMode::Ordered,
            }),
            ..Default::default()
        })
    }

    /// The default ordered-mode image with `patch` applied, loaded again.
    fn planted(patch: impl FnOnce(&mut Vec<u8>)) -> fs_core::Result<ExtFs> {
        let mut image = ext3(JournalMode::Ordered).disk().as_bytes().to_vec();
        patch(&mut image);
        ExtFs::from_image(image)
    }

    fn unsupported(patch: impl FnOnce(&mut Vec<u8>)) -> String {
        match planted(patch) {
            Err(Error::Unsupported(msg)) => msg,
            other => panic!("expected Unsupported, got {:?}", other.err()),
        }
    }

    fn corrupt(patch: impl FnOnce(&mut Vec<u8>)) -> String {
        match planted(patch) {
            Err(Error::CorruptImage(msg)) => msg,
            other => panic!("expected CorruptImage, got {:?}", other.err()),
        }
    }

    #[test]
    fn format_takes_1029_blocks_from_group_0_in_both_modes() {
        for mode in BOTH {
            let fs = ext3(mode);
            let sb = fs.superblock();
            assert_eq!(sb.free_blocks_count, 15_205, "{mode:?}");
            assert_eq!(sb.free_inodes_count, 1_013);
            let gds = fs.group_descriptors();
            assert_eq!(
                (
                    gds[0].free_blocks_count,
                    gds[0].free_inodes_count,
                    gds[0].used_dirs_count
                ),
                (7_082, 501, 2)
            );
            assert_eq!(
                (
                    gds[1].free_blocks_count,
                    gds[1].free_inodes_count,
                    gds[1].used_dirs_count
                ),
                (8_123, 512, 0)
            );
            assert!(fs.history().is_empty());
            assert_eq!(fs.fs_type(), "ext3");
            assert!(!fs.needs_recovery());
            assert_eq!(fs.journal_mode(), Some(mode));
            // Group 0's block bitmap: blocks 1..=1110 used, 1111 free.
            let bits = fs.disk().sector(3);
            let used = |block: usize| bits[(block - 1) / 8] & (1 << ((block - 1) % 8)) != 0;
            assert!((1..=1_110).all(used));
            assert!(!used(1_111));
        }
    }

    #[test]
    fn inode_8_maps_82_to_1105_with_pointer_blocks_1106_to_1110() {
        let fs = ext3(JournalMode::Ordered);
        let inode = fs.inode(8).unwrap();
        assert_eq!(inode.mode, 0x8180);
        assert_eq!((inode.uid, inode.gid, inode.flags), (0, 0, 0));
        assert_eq!(inode.size, 1_024 * 1_024);
        assert_eq!(inode.links_count, 1);
        assert_eq!(inode.blocks, 1_029 * 2);
        assert_eq!(
            (inode.atime, inode.ctime, inode.mtime, inode.dtime),
            (EPOCH_1980, EPOCH_1980, EPOCH_1980, 0)
        );
        assert_eq!(
            blockmap::file_blocks(fs.disk(), &inode),
            (82..1_106).collect::<Vec<u32>>()
        );
        assert_eq!(
            blockmap::indirect_blocks(fs.disk(), &inode),
            vec![(1_106, 1), (1_107, 2), (1_108, 1), (1_109, 1), (1_110, 1)]
        );
    }

    #[test]
    fn journal_index_0_holds_the_format_time_superblock_and_the_log_is_zero() {
        let fs = ext3(JournalMode::Data);
        let jsb = JournalSuperblock::decode(fs.disk().sector(82)).unwrap();
        assert_eq!(jsb, JournalSuperblock::new(1_024, DEFAULT_UUID));
        assert_eq!(
            &fs.disk().sector(82)[..0x20],
            &[
                0xC0, 0x3B, 0x39, 0x98, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 4, 0, //
                0, 0, 4, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0,
            ]
        );
        assert!((83..1_106).all(|b| fs.disk().sector(b).iter().all(|&x| x == 0)));
    }

    #[test]
    fn every_superblock_copy_carries_the_journal_fields() {
        for (mode, opts) in [(JournalMode::Ordered, 0x0040), (JournalMode::Data, 0x0020)] {
            let fs = ext3(mode);
            let inode = fs.inode(8).unwrap();
            for (block, group) in [(1u64, 0u16), (8_193, 1)] {
                let sb = Superblock::decode(fs.disk().sector(block));
                assert_eq!(sb.block_group_nr, group);
                assert_eq!(
                    Superblock {
                        block_group_nr: 0,
                        ..sb.clone()
                    },
                    *fs.superblock()
                );
                assert_eq!(sb.feature_compat, 0x0004);
                assert_eq!(sb.feature_incompat, 0x0002);
                assert_eq!(sb.feature_ro_compat, 0x0001);
                assert_eq!(sb.journal_inum, 8);
                assert_eq!(sb.journal_uuid, [0; 16]);
                assert_eq!((sb.journal_dev, sb.last_orphan), (0, 0));
                assert_eq!(sb.default_mount_opts, opts, "{mode:?}");
                assert_eq!(sb.jnl_backup_type, 1);
                assert_eq!(sb.jnl_blocks[..15], inode.block);
                assert_eq!((sb.jnl_blocks[15], sb.jnl_blocks[16]), (0, 1_048_576));
                assert_eq!(
                    (sb.free_blocks_count, sb.free_inodes_count),
                    (15_205, 1_013)
                );
            }
            // The raw bytes of the primary copy.
            let raw = fs.disk().sector(1);
            assert_eq!(&raw[0x5C..0x60], &[4, 0, 0, 0]);
            assert_eq!(&raw[0xE0..0xE4], &[8, 0, 0, 0]);
            assert_eq!(raw[0xFD], 1);
            assert_eq!(&raw[0x100..0x104], &[opts as u8, 0, 0, 0]);
            assert_eq!(&raw[0x10C..0x110], &82u32.to_le_bytes());
            assert_eq!(&raw[0x13C..0x140], &1_106u32.to_le_bytes());
            assert_eq!(&raw[0x140..0x144], &1_107u32.to_le_bytes());
            assert_eq!(&raw[0x14C..0x150], &1_048_576u32.to_le_bytes());
            // The backup descriptor table matches the primary one.
            assert_eq!(fs.disk().sector(2), fs.disk().sector(8_194));
        }
    }

    #[test]
    fn journal_info_describes_the_fresh_journal() {
        for mode in BOTH {
            assert_eq!(
                ext3(mode).journal_info(),
                Some(JournalInfo {
                    inode: 8,
                    maxlen: 1_024,
                    first_block: 82,
                    sequence: 1,
                    start: 0,
                    head: 1,
                    mode,
                    needs_recovery: false,
                    max_transaction: 256,
                })
            );
        }
    }

    #[test]
    fn from_image_opens_the_journal_it_formatted() {
        for mode in BOTH {
            let fs = ext3(mode);
            let again = ExtFs::from_image(fs.disk().as_bytes().to_vec()).unwrap();
            assert_eq!(again.fs_type(), "ext3");
            assert_eq!(again.journal_info(), fs.journal_info());
            assert_eq!(again.journal_mode(), Some(mode));
            assert_eq!(again.layout(), fs.layout());
            assert_eq!(again.block_owners(), fs.block_owners());
            assert!(again.corruption().is_none());
        }
    }

    #[test]
    fn a_set_needs_recovery_flag_loads_without_replaying() {
        let mut image = ext3(JournalMode::Ordered).disk().as_bytes().to_vec();
        put_u32(&mut image, 1_024 + 0x60, 0x0006);
        let fs = ExtFs::from_image(image.clone()).unwrap();
        assert!(fs.needs_recovery());
        assert!(fs.journal_info().unwrap().needs_recovery);
        assert_eq!(fs.disk().as_bytes(), &image[..]);
        assert!(fs.history().is_empty());
    }

    #[test]
    fn inode_8_is_never_exposed_by_the_tree() {
        for mode in BOTH {
            let fs = ext3(mode);
            let names: Vec<String> = fs
                .list_dir("/")
                .unwrap()
                .into_iter()
                .map(|e| e.name)
                .collect();
            assert_eq!(names, ["lost+found"]);
            for dir in ["/", "/lost+found"] {
                let inodes: Vec<u32> = fs
                    .dir_entries(dir)
                    .unwrap()
                    .into_iter()
                    .map(|(_, _, entry)| entry.inode)
                    .collect();
                assert!(!inodes.contains(&8), "{dir} lists {inodes:?}");
                assert_ne!(fs.lookup(dir), Ok(8));
            }
            for path in ["/<journal>", "/<8>", "/journal"] {
                assert_eq!(fs.lookup(path), Err(Error::NotFound), "{path}");
                assert_eq!(fs.stat(path), Err(Error::NotFound), "{path}");
                assert_eq!(fs.read_file(path), Err(Error::NotFound), "{path}");
            }
        }
    }

    #[test]
    fn the_journal_size_follows_mke2fs() {
        assert_eq!(
            with_journal(2_047, None).err(),
            Some(Error::InvalidGeometry(
                "a journal needs at least 2048 blocks".into()
            ))
        );
        assert_eq!(
            with_journal(2_047, Some(1_024)).err(),
            Some(Error::InvalidGeometry(
                "a journal needs at least 2048 blocks".into()
            ))
        );
        let small = with_journal(2_048, None).unwrap();
        assert_eq!(small.journal_info().unwrap().maxlen, 1_024);
        let fs = with_journal(4_096, None).unwrap();
        assert_eq!(fs.journal_info().unwrap().maxlen, 1_024);
        assert_eq!(
            with_journal(16_384, Some(1_023)).err(),
            Some(Error::InvalidGeometry(
                "journal size must be at least 1024 blocks".into()
            ))
        );
        // Half the 16,234 free blocks of the fresh ext2 layout is 8,117.
        assert_eq!(
            with_journal(16_384, Some(8_118)).err(),
            Some(Error::InvalidGeometry(
                "journal size too big for the volume".into()
            ))
        );
        let fs = with_journal(16_384, Some(8_117)).unwrap();
        let info = fs.journal_info().unwrap();
        assert_eq!((info.maxlen, info.max_transaction), (8_117, 2_029));
        assert_eq!(fs.inode(8).unwrap().size, 8_117 * 1_024);
    }

    #[test]
    fn a_262144_block_volume_gets_8192_journal_blocks() {
        let fs = with_journal(262_144, None).unwrap();
        let info = fs.journal_info().unwrap();
        assert_eq!(info.maxlen, 8_192);
        // mke2fs's goal: the group of block 131,071, group 15.
        assert_eq!(fs.geometry().group_of_block(info.first_block), 15);
    }

    #[test]
    fn ext2_is_unchanged() {
        let mut fs = ExtFs::format(ExtFormatOptions::default()).unwrap();
        assert_eq!(fs.fs_type(), "ext2");
        assert_eq!(fs.journal_info(), None);
        assert_eq!(fs.journal_mode(), None);
        assert!(!fs.needs_recovery());
        let sb = fs.superblock();
        assert_eq!((sb.feature_compat, sb.journal_inum), (0, 0));
        assert_eq!((sb.default_mount_opts, sb.jnl_backup_type), (0, 0));
        assert_eq!(sb.jnl_blocks, [0; 17]);
        assert_eq!(sb.free_blocks_count, 16_234);
        assert!(fs.disk().sector(1)[0xD0..0x150].iter().all(|&b| b == 0));
        assert!(fs.inode(8).unwrap().is_free());
        assert!(fs
            .block_owners()
            .values()
            .all(|o| o.role != BlockRole::Journal));
        assert!(fs.layout().iter().all(|r| r.kind != RegionKind::Journal));
        // The whole image is the one slice 2 formatted (FNV-1a over every
        // byte, taken on the base branch before this task).
        let hash = fs
            .disk()
            .as_bytes()
            .iter()
            .fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
                (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
            });
        assert_eq!(hash, EXT2_IMAGE_FNV);
        // A raw write where ext3 keeps its journal superblock is plain data.
        fs.write_raw(JSB as u64, &[0; 4]).unwrap();
        assert!(fs.corruption().is_none());
    }

    #[test]
    fn validate_accepts_only_the_ext3_feature_variants() {
        let msg = unsupported(|i| put_u32(i, 1_024 + 0x5C, 0x0004 | 0x0010));
        assert_eq!(msg, "compat features are not supported: resize_inode");
        let mut ext2 = ExtFs::format(ExtFormatOptions::default())
            .unwrap()
            .disk()
            .as_bytes()
            .to_vec();
        put_u32(&mut ext2, 1_024 + 0x60, 0x0006);
        assert_eq!(
            ExtFs::from_image(ext2).err(),
            Some(Error::CorruptImage(
                "needs_recovery is set but the volume has no journal".into()
            ))
        );
    }

    #[test]
    fn an_external_or_foreign_journal_inode_is_unsupported() {
        assert_eq!(
            unsupported(|i| put_u32(i, 1_024 + 0xE0, 9)),
            "journal inode 9 (only the reserved inode 8 is supported)"
        );
        assert_eq!(unsupported(|i| i[1_024 + 0xD0] = 1), "external journal");
        assert_eq!(
            unsupported(|i| put_u32(i, 1_024 + 0xE4, 0x0801)),
            "external journal"
        );
    }

    #[test]
    fn a_damaged_journal_inode_is_corrupt_or_too_small() {
        assert_eq!(
            corrupt(|i| i[INODE_8 + 1] = 0x41),
            "journal inode 8 is not a regular file (mode 0x4180)"
        );
        assert_eq!(
            corrupt(|i| put_u32(i, INODE_8 + 4, 1_048_577)),
            "journal inode 8 has size 1048577, not a whole number of blocks"
        );
        assert_eq!(
            corrupt(|i| put_u32(i, INODE_8 + 40 + 5 * 4, 0)),
            "journal inode 8 maps only 5 of its 1024 blocks inside the volume"
        );
        assert_eq!(
            corrupt(|i| put_u32(i, INODE_8 + 40 + 12 * 4, 20_000)),
            "journal inode 8 maps only 12 of its 1024 blocks inside the volume"
        );
        assert_eq!(
            unsupported(|i| put_u32(i, INODE_8 + 4, 1_023 * 1_024)),
            "journal of 1023 blocks is below the 1024-block minimum"
        );
    }

    #[test]
    fn the_journal_superblock_is_checked_field_by_field() {
        assert_eq!(
            corrupt(|i| put_be32(i, JSB, 0)),
            "journal superblock magic is 0x00000000"
        );
        assert_eq!(
            unsupported(|i| put_be32(i, JSB + 0x04, 3)),
            "journal superblock version 3"
        );
        assert_eq!(
            corrupt(|i| put_be32(i, JSB + 0x0C, 4_096)),
            "journal superblock s_blocksize is 4096, not 1024"
        );
        assert_eq!(
            corrupt(|i| put_be32(i, JSB + 0x10, 1_000)),
            "journal superblock s_maxlen is 1000 but the journal inode maps 1024 blocks"
        );
        assert_eq!(
            corrupt(|i| put_be32(i, JSB + 0x14, 2)),
            "journal superblock s_first is 2, not 1"
        );
        assert_eq!(
            unsupported(|i| put_be32(i, JSB + 0x24, 0x1)),
            "journal feature journal_checksum"
        );
        assert_eq!(
            unsupported(|i| put_be32(i, JSB + 0x28, 0x2)),
            "journal feature journal_64bit"
        );
        assert_eq!(
            unsupported(|i| put_be32(i, JSB + 0x28, 0x10)),
            "journal feature journal_checksum_v3"
        );
        assert_eq!(
            corrupt(|i| put_be32(i, JSB + 0x1C, 1_024)),
            "journal superblock s_start is 1024, outside the log 1..1024"
        );
    }

    #[test]
    fn the_mount_options_choose_the_mode_and_refuse_writeback() {
        assert_eq!(
            unsupported(|i| put_u32(i, 1_024 + 0x100, 0x0060)),
            "writeback journaling"
        );
        let fs = planted(|i| put_u32(i, 1_024 + 0x100, 0)).unwrap();
        assert_eq!(fs.journal_mode(), Some(JournalMode::Ordered));
        let fs = planted(|i| put_u32(i, 1_024 + 0x100, 0x0020 | 0x000C)).unwrap();
        assert_eq!(fs.journal_mode(), Some(JournalMode::Data));
    }

    #[test]
    fn the_checks_run_in_the_spec_order() {
        // Item 1 before item 2: a foreign inode number hides a broken inode 8.
        assert_eq!(
            unsupported(|i| {
                put_u32(i, 1_024 + 0xE0, 9);
                i[INODE_8 + 1] = 0x41;
            }),
            "journal inode 9 (only the reserved inode 8 is supported)"
        );
        // Item 2 before item 3.
        assert_eq!(
            corrupt(|i| {
                put_u32(i, INODE_8 + 4, 1_048_577);
                put_be32(i, JSB, 0);
            }),
            "journal inode 8 has size 1048577, not a whole number of blocks"
        );
        // Item 4 before item 5, item 5 before item 6.
        assert_eq!(
            unsupported(|i| {
                put_be32(i, JSB + 0x28, 0x1);
                put_u32(i, 1_024 + 0x100, 0x0060);
            }),
            "journal feature journal_incompat_revoke"
        );
        assert_eq!(
            unsupported(|i| {
                put_u32(i, 1_024 + 0x100, 0x0060);
                put_be32(i, JSB + 0x1C, 5_000);
            }),
            "writeback journaling"
        );
    }

    #[test]
    fn layout_carves_the_journal_out_of_group_0_data() {
        let fs = ext3(JournalMode::Ordered);
        let got: Vec<(String, u64, u64, RegionKind)> = fs
            .layout()
            .into_iter()
            .map(|r| (r.name, r.sectors.start, r.sectors.end, r.kind))
            .collect();
        let group0: Vec<(String, u64, u64, RegionKind)> = [
            ("inode table (group 0)", 5, 69, RegionKind::Metadata),
            ("data (group 0)", 69, 82, RegionKind::Data),
            ("journal", 82, 1_106, RegionKind::Journal),
            ("data (group 0)", 1_106, 8_193, RegionKind::Data),
            (
                "backup superblock (group 1)",
                8_193,
                8_194,
                RegionKind::Boot,
            ),
        ]
        .into_iter()
        .map(|(n, s, e, k)| (n.to_string(), s, e, k))
        .collect();
        assert_eq!(got.len(), 15);
        assert_eq!(got[5..10], group0[..]);
        assert_eq!(
            got.last().unwrap(),
            &(
                "data (group 1)".to_string(),
                8_261,
                16_384,
                RegionKind::Data
            )
        );
    }

    #[test]
    fn block_owners_list_the_journal_blocks_under_inode_8() {
        let fs = ext3(JournalMode::Ordered);
        let owners = fs.block_owners();
        assert_eq!(owners.len(), 13 + 1_029);
        let journal = |role| ext::BlockOwner {
            inode: 8,
            path: "<journal>".into(),
            role,
        };
        assert!((82..1_106).all(|b| owners[&b] == journal(BlockRole::Journal)));
        assert!((1_106..=1_110).all(|b| owners[&b] == journal(BlockRole::Indirect)));
        assert_eq!(owners[&69].path, "/");
    }

    /// The gate's message for a journal that no longer opens.
    fn gate(cause: &str) -> Option<Error> {
        Some(Error::CorruptImage(format!(
            "superblock or group descriptors no longer parse after a raw write: {cause}"
        )))
    }

    #[test]
    fn a_raw_write_that_breaks_the_journal_superblock_trips_the_gate() {
        let mut fs = ext3(JournalMode::Ordered);
        let info = fs.journal_info();
        let layout = fs.layout();
        fs.write_raw(JSB as u64, &[0; 4]).unwrap();
        assert_eq!(
            fs.corruption().cloned(),
            gate("journal superblock magic is 0x00000000")
        );
        assert!(fs.list_dir("/").is_err());
        assert!(fs.create_file("/x", b"").is_err());
        assert_eq!(fs.layout(), layout, "layout keeps the last good journal");
        fs.write_raw(JSB as u64, &[0xC0, 0x3B, 0x39, 0x98]).unwrap();
        assert!(fs.corruption().is_none());
        assert_eq!(fs.journal_info(), info);
        assert_eq!(fs.list_dir("/").unwrap().len(), 1);
    }

    #[test]
    fn raw_writes_to_inode_8_and_its_pointer_blocks_trip_the_gate() {
        let mut fs = ext3(JournalMode::Ordered);
        fs.write_raw(INODE_8 as u64 + 1, &[0x41]).unwrap();
        assert_eq!(
            fs.corruption().cloned(),
            gate("journal inode 8 is not a regular file (mode 0x4180)")
        );
        fs.write_raw(INODE_8 as u64 + 1, &[0x81]).unwrap();
        assert!(fs.corruption().is_none());

        let pointers = fs.disk().sector(1_106).to_vec();
        fs.write_raw(1_106 * 1_024, &[0; 1_024]).unwrap();
        assert_eq!(
            fs.corruption().cloned(),
            gate("journal inode 8 maps only 12 of its 1024 blocks inside the volume")
        );
        fs.write_raw(1_106 * 1_024, &pointers).unwrap();
        assert!(fs.corruption().is_none());

        // A second-level pointer block of the double-indirect tree.
        fs.write_raw(1_110 * 1_024, &[0; 4]).unwrap();
        assert_eq!(
            fs.corruption().cloned(),
            gate("journal inode 8 maps only 780 of its 1024 blocks inside the volume")
        );
        fs.write_raw(1_110 * 1_024, &862u32.to_le_bytes()).unwrap();
        assert!(fs.corruption().is_none());
    }

    #[test]
    fn raw_writes_to_the_log_or_a_valid_flag_edit_are_adopted() {
        let mut fs = ext3(JournalMode::Ordered);
        fs.write_raw(90 * 1_024, &super::common::pattern(1_024))
            .unwrap();
        assert!(fs.corruption().is_none());
        fs.write_raw(1_024 + 0x60, &0x0006u32.to_le_bytes())
            .unwrap();
        assert!(fs.corruption().is_none());
        assert!(fs.needs_recovery());
        fs.write_raw(1_024 + 0x100, &0x0020u32.to_le_bytes())
            .unwrap();
        assert_eq!(fs.journal_mode(), Some(JournalMode::Data));
        // Clearing has_journal turns the volume into ext2 (and clearing the
        // flag first keeps the superblock valid).
        fs.write_raw(1_024 + 0x60, &0x0002u32.to_le_bytes())
            .unwrap();
        fs.write_raw(1_024 + 0x5C, &0u32.to_le_bytes()).unwrap();
        assert!(fs.corruption().is_none());
        assert_eq!(fs.fs_type(), "ext2");
        assert_eq!(fs.journal_info(), None);
    }
}

/// Task 4: every mutation as one journal transaction (spec section 5) and
/// the armed crash (spec section 6).
mod transactions {
    use super::common::{changes_in_order, ext3, mask_journal, pattern};
    use ext::blockmap;
    use ext::{
        decode_commit, decode_descriptor, BlockRole, CrashPhase, ExtFormatOptions, ExtFs,
        JournalInfo, JournalMode, TAG_ESCAPE, TAG_LAST, TAG_SAME_UUID,
    };
    use fs_core::{DateTime, Error, OpRecord, Result};
    use std::collections::BTreeSet;

    const BS: usize = 1024;
    const BOTH: [JournalMode; 2] = [JournalMode::Ordered, JournalMode::Data];
    /// `s_feature_incompat` of the primary superblock.
    const FLAG: usize = 1024 + 0x60;
    /// `s_sequence` and `s_start` of the journal superblock (block 82).
    const JSB_SEQ: usize = 82 * 1024 + 0x18;
    /// Journal index `i` is physical block `82 + i` on the default disk.
    const JOURNAL_BLOCK_0: u32 = 82;
    const MAGIC: [u8; 4] = [0xC0, 0x3B, 0x39, 0x98];
    /// The event kinds a transaction adds to the body's own.
    const JOURNAL_KINDS: [&str; 7] = [
        "recovery_flag_set",
        "transaction_started",
        "journal_block_written",
        "checkpointed",
        "journal_emptied",
        "recovery_flag_cleared",
        "crashed",
    ];

    fn be(a: u32, b: u32) -> Vec<u8> {
        [a.to_be_bytes(), b.to_be_bytes()].concat()
    }

    /// `(block * 1024, 1024)` for each block: whole-block writes.
    fn whole(blocks: impl IntoIterator<Item = u32>) -> Vec<(usize, usize)> {
        blocks.into_iter().map(|b| (b as usize * BS, BS)).collect()
    }

    fn texts(record: &OpRecord) -> Vec<String> {
        record.events.iter().map(|e| e.to_string()).collect()
    }

    fn count(record: &OpRecord, kind: &str) -> usize {
        record.event_kinds().iter().filter(|&&k| k == kind).count()
    }

    /// The journal ring of the default 1024-block journal.
    fn wrap(index: u32) -> u32 {
        if index >= 1024 {
            index - 1023
        } else {
            index
        }
    }

    /// The ext2 volume with `fs`'s exact layout: the same image with
    /// `has_journal` cleared, so it allocates the same blocks and inodes
    /// and its records are what the ext3 body writes before the post-pass.
    fn ext2_twin(fs: &ExtFs) -> ExtFs {
        let mut image = fs.disk().as_bytes().to_vec();
        image[1024 + 0x5C] &= !0x04;
        let mut twin = ExtFs::from_image(image).unwrap();
        assert_eq!(twin.fs_type(), "ext2");
        twin.set_now(fs.now());
        twin
    }

    /// Check an ext3 record against spec 5.2 step by step: `draft` is the
    /// ext2 twin's record of the same operation (the body's own writes and
    /// events), `before` the journal before it. Returns the tagged blocks.
    fn assert_follows_5_2(
        fs: &ExtFs,
        rec: &OpRecord,
        draft: &OpRecord,
        before: &JournalInfo,
    ) -> Vec<u32> {
        let mode = fs.journal_mode().unwrap();
        let owners = fs.block_owners();
        let is_data = |b: u32| owners.get(&b).is_some_and(|o| o.role == BlockRole::Data);
        // The oracle's own classification of the draft (spec 5.1 step 3).
        let mut tagged = BTreeSet::new();
        let mut data: Vec<(usize, Vec<u8>)> = Vec::new();
        for c in &draft.changes {
            let end = c.offset + c.after.len();
            let mut at = c.offset;
            while at < end {
                let block = (at / BS) as u32;
                let stop = end.min((at / BS + 1) * BS);
                if is_data(block) {
                    data.push((at, c.after[at - c.offset..stop - c.offset].to_vec()));
                    if mode == JournalMode::Data {
                        tagged.insert(block);
                    }
                } else {
                    tagged.insert(block);
                }
                at = stop;
            }
        }
        let tagged: Vec<u32> = tagged.into_iter().collect();
        let n = tagged.len();
        let (tid, start) = (before.sequence, before.head);
        let block_of = |index: u32| JOURNAL_BLOCK_0 + index;
        let ordered = mode == JournalMode::Ordered;

        let mut want_changes = vec![(FLAG, 4)];
        if ordered {
            want_changes.extend(data.iter().map(|(at, bytes)| (*at, bytes.len())));
        }
        want_changes.push((JSB_SEQ, 8));
        let mut want_events = texts(draft);
        want_events.push("set needs_recovery in the superblock".into());
        want_events.push(format!(
            "started transaction {tid} at journal block {start} ({n} tagged block{})",
            if n == 1 { "" } else { "s" }
        ));
        let mut index = start;
        let mut copies = Vec::new();
        for chunk in tagged.chunks(122) {
            want_changes.extend(whole([block_of(index)]));
            want_events.push(format!(
                "wrote descriptor for transaction {tid} at journal block {index} (block {})",
                block_of(index)
            ));
            for &home in chunk {
                index = wrap(index + 1);
                let escaped = fs.disk().read(home as usize * BS, 4) == MAGIC;
                want_changes.extend(whole([block_of(index)]));
                want_events.push(format!(
                    "copied block {home} into journal block {index} (block {}){}",
                    block_of(index),
                    if escaped { " (escaped)" } else { "" }
                ));
                copies.push(index);
            }
            index = wrap(index + 1);
        }
        let commit = index;
        want_changes.extend(whole([block_of(commit)]));
        want_events.push(format!(
            "committed transaction {tid} at journal block {commit} (block {})",
            block_of(commit)
        ));
        for (&home, &from) in tagged.iter().zip(&copies) {
            want_changes.extend(whole([home]));
            want_events.push(format!(
                "checkpointed block {home} from journal block {from}"
            ));
        }
        want_changes.push((JSB_SEQ, 8));
        want_events.push(format!("journal emptied; next transaction {}", tid + 1));
        want_changes.push((FLAG, 4));
        want_events.push("cleared needs_recovery in the superblock".into());
        assert_eq!(changes_in_order(rec), want_changes, "{}", rec.op);
        assert_eq!(texts(rec), want_events, "{}", rec.op);

        // The bytes of every step.
        let c = &rec.changes;
        assert_eq!(
            (&c[0].before[..], &c[0].after[..]),
            (&[2, 0, 0, 0][..], &[6, 0, 0, 0][..])
        );
        let last = c.last().unwrap();
        assert_eq!(
            (&last.before[..], &last.after[..]),
            (&[6, 0, 0, 0][..], &[2, 0, 0, 0][..])
        );
        let mut pos = 1;
        if ordered {
            for (at, bytes) in &data {
                assert_eq!((c[pos].offset, &c[pos].after), (*at, bytes));
                pos += 1;
            }
        }
        assert_eq!(
            (c[pos].before.clone(), c[pos].after.clone()),
            (be(tid, 0), be(tid, start))
        );
        pos += 1;
        // The after-image of `home` as the checkpoint writes it: the final
        // bytes, with needs_recovery still set in the superblock's.
        let after_image = |home: u32| {
            let mut image = fs.disk().read(home as usize * BS, BS).to_vec();
            if home == 1 {
                image[0x60] |= 0x04;
            }
            image
        };
        for chunk in tagged.chunks(122) {
            let tags = decode_descriptor(&c[pos].after).unwrap();
            assert_eq!(tags.iter().map(|t| t.block).collect::<Vec<_>>(), chunk);
            for (i, tag) in tags.iter().enumerate() {
                let same = if i > 0 { TAG_SAME_UUID } else { 0 };
                let last = if i + 1 == tags.len() { TAG_LAST } else { 0 };
                assert_eq!(tag.flags & !TAG_ESCAPE, same | last, "tag {i}");
            }
            pos += 1;
            for tag in &tags {
                let mut copy = after_image(tag.block);
                if tag.flags & TAG_ESCAPE != 0 {
                    copy[..4].fill(0);
                }
                assert!(c[pos].after == copy, "copy of block {}", tag.block);
                pos += 1;
            }
        }
        assert_eq!(
            decode_commit(&c[pos].after),
            Some(fs.now().to_unix_seconds().max(1) as u64)
        );
        pos += 1;
        for &home in &tagged {
            assert!(c[pos].after == after_image(home), "checkpoint of {home}");
            pos += 1;
        }
        assert_eq!(
            (c[pos].before.clone(), c[pos].after.clone()),
            (be(tid, start), be(tid + 1, 0))
        );

        let info = fs.journal_info().unwrap();
        assert_eq!(
            (info.sequence, info.head, info.start, info.needs_recovery),
            (tid + 1, wrap(commit + 1), 0, false),
            "{}",
            rec.op
        );
        tagged
    }

    /// One mutation of the script below.
    #[derive(Debug, Clone, Copy)]
    enum Op {
        Create(&'static str, usize),
        /// An empty file in the directory whose name is the character
        /// repeated 255 times: an entry of 264 bytes.
        CreateLong(&'static str, char),
        Write(&'static str, usize),
        Delete(&'static str),
        Mkdir(&'static str),
        Rmdir(&'static str),
    }

    fn apply(fs: &mut ExtFs, op: Op) -> Result<OpRecord> {
        match op {
            Op::Create(path, len) => fs.create_file(path, &pattern(len)),
            Op::CreateLong(dir, c) => {
                fs.create_file(&format!("{dir}/{}", c.to_string().repeat(255)), b"")
            }
            Op::Write(path, len) => fs.write_file(path, &pattern(len + 5)[5..]),
            Op::Delete(path) => fs.delete_file(path),
            Op::Mkdir(path) => fs.create_dir(path),
            Op::Rmdir(path) => fs.remove_dir(path),
        }
    }

    /// The slice-2 mutation scenarios in one script: small, three-block,
    /// single-indirect, and 200 KiB files, growth, shrinks out of the
    /// indirect range, a same-length overwrite, deletes, a directory made
    /// and removed, and a directory that grows a second block (the fourth
    /// 264-byte entry does not fit beside `.`, `..`, and three others).
    /// 200 KiB is the largest round size a data-mode transaction takes
    /// (256 tagged blocks at most).
    const SCRIPT: [Op; 17] = [
        Op::Mkdir("/dir"),
        Op::Create("/dir/a.txt", 100),
        Op::Create("/three.bin", 3000),
        Op::Create("/indirect.bin", 20 * 1024),
        Op::Write("/three.bin", 5000),
        Op::Write("/indirect.bin", 100),
        Op::Write("/dir/a.txt", 100),
        Op::Delete("/dir/a.txt"),
        Op::Rmdir("/dir"),
        Op::Delete("/three.bin"),
        Op::Create("/big.bin", 200 * 1024),
        Op::Write("/big.bin", 13 * 1024),
        Op::Mkdir("/wide"),
        Op::CreateLong("/wide", 'a'),
        Op::CreateLong("/wide", 'b'),
        Op::CreateLong("/wide", 'c'),
        Op::CreateLong("/wide", 'd'),
    ];

    /// Ordered mode only: a file grown into the double-indirect range. Its
    /// 300 data blocks are never tagged in ordered mode; data mode would tag
    /// them and pass the 256-block limit.
    const ORDERED_SCRIPT: [Op; 2] = [
        Op::Create("/grow.bin", 1000),
        Op::Write("/grow.bin", 300 * 1024),
    ];

    #[test]
    fn every_mutation_follows_5_2_and_ends_equal_to_ext2_in_both_modes() {
        for mode in BOTH {
            let mut fs = ext3(mode);
            fs.set_now(DateTime::new(2024, 5, 6, 7, 8, 9));
            let mut twin = ext2_twin(&fs);
            let mut script = SCRIPT.to_vec();
            if mode == JournalMode::Ordered {
                script.extend(ORDERED_SCRIPT);
            }
            for (k, &op) in script.iter().enumerate() {
                let before = fs.journal_info().unwrap();
                assert_eq!(before.sequence, 1 + k as u32);
                let rec = apply(&mut fs, op).unwrap();
                let draft = apply(&mut twin, op).unwrap();
                assert_eq!(rec.op, draft.op);
                assert_follows_5_2(&fs, &rec, &draft, &before);
                let mut ours = fs.disk().as_bytes().to_vec();
                let mut theirs = twin.disk().as_bytes().to_vec();
                mask_journal(&mut ours, &fs);
                mask_journal(&mut theirs, &fs);
                assert!(ours == theirs, "{mode:?} {op:?}: differs from ext2");
            }
            assert_eq!(fs.history().len(), script.len());
            assert_eq!(fs.list_dir("/").unwrap(), twin.list_dir("/").unwrap());
            assert_eq!(
                fs.read_file("/big.bin").unwrap(),
                pattern(13 * 1024 + 5)[5..]
            );
            assert_eq!(fs.list_dir("/wide").unwrap().len(), 4);
            assert_eq!(fs.stat("/wide").unwrap().size, 2 * BS as u64);
            if mode == JournalMode::Ordered {
                assert_eq!(
                    fs.read_file("/grow.bin").unwrap(),
                    pattern(300 * 1024 + 5)[5..]
                );
            }
        }
    }

    #[test]
    fn a_clock_at_the_unix_epoch_commits_at_second_1() {
        for mode in BOTH {
            let mut fs = ext3(mode);
            fs.set_now(DateTime::from_unix_seconds(0));
            let rec = fs.create_dir("/d").unwrap();
            // Eight tagged blocks: the descriptor at index 1, the copies at
            // 2..=9, the commit at index 10 (block 92).
            assert!(texts(&rec)
                .iter()
                .any(|t| t == "committed transaction 1 at journal block 10 (block 92)"));
            let commit = fs.disk().read(92 * BS, BS);
            assert_eq!(decode_commit(commit), Some(1));
            assert_eq!(commit[48..56], 1u64.to_be_bytes());
        }
    }

    #[test]
    fn a_sequence_at_u32_max_wraps_to_zero_instead_of_panicking() {
        let mut fs = ext3(JournalMode::Ordered);
        fs.write_raw(JSB_SEQ as u64, &0xFFFF_FFFFu32.to_be_bytes())
            .unwrap();
        assert_eq!(fs.journal_info().unwrap().sequence, 0xFFFF_FFFF);
        let rec = fs.create_file("/w", b"x").unwrap();
        assert!(texts(&rec)
            .iter()
            .any(|t| t == "journal emptied; next transaction 0"));
        assert_eq!(fs.disk().read(JSB_SEQ, 4), 0u32.to_be_bytes());
        assert_eq!(fs.journal_info().unwrap().sequence, 0);
    }

    #[test]
    fn a_three_block_create_in_ordered_mode_writes_the_spec_sequence() {
        let mut fs = ext3(JournalMode::Ordered);
        let rec = fs.create_file("/f", &pattern(3000)).unwrap();
        // Data home first (the body's write and its zero tail), then the
        // journal superblock, descriptor at index 1 (block 83), seven copies,
        // the commit at index 9 (block 91), seven checkpoints.
        let mut want = vec![(FLAG, 4)];
        want.extend([
            (1111 * BS, BS),
            (1112 * BS, BS),
            (1113 * BS, 952),
            (1113 * BS + 952, 72),
        ]);
        want.push((JSB_SEQ, 8));
        want.extend(whole(83..=91));
        want.extend(whole([1, 2, 3, 4, 5, 6, 69]));
        want.push((JSB_SEQ, 8));
        want.push((FLAG, 4));
        assert_eq!(changes_in_order(&rec), want);
        let mut kinds = vec![
            "inode_allocated",
            "bitmap_updated",
            "counters_updated",
            "blocks_allocated",
            "bitmap_updated",
            "counters_updated",
            "data_written",
            "inode_written",
            "dir_entry_written",
            "inode_written",
            "recovery_flag_set",
            "transaction_started",
        ];
        kinds.extend(["journal_block_written"; 9]);
        kinds.extend(["checkpointed"; 7]);
        kinds.extend(["journal_emptied", "recovery_flag_cleared"]);
        assert_eq!(rec.event_kinds(), kinds);
        let info = fs.journal_info().unwrap();
        assert_eq!((info.sequence, info.head, info.start), (2, 10, 0));
        assert_eq!(fs.read_file("/f").unwrap(), pattern(3000));
    }

    #[test]
    fn a_three_block_create_in_data_mode_journals_the_data_too() {
        let mut fs = ext3(JournalMode::Data);
        let rec = fs.create_file("/f", &pattern(3000)).unwrap();
        // No data before the journal superblock; ten copies at indexes
        // 2..=11 (blocks 84..=93), the commit at index 12 (block 94).
        let mut want = vec![(FLAG, 4), (JSB_SEQ, 8)];
        want.extend(whole(83..=94));
        want.extend(whole([1, 2, 3, 4, 5, 6, 69, 1111, 1112, 1113]));
        want.push((JSB_SEQ, 8));
        want.push((FLAG, 4));
        assert_eq!(changes_in_order(&rec), want);
        assert_eq!(count(&rec, "journal_block_written"), 12);
        assert_eq!(count(&rec, "checkpointed"), 10);
        let info = fs.journal_info().unwrap();
        assert_eq!((info.sequence, info.head, info.start), (2, 13, 0));
        assert_eq!(fs.read_file("/f").unwrap(), pattern(3000));
    }

    #[test]
    fn head_and_sequence_advance_across_operations() {
        let mut fs = ext3(JournalMode::Ordered);
        let mut heads = vec![fs.journal_info().unwrap().head];
        for i in 0..4 {
            let rec = fs.create_dir(&format!("/d{i}")).unwrap();
            let info = fs.journal_info().unwrap();
            let length = count(&rec, "journal_block_written") as u32;
            assert_eq!(info.head, heads.last().unwrap() + length);
            assert_eq!(info.sequence, 2 + i);
            assert_eq!(info.start, 0);
            heads.push(info.head);
        }
        // Each directory tags 8 blocks (superblock, descriptors, both bitmaps,
        // two inode table blocks, the root's block, its own): 10 log blocks.
        assert_eq!(heads, vec![1, 11, 21, 31, 41]);
    }

    #[test]
    fn ordered_mode_writes_data_home_before_the_journal_superblock_and_never_tags_it() {
        let mut fs = ext3(JournalMode::Ordered);
        let rec = fs.create_file("/f", &pattern(20 * 1024)).unwrap();
        let data: BTreeSet<u32> = (1111..1131).collect();
        let step3 = rec
            .changes
            .iter()
            .position(|c| c.offset == JSB_SEQ)
            .unwrap();
        assert!(step3 > 1);
        for c in &rec.changes[1..step3] {
            assert!(data.contains(&((c.offset / BS) as u32)), "{c:?}");
        }
        for c in &rec.changes[step3..] {
            assert!(!data.contains(&((c.offset / BS) as u32)), "{c:?}");
        }
        assert!(texts(&rec)
            .iter()
            .all(|t| !(1111..1131).any(|b| t.starts_with(&format!("copied block {b} ")))));
        // The pointer block after the data (1131) is metadata: tagged.
        assert!(texts(&rec)
            .iter()
            .any(|t| t.starts_with("copied block 1131 ")));
    }

    #[test]
    fn data_mode_tags_data_and_writes_it_home_only_at_the_checkpoint() {
        let mut fs = ext3(JournalMode::Data);
        let rec = fs.create_file("/f", &pattern(20 * 1024)).unwrap();
        let commit = rec
            .changes
            .iter()
            .position(|c| decode_commit(&c.after).is_some())
            .unwrap();
        for c in &rec.changes[..=commit] {
            let block = (c.offset / BS) as u32;
            assert!(
                !(1111..1131).contains(&block),
                "data home before the commit: {c:?}"
            );
        }
        for b in 1111..1131 {
            assert!(texts(&rec)
                .iter()
                .any(|t| t.starts_with(&format!("copied block {b} "))));
            assert!(texts(&rec)
                .iter()
                .any(|t| t.starts_with(&format!("checkpointed block {b} "))));
        }
    }

    #[test]
    fn transactions_wrap_around_the_end_of_the_log_block_by_block() {
        let mut fs = ext3(JournalMode::Ordered);
        let mut twin = ext2_twin(&fs);
        let mut wrapped = None;
        for i in 0..300 {
            let before = fs.journal_info().unwrap();
            let path = format!("/r{i:03}");
            let rec = fs.create_file(&path, b"").unwrap();
            let draft = twin.create_file(&path, b"").unwrap();
            assert_follows_5_2(&fs, &rec, &draft, &before);
            if fs.journal_info().unwrap().head < before.head {
                wrapped = Some((before.head, rec));
                break;
            }
        }
        let (start, rec) = wrapped.expect("300 creates pass the end of a 1024-block log");
        // The journal writes run up to index 1023 (block 1105) and continue
        // at index 1 (block 83).
        let blocks: Vec<u32> = rec
            .changes
            .iter()
            .map(|c| (c.offset / BS) as u32)
            .filter(|b| (83..=1105).contains(b))
            .collect();
        let wrap_at = blocks.iter().position(|&b| b == 83).unwrap();
        assert!(wrap_at > 0);
        assert_eq!(blocks[wrap_at - 1], 1105);
        assert_eq!(
            blocks,
            (start..1024)
                .chain(1..)
                .take(blocks.len())
                .map(|i| JOURNAL_BLOCK_0 + i)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_data_mode_create_past_the_limit_fails_and_changes_nothing() {
        let mut fs = ext3(JournalMode::Data);
        fs.create_file("/keep", b"keep").unwrap();
        let image = fs.disk().as_bytes().to_vec();
        let sb = fs.superblock().clone();
        let gds = fs.group_descriptors().to_vec();
        let info = fs.journal_info();
        assert_eq!(
            fs.create_file("/big.bin", &pattern(300 * 1024))
                .unwrap_err(),
            Error::Unsupported(
                "transaction of 310 blocks exceeds the journal's maximum of 256".into()
            )
        );
        assert!(fs.disk().as_bytes() == image.as_slice());
        assert_eq!(fs.history().len(), 1);
        assert_eq!(fs.superblock(), &sb);
        assert_eq!(fs.group_descriptors(), &gds[..]);
        assert_eq!(fs.journal_info(), info);
        assert!(!fs.disk().op_open());
        // Ordered mode tags only the metadata, so the same file fits.
        let mut ordered = ext3(JournalMode::Ordered);
        ordered
            .create_file("/big.bin", &pattern(300 * 1024))
            .unwrap();
    }

    #[test]
    fn a_data_block_that_starts_with_the_magic_is_escaped_in_its_copy() {
        let mut fs = ext3(JournalMode::Data);
        let mut data = pattern(1024);
        data[..4].copy_from_slice(&MAGIC);
        let rec = fs.create_file("/magic", &data).unwrap();
        let text = "copied block 1111 into journal block 9 (block 91) (escaped)";
        assert!(texts(&rec).iter().any(|t| t == text), "{:?}", texts(&rec));
        let descriptor = rec.changes.iter().find(|c| c.offset == 83 * BS).unwrap();
        let tags = decode_descriptor(&descriptor.after).unwrap();
        let tag = tags.iter().find(|t| t.block == 1111).unwrap();
        assert_eq!(tag.flags, TAG_ESCAPE | TAG_SAME_UUID | TAG_LAST);
        let copy = rec.changes.iter().find(|c| c.offset == 91 * BS).unwrap();
        assert_eq!(copy.after[..4], [0, 0, 0, 0]);
        assert_eq!(copy.after[4..], data[4..]);
        let home = rec.changes.iter().rfind(|c| c.offset == 1111 * BS).unwrap();
        assert_eq!(home.after, data);
        assert_eq!(fs.read_file("/magic").unwrap(), data);
        assert_eq!(fs.disk().read(1111 * BS, 4), MAGIC);
    }

    #[test]
    fn ext2_mutations_record_no_journal_events() {
        let mut fs = ExtFs::format(ExtFormatOptions::default()).unwrap();
        let records = vec![
            fs.create_dir("/d").unwrap(),
            fs.create_file("/d/f", &pattern(20 * 1024)).unwrap(),
            fs.write_file("/d/f", &pattern(10)).unwrap(),
            fs.delete_file("/d/f").unwrap(),
            fs.remove_dir("/d").unwrap(),
        ];
        for rec in &records {
            assert!(
                rec.event_kinds().iter().all(|k| !JOURNAL_KINDS.contains(k)),
                "{}",
                rec.op
            );
        }
    }

    const PHASES: [CrashPhase; 3] = [
        CrashPhase::BeforeCommit,
        CrashPhase::AfterCommit,
        CrashPhase::DuringCheckpoint,
    ];

    /// A mutation to crash, with the state it needs first.
    struct Scenario {
        setup: fn(&mut ExtFs),
        op: fn(&mut ExtFs) -> Result<OpRecord>,
    }

    /// `create_file`, `write_file` growing and shrinking, `delete_file`,
    /// `create_dir`, `remove_dir`.
    fn scenarios() -> Vec<Scenario> {
        vec![
            Scenario {
                setup: |_| {},
                op: |fs| fs.create_file("/f", &pattern(3000)),
            },
            Scenario {
                setup: |fs| {
                    fs.create_file("/f", &pattern(1000)).unwrap();
                },
                op: |fs| fs.write_file("/f", &pattern(20 * 1024)),
            },
            Scenario {
                setup: |fs| {
                    fs.create_file("/f", &pattern(20 * 1024)).unwrap();
                },
                op: |fs| fs.write_file("/f", &pattern(100)),
            },
            Scenario {
                setup: |fs| {
                    fs.create_file("/f", &pattern(3000)).unwrap();
                },
                op: |fs| fs.delete_file("/f"),
            },
            Scenario {
                setup: |_| {},
                op: |fs| fs.create_dir("/d"),
            },
            Scenario {
                setup: |fs| {
                    fs.create_dir("/d").unwrap();
                },
                op: |fs| fs.remove_dir("/d"),
            },
        ]
    }

    /// Run `s` twice in `mode`: once whole, once with `phase` armed.
    /// Returns the crashed volume, its record, and the whole record.
    fn crash(mode: JournalMode, s: &Scenario, phase: CrashPhase) -> (ExtFs, OpRecord, OpRecord) {
        let mut whole = ext3(mode);
        (s.setup)(&mut whole);
        let full = (s.op)(&mut whole).unwrap();
        let mut fs = ext3(mode);
        (s.setup)(&mut fs);
        fs.arm_crash(phase).unwrap();
        assert_eq!(fs.crash_phase(), Some(phase));
        let rec = (s.op)(&mut fs).unwrap();
        (fs, rec, full)
    }

    #[test]
    fn each_phase_stops_where_section_6_says_for_every_mutation_in_both_modes() {
        for mode in BOTH {
            for s in scenarios() {
                for phase in PHASES {
                    let mut probe = ext3(mode);
                    (s.setup)(&mut probe);
                    let before = probe.journal_info().unwrap();
                    let history = probe.history().len();
                    let (mut fs, rec, full) = crash(mode, &s, phase);
                    let what = format!("{mode:?} {} {phase:?}", full.op);
                    // The changes: a prefix of the whole record.
                    let n = count(&full, "checkpointed");
                    let after_commit = full.changes.len() - n - 2;
                    let keep = match phase {
                        CrashPhase::BeforeCommit => after_commit - 1,
                        CrashPhase::AfterCommit => after_commit,
                        CrashPhase::DuringCheckpoint => after_commit + 1,
                    };
                    assert_eq!(rec.changes, full.changes[..keep], "{what}");
                    // The events: the same prefix, then `Crashed`.
                    let full_texts = texts(&full);
                    let commit = full_texts
                        .iter()
                        .position(|t| t.starts_with("committed transaction"))
                        .unwrap();
                    let cut = match phase {
                        CrashPhase::BeforeCommit => commit,
                        CrashPhase::AfterCommit => commit + 1,
                        CrashPhase::DuringCheckpoint => commit + 2,
                    };
                    let mut want = full_texts[..cut].to_vec();
                    want.push(format!("crashed {phase}"));
                    assert_eq!(texts(&rec), want, "{what}");
                    let last = rec.changes.last().unwrap();
                    assert_eq!(
                        rec.events.last().unwrap().region(),
                        Some(last.offset..last.offset + last.after.len()),
                        "{what}"
                    );
                    assert_eq!(rec.op, format!("{} (crashed {phase})", full.op));
                    assert_eq!(fs.history().len(), history + 1);
                    assert_eq!(fs.history().last().unwrap().op, rec.op);
                    // The volume needs recovery; the head and sequence wait.
                    assert!(fs.needs_recovery(), "{what}");
                    assert_eq!(fs.crash_phase(), None);
                    let info = fs.journal_info().unwrap();
                    assert_eq!(
                        (info.sequence, info.head, info.start, info.needs_recovery),
                        (before.sequence, before.head, before.head, true),
                        "{what}"
                    );
                    assert_eq!(fs.superblock().feature_incompat, 0x0006, "{what}");
                    assert_eq!(fs.create_dir("/other").unwrap_err(), Error::NeedsRecovery);
                    assert_eq!(fs.history().len(), history + 1);
                    // Reads, inspection, and raw writes see the raw state.
                    assert!(fs.list_dir("/").is_ok());
                    assert!(fs.stat("/").is_ok());
                    assert_eq!(fs.lookup("/lost+found"), Ok(11), "{what}");
                    let root = fs.dir_entries("/").unwrap();
                    assert!(root.iter().any(|(_, _, e)| e.name == b"lost+found"));
                    assert!(!fs.layout().is_empty());
                    assert!(!fs.block_owners().is_empty());
                    assert!(!fs.annotate_sector(1).is_empty());
                    fs.write_raw(16_000 * 1024, &[1]).unwrap();
                    assert_eq!(fs.history().len(), history + 2);
                    assert!(fs.needs_recovery());
                }
            }
        }
    }

    #[test]
    fn a_crashed_create_leaves_the_file_out_of_its_directory_in_every_phase() {
        for mode in BOTH {
            for phase in PHASES {
                let (fs, _, _) = crash(mode, &scenarios()[0], phase);
                let names: Vec<String> = fs
                    .list_dir("/")
                    .unwrap()
                    .into_iter()
                    .map(|e| e.name)
                    .collect();
                assert_eq!(names, vec!["lost+found"], "{mode:?} {phase:?}");
                assert_eq!(fs.read_file("/f").unwrap_err(), Error::NotFound);
                assert_eq!(fs.lookup("/f"), Err(Error::NotFound));
                let root = fs.dir_entries("/").unwrap();
                assert!(root.iter().all(|(_, _, e)| e.name != b"f"), "{root:?}");
            }
        }
    }

    #[test]
    fn ordered_before_commit_leaves_the_data_home_in_blocks_the_bitmap_calls_free() {
        let (fs, _, _) = crash(
            JournalMode::Ordered,
            &scenarios()[0],
            CrashPhase::BeforeCommit,
        );
        let bitmap = fs.disk().sector(3);
        let data = pattern(3000);
        for (k, block) in (1111u32..1114).enumerate() {
            let bit = (block - 1) as usize;
            assert_eq!(
                bitmap[bit / 8] & (1 << (bit % 8)),
                0,
                "block {block} is free"
            );
            let want = &data[k * BS..data.len().min((k + 1) * BS)];
            assert_eq!(&fs.disk().sector(u64::from(block))[..want.len()], want);
        }
        assert_eq!(fs.superblock().free_blocks_count, 15_205);
        // Data mode leaves nothing home.
        let (fs, _, _) = crash(JournalMode::Data, &scenarios()[0], CrashPhase::BeforeCommit);
        assert!(fs.disk().sector(1111).iter().all(|&b| b == 0));
    }

    #[test]
    fn arm_crash_is_unsupported_on_ext2() {
        let mut fs = ExtFs::format(ExtFormatOptions::default()).unwrap();
        assert_eq!(
            fs.arm_crash(CrashPhase::AfterCommit),
            Err(Error::Unsupported("this volume has no journal".into()))
        );
        assert_eq!(fs.crash_phase(), None);
        fs.disarm_crash();
        let rec = fs.create_file("/f", &pattern(3000)).unwrap();
        assert_eq!(rec.op, "create_file /f");
        assert!(!fs.needs_recovery());
    }

    #[test]
    fn disarm_crash_lets_the_next_mutation_complete() {
        let mut fs = ext3(JournalMode::Ordered);
        fs.arm_crash(CrashPhase::BeforeCommit).unwrap();
        fs.arm_crash(CrashPhase::AfterCommit).unwrap();
        assert_eq!(fs.crash_phase(), Some(CrashPhase::AfterCommit));
        fs.disarm_crash();
        assert_eq!(fs.crash_phase(), None);
        let rec = fs.create_file("/f", b"f").unwrap();
        assert_eq!(rec.op, "create_file /f");
        assert!(!fs.needs_recovery());
        assert_eq!(count(&rec, "crashed"), 0);
    }

    #[test]
    fn a_body_that_fails_rolls_back_and_keeps_the_armed_phase() {
        // A 2048-block ext3 volume filled to its last block, whose root
        // block then holds three 255-byte names: a fourth needs a new
        // directory block, and `dir_insert` fails inside the body.
        let mut fs = ExtFs::format(ExtFormatOptions {
            total_blocks: 2048,
            ..ExtFormatOptions::ext3()
        })
        .unwrap();
        let free = fs.superblock().free_blocks_count;
        let data = (1..=free)
            .rev()
            .find(|&d| d + blockmap::indirect_blocks_needed(d) == free)
            .unwrap();
        fs.create_file("/fill", &pattern(data as usize * BS))
            .unwrap();
        assert_eq!(fs.superblock().free_blocks_count, 0);
        for c in ['a', 'b', 'c'] {
            fs.create_file(&format!("/{}", c.to_string().repeat(255)), b"")
                .unwrap();
        }
        let image = fs.disk().as_bytes().to_vec();
        let (sb, info, history) = (
            fs.superblock().clone(),
            fs.journal_info(),
            fs.history().len(),
        );
        fs.arm_crash(CrashPhase::AfterCommit).unwrap();
        assert_eq!(
            fs.create_file(&format!("/{}", "d".repeat(255)), b"")
                .unwrap_err(),
            Error::DiskFull
        );
        assert!(fs.disk().as_bytes() == image.as_slice());
        assert_eq!(fs.superblock(), &sb);
        assert_eq!(fs.journal_info(), info);
        assert_eq!(fs.history().len(), history);
        assert_eq!(fs.crash_phase(), Some(CrashPhase::AfterCommit));
        assert!(!fs.needs_recovery());
    }

    #[test]
    fn a_crashed_volume_refuses_every_mutation_before_checking_its_arguments() {
        let (mut fs, _, _) = crash(
            JournalMode::Ordered,
            &scenarios()[0],
            CrashPhase::AfterCommit,
        );
        let history = fs.history().len();
        assert_eq!(
            fs.delete_file("/missing").unwrap_err(),
            Error::NeedsRecovery
        );
        assert_eq!(
            fs.create_dir("/lost+found").unwrap_err(),
            Error::NeedsRecovery
        );
        assert_eq!(
            fs.create_file("no-slash", b"").unwrap_err(),
            Error::NeedsRecovery
        );
        assert_eq!(
            fs.write_file("/missing", b"x").unwrap_err(),
            Error::NeedsRecovery
        );
        assert_eq!(fs.remove_dir("/").unwrap_err(), Error::NeedsRecovery);
        assert_eq!(fs.history().len(), history);
        fs.write_raw(16_000 * 1024, &[1]).unwrap();
        assert_eq!(fs.history().len(), history + 1);
        assert!(fs.needs_recovery());
    }

    #[test]
    fn arming_while_recovery_is_needed_is_allowed_and_waits() {
        let (mut fs, _, _) = crash(
            JournalMode::Ordered,
            &scenarios()[4],
            CrashPhase::AfterCommit,
        );
        fs.arm_crash(CrashPhase::DuringCheckpoint).unwrap();
        assert_eq!(fs.crash_phase(), Some(CrashPhase::DuringCheckpoint));
        assert_eq!(fs.create_dir("/e").unwrap_err(), Error::NeedsRecovery);
        assert_eq!(fs.crash_phase(), Some(CrashPhase::DuringCheckpoint));
    }
}
