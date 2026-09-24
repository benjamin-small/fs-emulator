use ext::{BlockOwner, BlockRole, DirEntry, ExtFormatOptions, ExtFs, Inode, Superblock};
use fs_core::{DateTime, Disk, EntryInfo, Error, FileSystem, RegionKind};

/// 1980-01-01 00:00:00 UTC, the default `now`, as ext stores it.
const EPOCH_1980: u32 = 315_532_800;

fn default_fs() -> ExtFs {
    ExtFs::format(ExtFormatOptions::default()).unwrap()
}

fn bit(bytes: &[u8], i: usize) -> bool {
    bytes[i / 8] & (1 << (i % 8)) != 0
}

fn put_u16(image: &mut [u8], offset: usize, value: u16) {
    image[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(image: &mut [u8], offset: usize, value: u32) {
    image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn format_writes_the_counters_of_the_spec() {
    let fs = default_fs();
    let sb = fs.superblock();
    assert_eq!(sb.blocks_count, 16_384);
    assert_eq!(sb.inodes_count, 1_024);
    assert_eq!(sb.free_blocks_count, 16_234);
    assert_eq!(sb.free_inodes_count, 1_013);
    assert_eq!(sb.block_group_nr, 0);
    assert_eq!(sb.wtime, EPOCH_1980);
    assert_eq!(sb.lastcheck, EPOCH_1980);
    assert_eq!(sb.mtime, 0);
    assert_eq!(sb.label(), "");
    let gds = fs.group_descriptors();
    assert_eq!(gds.len(), 2);
    assert_eq!(
        (
            gds[0].free_blocks_count,
            gds[0].free_inodes_count,
            gds[0].used_dirs_count
        ),
        (8_111, 501, 2)
    );
    assert_eq!(
        (
            gds[1].free_blocks_count,
            gds[1].free_inodes_count,
            gds[1].used_dirs_count
        ),
        (8_123, 512, 0)
    );
    assert_eq!(fs.disk().len(), 16 * 1024 * 1024);
    assert!(fs.history().is_empty());
    assert!(
        fs.disk().sector(0).iter().all(|&b| b == 0),
        "boot block untouched"
    );
}

#[test]
fn format_writes_every_superblock_and_descriptor_copy() {
    let fs = default_fs();
    let primary = Superblock::decode(fs.disk().sector(1));
    assert_eq!(&primary, fs.superblock());
    let backup = Superblock::decode(fs.disk().sector(8_193));
    assert_eq!(backup.block_group_nr, 1);
    assert_eq!(
        Superblock {
            block_group_nr: 0,
            ..backup
        },
        primary
    );
    assert_eq!(fs.disk().sector(2), fs.disk().sector(8_194));
}

#[test]
fn format_sets_metadata_bits_and_padding_in_every_bitmap() {
    let fs = default_fs();
    let disk = fs.disk();
    // Group 0 block bitmap: bit i is block 1 + i; blocks 1..=81 are the
    // metadata, the root's block 69, and lost+found's blocks 70..=81.
    let blocks0 = disk.sector(3);
    assert!((0..81).all(|i| bit(blocks0, i)));
    assert!((81..8_192).all(|i| !bit(blocks0, i)));
    // Group 1 holds blocks 8193..=16383: 68 metadata blocks, then free
    // blocks, then one padding bit.
    let blocks1 = disk.sector(8_195);
    assert!((0..68).all(|i| bit(blocks1, i)));
    assert!((68..8_191).all(|i| !bit(blocks1, i)));
    assert!(bit(blocks1, 8_191));
    // Inode bitmaps: inodes 1..=11 used in group 0; bits 512.. are padding.
    let inodes0 = disk.sector(4);
    assert!((0..11).all(|i| bit(inodes0, i)));
    assert!((11..512).all(|i| !bit(inodes0, i)));
    assert!((512..8_192).all(|i| bit(inodes0, i)));
    let inodes1 = disk.sector(8_196);
    assert!((0..512).all(|i| !bit(inodes1, i)));
    assert!((512..8_192).all(|i| bit(inodes1, i)));
    // Every inode table block apart from the one holding inodes 1..=8 and
    // the one holding inodes 9..=16 is zero.
    assert!((7..69).all(|b| disk.sector(b).iter().all(|&x| x == 0)));
    assert!((8_197..8_261).all(|b| disk.sector(b).iter().all(|&x| x == 0)));
}

#[test]
fn format_writes_root_and_lost_found() {
    let fs = default_fs();
    let root = fs.inode(2).unwrap();
    assert_eq!(root.mode, 0x41ED);
    assert_eq!((root.links_count, root.size, root.blocks), (3, 1_024, 2));
    assert_eq!(root.block[0], 69);
    assert_eq!(
        (root.atime, root.ctime, root.mtime),
        (EPOCH_1980, EPOCH_1980, EPOCH_1980)
    );
    let lost_found = fs.inode(11).unwrap();
    assert_eq!(lost_found.mode, 0x41C0);
    assert_eq!(
        (lost_found.links_count, lost_found.size, lost_found.blocks),
        (2, 12 * 1_024, 24)
    );
    assert_eq!(lost_found.block[..12], (70..82).collect::<Vec<u32>>()[..]);
    assert_eq!(lost_found.block[12..], [0, 0, 0]);
    for ino in [1, 3, 7, 8, 10, 12, 1_024] {
        assert_eq!(fs.inode(ino).unwrap(), Inode::default(), "inode {ino}");
    }
    assert_eq!(fs.inode(0), Err(Error::NotFound));
    assert_eq!(fs.inode(1_025), Err(Error::NotFound));
}

#[test]
fn layout_is_the_region_table_of_the_spec() {
    let fs = default_fs();
    let got: Vec<(String, u64, u64, RegionKind)> = fs
        .layout()
        .into_iter()
        .map(|r| (r.name, r.sectors.start, r.sectors.end, r.kind))
        .collect();
    let want = [
        ("boot block", 0, 1, RegionKind::Boot),
        ("superblock", 1, 2, RegionKind::Boot),
        ("group descriptors (group 0)", 2, 3, RegionKind::Metadata),
        ("block bitmap (group 0)", 3, 4, RegionKind::AllocationTable),
        ("inode bitmap (group 0)", 4, 5, RegionKind::AllocationTable),
        ("inode table (group 0)", 5, 69, RegionKind::Metadata),
        ("data (group 0)", 69, 8_193, RegionKind::Data),
        (
            "backup superblock (group 1)",
            8_193,
            8_194,
            RegionKind::Boot,
        ),
        (
            "group descriptors (group 1)",
            8_194,
            8_195,
            RegionKind::Metadata,
        ),
        (
            "block bitmap (group 1)",
            8_195,
            8_196,
            RegionKind::AllocationTable,
        ),
        (
            "inode bitmap (group 1)",
            8_196,
            8_197,
            RegionKind::AllocationTable,
        ),
        ("inode table (group 1)", 8_197, 8_261, RegionKind::Metadata),
        ("data (group 1)", 8_261, 16_384, RegionKind::Data),
    ];
    let want: Vec<(String, u64, u64, RegionKind)> = want
        .into_iter()
        .map(|(n, s, e, k)| (n.to_string(), s, e, k))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn a_small_volume_is_one_group_with_the_same_shape() {
    let mut label = [0u8; 16];
    label[..4].copy_from_slice(b"demo");
    let fs = ExtFs::format(ExtFormatOptions {
        total_blocks: 64,
        label,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(fs.geometry().groups, 1);
    assert_eq!(fs.superblock().inodes_count, 16);
    // 63 blocks, minus superblock, descriptors, two bitmaps, a 2-block
    // inode table, the root's block, and lost+found's 12.
    assert_eq!(fs.superblock().free_blocks_count, 44);
    assert_eq!(fs.superblock().free_inodes_count, 5);
    assert_eq!(fs.superblock().label(), "demo");
    assert_eq!(fs.layout().last().unwrap().sectors, 7..64);
    assert_eq!(fs.inode(11).unwrap().block[0], 8);
}

#[test]
fn format_rejects_geometry_that_cannot_hold_the_metadata() {
    for options in [
        ExtFormatOptions {
            total_blocks: 63,
            ..Default::default()
        },
        ExtFormatOptions {
            total_blocks: 64,
            inodes_per_group: Some(8_192),
            ..Default::default()
        },
        ExtFormatOptions {
            total_blocks: 8_200,
            ..Default::default()
        },
    ] {
        assert!(
            matches!(
                ExtFs::format(options.clone()),
                Err(Error::InvalidGeometry(_))
            ),
            "{options:?}"
        );
    }
}

#[test]
fn from_image_round_trips_and_drops_trailing_bytes() {
    let mut label = [0u8; 16];
    label[..8].copy_from_slice(b"emulator");
    let fs = ExtFs::format(ExtFormatOptions {
        label,
        ..Default::default()
    })
    .unwrap();
    let mut image = fs.disk().as_bytes().to_vec();
    image.extend_from_slice(&[0xAB; 4_096]);
    let again = ExtFs::from_image(image).unwrap();
    assert_eq!(again.disk().as_bytes(), fs.disk().as_bytes());
    assert_eq!(again.superblock(), fs.superblock());
    assert_eq!(again.superblock().label(), "emulator");
    assert_eq!(again.group_descriptors(), fs.group_descriptors());
    assert_eq!(again.geometry(), fs.geometry());
    assert_eq!(again.inode(11).unwrap(), fs.inode(11).unwrap());
    assert!(again.history().is_empty());
    assert!(again.corruption().is_none());
}

#[test]
fn from_image_rejects_short_unsigned_and_truncated_images() {
    let image = default_fs().disk().as_bytes().to_vec();
    assert!(matches!(
        ExtFs::from_image(vec![0; 2_047]),
        Err(Error::CorruptImage(_))
    ));
    let mut unsigned = image.clone();
    put_u16(&mut unsigned, 1_024 + 56, 0);
    assert!(matches!(
        ExtFs::from_image(unsigned),
        Err(Error::CorruptImage(_))
    ));
    let truncated = image[..8 * 1024 * 1024].to_vec();
    assert!(matches!(
        ExtFs::from_image(truncated),
        Err(Error::CorruptImage(_))
    ));
    let mut moved = image.clone();
    put_u32(&mut moved, 2 * 1_024, 4_000);
    assert!(matches!(
        ExtFs::from_image(moved),
        Err(Error::CorruptImage(_))
    ));
}

#[test]
fn from_image_names_each_unsupported_reason() {
    let image = default_fs().disk().as_bytes().to_vec();
    let cases: [(&str, usize, u32, usize); 5] = [
        ("revision 0", 76, 0, 4),
        ("2 KiB blocks", 24, 1, 4),
        ("256-byte inodes", 88, 256, 2),
        ("extents (incompat 0x40)", 96, 0x0002 | 0x0040, 4),
        ("large_file (ro_compat 0x2)", 100, 0x0001 | 0x0002, 4),
    ];
    for (what, offset, value, len) in cases {
        let mut bad = image.clone();
        let at = 1_024 + offset;
        if len == 2 {
            put_u16(&mut bad, at, value as u16);
        } else {
            put_u32(&mut bad, at, value);
        }
        match ExtFs::from_image(bad) {
            Err(Error::Unsupported(msg)) => assert!(!msg.is_empty(), "{what}"),
            other => panic!("{what}: expected Unsupported, got {:?}", other.err()),
        }
    }
}

/// The bytes of the planted file: 12 KiB plus 300, so its last block is
/// reached through the single-indirect block.
fn planted_data() -> Vec<u8> {
    (0..12 * 1024 + 300).map(|i| (i % 251) as u8).collect()
}

/// A default volume with `/hello.txt` written by hand through a `Disk`:
/// inode 12, 13 data blocks placed in descending order (logical block `k`
/// at physical `212 - k`) so only the mapping can read them back, and the
/// single-indirect block 213 holding the pointer to logical block 12. The
/// bitmaps and counters are left alone: reading needs only the directory
/// entry, the inode, and the blocks. `size` is what the inode claims.
fn planted_fs(size: u32) -> ExtFs {
    let fs = default_fs();
    let root_block = fs.inode(2).unwrap().block[0] as usize;
    let (inode_block, inode_offset) = fs.geometry().inode_location(12);
    let mut disk = Disk::from_bytes(1024, fs.disk().as_bytes().to_vec()).unwrap();

    // Shrink lost+found's entry (at byte 24) to its 20 bytes and put
    // hello.txt after it, running to the end of the block.
    disk.write(root_block * 1024 + 24 + 4, &20u16.to_le_bytes());
    let mut entry = [0u8; 980];
    DirEntry {
        inode: 12,
        rec_len: 980,
        name_len: 9,
        file_type: 1,
        name: b"hello.txt".to_vec(),
    }
    .encode(&mut entry);
    disk.write(root_block * 1024 + 44, &entry);

    let data = planted_data();
    let mut inode = Inode {
        mode: 0x81A4,
        size,
        links_count: 1,
        blocks: 28,
        atime: EPOCH_1980,
        ctime: EPOCH_1980,
        mtime: EPOCH_1980,
        ..Default::default()
    };
    for (k, chunk) in data.chunks(1024).enumerate() {
        let physical = 212 - k as u32;
        disk.write(physical as usize * 1024, chunk);
        if k < 12 {
            inode.block[k] = physical;
        }
    }
    inode.block[12] = 213;
    disk.write(213 * 1024, &200u32.to_le_bytes());
    let mut raw = [0u8; 128];
    inode.encode(&mut raw);
    disk.write(inode_block as usize * 1024 + inode_offset, &raw);
    ExtFs::from_image(disk.as_bytes().to_vec()).unwrap()
}

#[test]
fn root_lists_only_lost_found_and_stat_reports_both_directories() {
    let fs = default_fs();
    let at = Some(DateTime::default());
    assert_eq!(
        fs.list_dir("/").unwrap(),
        vec![EntryInfo {
            name: "lost+found".into(),
            is_dir: true,
            size: 12 * 1_024,
            created: at,
            modified: at,
            accessed: at,
        }]
    );
    assert_eq!(fs.list_dir("/lost+found").unwrap(), Vec::new());
    assert_eq!(
        fs.stat("/").unwrap(),
        EntryInfo {
            name: "/".into(),
            is_dir: true,
            size: 1_024,
            created: at,
            modified: at,
            accessed: at,
        }
    );
    let lost_found = fs.stat("/lost+found").unwrap();
    assert_eq!(lost_found.name, "lost+found");
    assert!(lost_found.is_dir);
    assert_eq!(
        fs.stat("/Lost+Found"),
        Err(Error::NotFound),
        "names are case-sensitive"
    );
    assert_eq!(fs.stat("/missing"), Err(Error::NotFound));
    assert_eq!(fs.list_dir("/lost+found/x"), Err(Error::NotFound));
    assert_eq!(fs.read_file("/lost+found"), Err(Error::IsADirectory));
    assert_eq!(fs.read_file("/"), Err(Error::IsADirectory));
    assert_eq!(fs.stat("relative"), Err(Error::InvalidPath));
}

#[test]
fn read_file_follows_direct_and_indirect_pointers() {
    let data = planted_data();
    let fs = planted_fs(data.len() as u32);
    assert_eq!(fs.read_file("/hello.txt").unwrap(), data);
    assert_eq!(fs.read_file("\\hello.txt").unwrap(), data);
    let info = fs.stat("/hello.txt").unwrap();
    assert!(!info.is_dir);
    assert_eq!(info.size, data.len() as u64);
    assert_eq!(info.modified, Some(DateTime::default()));
    let names: Vec<String> = fs
        .list_dir("/")
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, ["lost+found", "hello.txt"]);
    assert_eq!(fs.list_dir("/hello.txt"), Err(Error::NotADirectory));
    assert_eq!(fs.read_file("/hello.txt/x"), Err(Error::NotADirectory));
    assert_eq!(fs.read_file("/HELLO.TXT"), Err(Error::NotFound));
}

#[test]
fn read_file_reports_an_inode_that_maps_too_few_blocks() {
    let fs = planted_fs(20_000);
    assert!(matches!(
        fs.read_file("/hello.txt"),
        Err(Error::CorruptImage(_))
    ));
}

#[test]
fn block_owners_names_directory_data_and_indirect_blocks() {
    let owners = default_fs().block_owners();
    assert_eq!(owners.len(), 13);
    assert_eq!(
        owners[&69],
        BlockOwner {
            inode: 2,
            path: "/".into(),
            role: BlockRole::Directory
        }
    );
    assert!((70..82)
        .all(|b| owners[&b].path == "/lost+found" && owners[&b].role == BlockRole::Directory));

    let owners = planted_fs(planted_data().len() as u32).block_owners();
    assert_eq!(owners.len(), 13 + 13 + 1);
    assert_eq!(
        owners[&213],
        BlockOwner {
            inode: 12,
            path: "/hello.txt".into(),
            role: BlockRole::Indirect
        }
    );
    assert!((200..=212).all(|b| owners[&b].inode == 12 && owners[&b].role == BlockRole::Data));
}

#[test]
fn set_now_sets_the_clock() {
    let mut fs = default_fs();
    assert_eq!(fs.now(), DateTime::default());
    fs.set_now(DateTime::new(2026, 9, 23, 12, 0, 0));
    assert_eq!(fs.now(), DateTime::new(2026, 9, 23, 12, 0, 0));
}

#[test]
fn a_raw_write_that_breaks_the_superblock_gates_path_operations() {
    let mut fs = default_fs();
    let layout = fs.layout();
    let good = fs.disk().sector(1).to_vec();
    fs.write_raw(1_024, &[0; 1_024]).unwrap();
    let Some(Error::CorruptImage(msg)) = fs.corruption().cloned() else {
        panic!("expected a CorruptImage gate, got {:?}", fs.corruption());
    };
    assert!(
        msg.starts_with("superblock or group descriptors no longer parse after a raw write: "),
        "{msg}"
    );
    let gate = Err(Error::CorruptImage(msg));
    assert_eq!(fs.list_dir("/"), gate.clone().map(|_: ()| Vec::new()));
    assert_eq!(fs.stat("/").map(|_| ()), gate.clone());
    assert_eq!(fs.read_file("/x").map(|_| ()), gate.clone());
    assert_eq!(fs.create_file("/x", b"").map(|_| ()), gate.clone());
    assert_eq!(fs.layout(), layout, "layout keeps the last good geometry");
    assert_eq!(fs.superblock().free_blocks_count, 16_234);

    fs.write_raw(1_024, &good).unwrap();
    assert!(fs.corruption().is_none());
    assert_eq!(fs.list_dir("/").unwrap().len(), 1);
    assert_eq!(fs.history().len(), 2);
    assert_eq!(fs.history()[0].op, "write_raw 0x400 +1024");
}

#[test]
fn a_raw_write_that_breaks_the_descriptors_gates_path_operations() {
    let mut fs = default_fs();
    fs.write_raw(2 * 1_024, &[0; 32]).unwrap();
    assert!(matches!(fs.corruption(), Some(Error::CorruptImage(_))));
    assert!(fs.list_dir("/").is_err());
    let restore = fs.history()[0].changes[0].before.clone();
    fs.write_raw(2 * 1_024, &restore).unwrap();
    assert!(fs.corruption().is_none());
}

#[test]
fn a_valid_raw_edit_of_the_superblock_is_adopted() {
    let mut fs = default_fs();
    fs.write_raw(1_024 + 120, b"renamed\0").unwrap();
    assert!(fs.corruption().is_none());
    assert_eq!(fs.superblock().label(), "renamed");
    fs.write_raw(2 * 1_024 + 12, &100u16.to_le_bytes()).unwrap();
    assert_eq!(fs.group_descriptors()[0].free_blocks_count, 100);
}

#[test]
fn raw_writes_elsewhere_leave_the_cached_metadata_alone() {
    let mut fs = default_fs();
    fs.write_raw(3 * 1_024, &[0xFF; 16]).unwrap();
    fs.write_raw(1_500, &[]).unwrap();
    assert!(fs.corruption().is_none());
    assert_eq!(fs.history().len(), 2);
    assert_eq!(
        fs.write_raw(16 * 1024 * 1024, &[1]).unwrap_err(),
        Error::OutOfBounds {
            offset: 16 * 1024 * 1024,
            len: 1,
            disk_len: 16 * 1024 * 1024
        }
    );
    assert_eq!(fs.history().len(), 2);
}

#[test]
fn raw_write_records_rewind_byte_for_byte() {
    let mut fs = default_fs();
    let before = fs.disk().as_bytes().to_vec();
    let record = fs.write_raw(1_024 + 120, b"label").unwrap();
    let mut rewound = fs.disk().as_bytes().to_vec();
    for change in record.changes.iter().rev() {
        rewound[change.offset..change.offset + change.before.len()].copy_from_slice(&change.before);
    }
    assert_eq!(rewound, before);
    assert_eq!(record.event_kinds(), vec!["raw_write"]);
}

#[test]
fn annotations_describe_superblocks_descriptors_bitmaps_and_inodes() {
    let fs = default_fs();
    let has = |block: u64, label: &str, value: &str| {
        let notes = fs.annotate_sector(block);
        assert!(
            notes.iter().any(|a| a.label == label && a.value == value),
            "block {block}: no {label:?} = {value:?} in {notes:#?}"
        );
    };
    has(0, "boot block", "not used by ext2");
    has(1, "magic", "0xEF53");
    has(1, "free blocks count", "16234");
    has(1, "free inodes count", "1013");
    has(1, "max mount count", "-1");
    has(1, "incompatible features", "0x00000002");
    has(1, "read-only compatible features", "0x00000001");
    has(1, "UUID", "e2f5ee00-2026-4923-8000-000000000001");
    has(1, "last write time", "315532800 (1980-01-01 00:00:00)");
    has(1, "block group number", "0");
    has(8_193, "block group number", "1");
    let magic = fs
        .annotate_sector(1)
        .into_iter()
        .find(|a| a.label == "magic")
        .unwrap();
    assert_eq!(magic.range, 56..58);
    has(2, "group 0 free blocks", "8111");
    has(2, "group 1 free inodes", "512");
    has(2, "group 0 directories", "2");
    has(2, "unused", "past the last group");
    has(8_194, "group 1 inode table", "8197");
    has(3, "blocks 1..81", "used");
    has(3, "blocks 82..8192", "free");
    has(8_195, "blocks 8193..8260", "used");
    has(8_195, "blocks 8261..16383", "free");
    has(8_195, "padding", "set: bits past the end of the group");
    has(4, "inodes 1..11", "used");
    has(4, "inodes 12..512", "free");
    has(4, "padding", "set: bits past the end of the group");
    has(8_196, "inodes 513..1024", "free");
    has(5, "inode 1", "reserved");
    has(
        5,
        "inode 2",
        "mode 040755 (directory), size 1024, links 3, blocks 2, direct 69, \
         accessed 1980-01-01 00:00:00, changed 1980-01-01 00:00:00, modified 1980-01-01 00:00:00",
    );
    let inode_11 = fs
        .annotate_sector(6)
        .into_iter()
        .find(|a| a.label == "inode 11")
        .unwrap();
    assert_eq!(inode_11.range, 256..384);
    assert!(inode_11.value.starts_with(
        "mode 040700 (directory), size 12288, links 2, blocks 24, direct 70, 71, 72, 73, …"
    ));
    has(6, "inode 12", "free");
    has(8_260, "inode 1024", "free");
    assert!(fs.annotate_sector(16_384).is_empty());
}

#[test]
fn usable_as_a_trait_object() {
    let mut fs: Box<dyn FileSystem> = Box::new(default_fs());
    assert_eq!(fs.fs_type(), "ext2");
    assert_eq!(fs.list_dir("/").unwrap()[0].name, "lost+found");
    assert!(fs.stat("/lost+found").unwrap().is_dir);
    assert_eq!(fs.layout().len(), 13);
    assert!(!fs.annotate_sector(1).is_empty());
    fs.set_now(DateTime::new(2026, 1, 1, 0, 0, 0));
    assert!(fs.corruption().is_none());
    fs.write_raw(1_024 + 56, &[0, 0]).unwrap();
    assert!(matches!(fs.corruption(), Some(Error::CorruptImage(_))));
    fs.write_raw(1_024 + 56, &[0x53, 0xEF]).unwrap();
    assert!(fs.corruption().is_none());
    assert_eq!(fs.history().len(), 2);
}

#[test]
fn annotations_show_the_live_bytes_while_the_volume_is_corrupt() {
    let mut fs = default_fs();
    fs.write_raw(1_024, &[0; 1_024]).unwrap();
    assert!(fs.corruption().is_some());
    let magic = fs
        .annotate_sector(1)
        .into_iter()
        .find(|a| a.label == "magic")
        .unwrap();
    assert_eq!(magic.value, "0x0000");
    assert!(fs
        .annotate_sector(5)
        .iter()
        .any(|a| a.label == "inode 2" && a.value.starts_with("mode 040755")));
}
