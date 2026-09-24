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
