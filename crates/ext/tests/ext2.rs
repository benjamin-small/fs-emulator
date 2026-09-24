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
    let cases: [(&str, usize, u32, usize, &str); 8] = [
        ("revision 0", 76, 0, 4, "revision 0"),
        ("2 KiB blocks", 24, 1, 4, "block size"),
        ("256-byte inodes", 88, 256, 2, "inode size 256"),
        ("extents (incompat 0x40)", 96, 0x0002 | 0x0040, 4, "extent"),
        (
            "large_file (ro_compat 0x2)",
            100,
            0x0001 | 0x0002,
            4,
            "large_file",
        ),
        ("resize_inode (compat 0x10)", 92, 0x0010, 4, "resize_inode"),
        (
            "missing filetype (incompat 0)",
            96,
            0,
            4,
            "filetype feature is required",
        ),
        (
            "missing sparse_super (ro_compat 0)",
            100,
            0,
            4,
            "sparse_super feature is required",
        ),
    ];
    for (what, offset, value, len, want) in cases {
        let mut bad = image.clone();
        let at = 1_024 + offset;
        if len == 2 {
            put_u16(&mut bad, at, value as u16);
        } else {
            put_u32(&mut bad, at, value);
        }
        match ExtFs::from_image(bad) {
            Err(Error::Unsupported(msg)) => {
                assert!(
                    msg.contains(want),
                    "{what}: {msg:?} does not contain {want:?}"
                );
            }
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
            created: None,
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
            created: None,
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
fn lookup_returns_inode_numbers_through_the_gate() {
    let mut fs = default_fs();
    assert_eq!(fs.lookup("/"), Ok(2));
    assert_eq!(fs.lookup("/lost+found"), Ok(11));
    fs.create_file("/made.txt", b"made").unwrap();
    let ino = fs.lookup("/made.txt").unwrap();
    assert_eq!(ino, 12);
    assert_eq!(fs.inode(ino).unwrap().size, 4);
    assert_eq!(fs.lookup("/missing"), Err(Error::NotFound));
    assert_eq!(fs.lookup("/made.txt/x"), Err(Error::NotADirectory));
    assert_eq!(fs.lookup("relative"), Err(Error::InvalidPath));
    fs.write_raw(1024 + 56, &[0, 0]).unwrap();
    assert!(matches!(fs.lookup("/"), Err(Error::CorruptImage(_))));
}

#[test]
fn dir_entries_lists_every_entry_with_its_block_and_offset() {
    let mut fs = default_fs();
    let root = fs.dir_entries("/").unwrap();
    let got: Vec<(u32, usize, u32, Vec<u8>)> = root
        .iter()
        .map(|(block, offset, e)| (*block, *offset, e.inode, e.name.clone()))
        .collect();
    assert_eq!(
        got,
        vec![
            (69, 0, 2, b".".to_vec()),
            (69, 12, 2, b"..".to_vec()),
            (69, 24, 11, b"lost+found".to_vec()),
        ]
    );
    assert_eq!(root[2].2.rec_len, 1000);
    // lost+found: `.` and `..`, then 11 unused block-long entries.
    let lost = fs.dir_entries("/lost+found").unwrap();
    assert_eq!(lost.len(), 13);
    assert!(lost[2..]
        .iter()
        .all(|(_, offset, e)| *offset == 0 && e.inode == 0));
    fs.create_file("/f.txt", b"").unwrap();
    assert_eq!(fs.dir_entries("/f.txt"), Err(Error::NotADirectory));
    assert_eq!(fs.dir_entries("/missing"), Err(Error::NotFound));
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

/// Task 4: creating files and directories (spec sections 4 and 5).
mod create_ops {
    use super::default_fs;
    use ext::{
        bitmap, DirEntry, ExtFormatOptions, ExtFs, GroupDescriptor, Superblock, LOST_FOUND_INO,
        ROOT_INO,
    };
    use fs_core::{DateTime, Error, FileSystem, OpRecord};

    const MAX_FILE_BYTES: usize = (12 + 256 + 65_536) * 1024;

    /// Superblock free blocks and inodes, then per group (free blocks, free
    /// inodes, used directories).
    type Counters = (u32, u32, Vec<(u16, u16, u16)>);

    /// 64 blocks: one group, 16 inodes, 44 free blocks and 5 free inodes after format.
    fn tiny_disk() -> ExtFs {
        ExtFs::format(ExtFormatOptions {
            total_blocks: 64,
            ..Default::default()
        })
        .unwrap()
    }

    /// The default 16 MiB, two groups, but 16 inodes per group, so group 0
    /// runs out of inodes after five creates.
    fn small_groups_disk() -> ExtFs {
        ExtFs::format(ExtFormatOptions {
            inodes_per_group: Some(16),
            ..Default::default()
        })
        .unwrap()
    }

    /// Bytes whose period (251) does not divide the block size, so a block
    /// read from the wrong place cannot pass.
    fn pattern(len: usize, seed: usize) -> Vec<u8> {
        (0..len)
            .map(|i| ((i * 7 + i / 1024 + seed) % 251) as u8)
            .collect()
    }

    fn count(rec: &OpRecord, kind: &str) -> usize {
        rec.event_kinds().iter().filter(|k| **k == kind).count()
    }

    /// The counters as the primary superblock and descriptor table hold
    /// them on disk, after asserting the cached copies match those bytes.
    fn counters(fs: &ExtFs) -> Counters {
        let bytes = fs.disk().as_bytes();
        let sb = Superblock::decode(&bytes[1024..2048]);
        assert_eq!(
            &sb,
            fs.superblock(),
            "cached superblock differs from block 1"
        );
        let table = fs.geometry().groups_layout[0].descriptors_block.unwrap() as usize * 1024;
        let gds: Vec<GroupDescriptor> = (0..fs.geometry().groups as usize)
            .map(|i| GroupDescriptor::decode(&bytes[table + i * 32..table + i * 32 + 32]))
            .collect();
        assert_eq!(
            gds.as_slice(),
            fs.group_descriptors(),
            "cached descriptors differ from the table"
        );
        // The bitmaps themselves must agree with the counters they back:
        // recount each group's clear bits (respecting the padding bits past
        // its last block or inode) and check the sums against the superblock.
        let geo = fs.geometry();
        let (mut free_blocks, mut free_inodes) = (0u32, 0u32);
        for (i, gd) in gds.iter().enumerate() {
            let gl = geo.group(i as u32);
            let start = gl.block_bitmap as usize * 1024;
            let clear = (0..gl.block_count as usize)
                .filter(|&b| !bitmap::is_set(&bytes[start..start + 1024], b))
                .count();
            assert_eq!(
                clear, gd.free_blocks_count as usize,
                "group {i} block bitmap"
            );
            let start = gl.inode_bitmap as usize * 1024;
            let clear = (0..geo.inodes_per_group as usize)
                .filter(|&b| !bitmap::is_set(&bytes[start..start + 1024], b))
                .count();
            assert_eq!(
                clear, gd.free_inodes_count as usize,
                "group {i} inode bitmap"
            );
            free_blocks += u32::from(gd.free_blocks_count);
            free_inodes += u32::from(gd.free_inodes_count);
        }
        assert_eq!(
            free_blocks, sb.free_blocks_count,
            "sum of group free blocks"
        );
        assert_eq!(
            free_inodes, sb.free_inodes_count,
            "sum of group free inodes"
        );
        (
            sb.free_blocks_count,
            sb.free_inodes_count,
            gds.iter()
                .map(|g| (g.free_blocks_count, g.free_inodes_count, g.used_dirs_count))
                .collect(),
        )
    }

    /// What the explorer's timeline does: put every `before` back, last first.
    fn rewind(image: &[u8], rec: &OpRecord) -> Vec<u8> {
        let mut out = image.to_vec();
        for c in rec.changes.iter().rev() {
            out[c.offset..c.offset + c.before.len()].copy_from_slice(&c.before);
        }
        out
    }

    /// Everything a failed operation must leave alone.
    fn snapshot(fs: &ExtFs) -> (Vec<u8>, usize, Counters) {
        (
            fs.disk().as_bytes().to_vec(),
            fs.history().len(),
            counters(fs),
        )
    }

    /// Run one operation; check it joined history, rewinds byte for byte,
    /// reports events with regions, and leaves `expected` counters.
    fn step(
        fs: &mut ExtFs,
        op: impl FnOnce(&mut ExtFs) -> fs_core::Result<OpRecord>,
        expected: Counters,
    ) -> OpRecord {
        let before = fs.disk().as_bytes().to_vec();
        let history = fs.history().len();
        let rec = op(fs).unwrap();
        assert_eq!(fs.history().len(), history + 1);
        assert!(!rec.events.is_empty(), "{} reported no events", rec.op);
        for e in &rec.events {
            assert!(
                e.region().is_some(),
                "{} has an event without a region",
                rec.op
            );
            assert!(!e.to_string().is_empty());
        }
        assert!(
            rewind(fs.disk().as_bytes(), &rec) == before,
            "{} does not rewind",
            rec.op
        );
        assert_eq!(counters(fs), expected, "counters after {}", rec.op);
        rec
    }

    /// (name, inode, file type, rec_len) of every entry in a directory block.
    fn dir_block(fs: &ExtFs, block: u32) -> Vec<(String, u32, u8, u16)> {
        let start = block as usize * 1024;
        let bytes = &fs.disk().as_bytes()[start..start + 1024];
        let mut out = Vec::new();
        let mut off = 0;
        while off < 1024 {
            let e = DirEntry::decode(&bytes[off..]).unwrap();
            out.push((
                String::from_utf8(e.name.clone()).unwrap(),
                e.inode,
                e.file_type,
                e.rec_len,
            ));
            off += e.rec_len as usize;
        }
        assert_eq!(off, 1024, "the last entry must run to the end of the block");
        out
    }

    fn u32s(fs: &ExtFs, block: u32) -> Vec<u32> {
        let start = block as usize * 1024;
        fs.disk().as_bytes()[start..start + 1024]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    fn names(fs: &ExtFs, path: &str) -> Vec<String> {
        fs.list_dir(path)
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect()
    }

    #[test]
    fn a_small_file_round_trips_with_now_on_every_time() {
        let mut fs = default_fs();
        let now = DateTime::new(2026, 9, 23, 12, 0, 0);
        let secs = now.to_unix_seconds() as u32;
        fs.set_now(now);
        let rec = fs.create_file("/hello.txt", b"hello, ext2\n").unwrap();
        assert_eq!(rec.op, "create_file /hello.txt");
        for kind in [
            "inode_allocated",
            "bitmap_updated",
            "blocks_allocated",
            "counters_updated",
            "data_written",
            "dir_entry_written",
        ] {
            assert!(count(&rec, kind) > 0, "no {kind} event");
        }
        assert_eq!(count(&rec, "data_written"), 1);
        assert_eq!(
            count(&rec, "inode_written"),
            2,
            "the new inode and the parent"
        );
        assert_eq!(count(&rec, "indirect_written"), 0);
        fs.create_file("/b.txt", b"").unwrap();
        assert_eq!(fs.read_file("/hello.txt").unwrap(), b"hello, ext2\n");
        assert_eq!(fs.read_file("/b.txt").unwrap(), b"");
        assert_eq!(names(&fs, "/"), ["lost+found", "hello.txt", "b.txt"]);
        let info = fs.stat("/hello.txt").unwrap();
        assert!(!info.is_dir);
        assert_eq!(info.size, 12);
        assert_eq!(
            (info.created, info.modified, info.accessed),
            (None, Some(now), Some(now))
        );
        let inode = fs.inode(12).unwrap();
        assert_eq!(
            (inode.mode, inode.links_count, inode.size, inode.blocks),
            (0x81A4, 1, 12, 2)
        );
        assert_eq!(
            (inode.atime, inode.ctime, inode.mtime, inode.dtime),
            (secs, secs, secs, 0)
        );
        assert_eq!(fs.geometry().group_of_block(inode.block[0]), 0);
        assert!(inode.block[1..].iter().all(|&b| b == 0));
        let data_at = inode.block[0] as usize * 1024;
        let bytes = fs.disk().as_bytes();
        assert_eq!(&bytes[data_at..data_at + 12], b"hello, ext2\n");
        assert!(bytes[data_at + 12..data_at + 1024].iter().all(|&b| b == 0));
        let empty = fs.inode(13).unwrap();
        assert_eq!((empty.size, empty.blocks, empty.block), (0, 0, [0; 15]));
        let root = fs.inode(ROOT_INO).unwrap();
        assert_eq!((root.mtime, root.ctime), (secs, secs));
        assert_eq!(
            dir_block(&fs, root.block[0]),
            vec![
                (".".to_string(), ROOT_INO, 2, 12),
                ("..".to_string(), ROOT_INO, 2, 12),
                ("lost+found".to_string(), LOST_FOUND_INO, 2, 20),
                ("hello.txt".to_string(), 12, 1, 20),
                ("b.txt".to_string(), 13, 1, 960),
            ]
        );
        assert_eq!(fs.superblock().wtime, secs);
        assert_eq!(fs.history().len(), 2);
    }

    #[test]
    fn counters_follow_a_hand_computation_and_every_op_rewinds() {
        let mut fs = default_fs();
        assert_eq!(
            counters(&fs),
            (16_234, 1_013, vec![(8_111, 501, 2), (8_123, 512, 0)])
        );
        let hello = pattern(100, 1);
        let mid = pattern(20 * 1024, 2);
        let big = pattern(300 * 1024, 3);
        // 1 data block.
        step(
            &mut fs,
            |fs| fs.create_file("/hello.txt", &hello),
            (16_233, 1_012, vec![(8_110, 500, 2), (8_123, 512, 0)]),
        );
        // 20 data blocks + 1 single-indirect.
        step(
            &mut fs,
            |fs| fs.create_file("/mid.bin", &mid),
            (16_212, 1_011, vec![(8_089, 499, 2), (8_123, 512, 0)]),
        );
        // 300 data blocks + single + double + 1 second-level.
        step(
            &mut fs,
            |fs| fs.create_file("/big.bin", &big),
            (15_909, 1_010, vec![(7_786, 498, 2), (8_123, 512, 0)]),
        );
        // 1 directory block, one more used directory.
        step(
            &mut fs,
            |fs| fs.create_dir("/dir"),
            (15_908, 1_009, vec![(7_785, 497, 3), (8_123, 512, 0)]),
        );
        step(
            &mut fs,
            |fs| fs.create_file("/dir/inner.txt", b"inner"),
            (15_907, 1_008, vec![(7_784, 496, 3), (8_123, 512, 0)]),
        );
        step(
            &mut fs,
            |fs| fs.create_dir("/dir/sub"),
            (15_906, 1_007, vec![(7_783, 495, 4), (8_123, 512, 0)]),
        );
        // An empty file takes an inode and no block.
        step(
            &mut fs,
            |fs| fs.create_file("/empty", b""),
            (15_906, 1_006, vec![(7_783, 494, 4), (8_123, 512, 0)]),
        );
        let ops: Vec<&str> = fs.history().iter().map(|r| r.op.as_str()).collect();
        assert_eq!(
            ops,
            [
                "create_file /hello.txt",
                "create_file /mid.bin",
                "create_file /big.bin",
                "create_dir /dir",
                "create_file /dir/inner.txt",
                "create_dir /dir/sub",
                "create_file /empty",
            ]
        );
        assert_eq!(fs.read_file("/hello.txt").unwrap(), hello);
        assert_eq!(fs.read_file("/dir/inner.txt").unwrap(), b"inner");
        // Only primary copies change; group 1's backup keeps the format counts.
        let backup_at = fs.geometry().group(1).superblock_block.unwrap() as usize * 1024;
        let backup = Superblock::decode(&fs.disk().as_bytes()[backup_at..backup_at + 1024]);
        assert_eq!(
            (backup.free_blocks_count, backup.free_inodes_count),
            (16_234, 1_013)
        );
        // Rewinding the whole history returns the formatted image.
        let mut image = fs.disk().as_bytes().to_vec();
        for rec in fs.history().iter().rev() {
            image = rewind(&image, rec);
        }
        assert!(
            image == default_fs().disk().as_bytes(),
            "history does not rewind to the format"
        );
    }

    #[test]
    fn twenty_kib_uses_a_single_indirect_block_and_300_kib_a_double() {
        let mut fs = default_fs();
        let mid = pattern(20 * 1024, 1);
        let rec = fs.create_file("/mid.bin", &mid).unwrap();
        assert_eq!(fs.read_file("/mid.bin").unwrap(), mid);
        let inode = fs.inode(12).unwrap();
        assert_ne!(inode.block[12], 0);
        assert_eq!((inode.block[13], inode.block[14]), (0, 0));
        assert_eq!(inode.blocks, (20 + 1) * 2);
        assert_eq!(count(&rec, "indirect_written"), 1);
        assert_eq!(
            count(&rec, "data_written"),
            1,
            "a fresh disk gives one contiguous run"
        );
        for (i, &b) in inode.block[..12].iter().enumerate() {
            assert_eq!(b, inode.block[0] + i as u32);
        }
        let single = u32s(&fs, inode.block[12]);
        for (i, &p) in single[..8].iter().enumerate() {
            assert_eq!(p, inode.block[0] + 12 + i as u32);
        }
        assert!(single[8..].iter().all(|&p| p == 0));
        assert_eq!(fs.stat("/mid.bin").unwrap().size, 20 * 1024);

        let big = pattern(300 * 1024, 2);
        let rec = fs.create_file("/big.bin", &big).unwrap();
        assert_eq!(fs.read_file("/big.bin").unwrap(), big);
        let inode = fs.inode(13).unwrap();
        assert_ne!(inode.block[12], 0);
        assert_ne!(inode.block[13], 0);
        assert_eq!(inode.block[14], 0);
        assert_eq!(inode.blocks, (300 + 3) * 2);
        assert_eq!(count(&rec, "indirect_written"), 3);
        assert!(u32s(&fs, inode.block[12]).iter().all(|&p| p != 0));
        let double = u32s(&fs, inode.block[13]);
        assert_ne!(double[0], 0);
        assert!(double[1..].iter().all(|&p| p == 0));
        let child = u32s(&fs, double[0]);
        assert!(child[..32].iter().all(|&p| p != 0));
        assert!(child[32..].iter().all(|&p| p == 0));
        assert_eq!(fs.stat("/big.bin").unwrap().size, 300 * 1024);
    }

    #[test]
    fn file_too_large_comes_after_the_name_checks_and_before_disk_full() {
        let mut fs = default_fs();
        let huge = vec![0u8; MAX_FILE_BYTES + 1];
        assert_eq!(
            fs.create_file("/huge", &huge).unwrap_err(),
            Error::FileTooLarge
        );
        assert_eq!(
            fs.create_file("/lost+found", &huge).unwrap_err(),
            Error::AlreadyExists
        );
        // Exactly the double-indirect capacity is allowed, but not on 16 MiB.
        assert_eq!(
            fs.create_file("/huge", &huge[..MAX_FILE_BYTES])
                .unwrap_err(),
            Error::DiskFull
        );
        assert!(fs.history().is_empty());
        assert_eq!(names(&fs, "/"), ["lost+found"]);
    }

    #[test]
    fn name_and_path_errors_come_in_the_spec_order_and_write_nothing() {
        let mut fs = default_fs();
        fs.create_file("/f", b"x").unwrap();
        let before = snapshot(&fs);
        let long = "a".repeat(256);
        let cases: Vec<(String, Error)> = vec![
            ("/f".into(), Error::AlreadyExists),
            ("/lost+found".into(), Error::AlreadyExists),
            ("/f/x".into(), Error::NotADirectory),
            ("/missing/x".into(), Error::NotFound),
            (format!("/{long}"), Error::InvalidName),
            ("/a\0b".into(), Error::InvalidName),
            (format!("/missing/{long}"), Error::InvalidName),
            (format!("/f/{long}"), Error::InvalidName),
            ("/".into(), Error::InvalidPath),
            ("relative".into(), Error::InvalidPath),
        ];
        for (path, expected) in &cases {
            assert_eq!(
                fs.create_file(path, b"data").unwrap_err(),
                *expected,
                "create_file {path:?}"
            );
            assert_eq!(
                fs.create_dir(path).unwrap_err(),
                *expected,
                "create_dir {path:?}"
            );
        }
        assert_eq!(snapshot(&fs), before);
        // 255 bytes is the longest name; names are case-sensitive.
        let longest = format!("/{}", "b".repeat(255));
        fs.create_file(&longest, b"").unwrap();
        fs.create_file("/F", b"upper").unwrap();
        assert_eq!(fs.read_file("/F").unwrap(), b"upper");
        assert_eq!(fs.read_file("/f").unwrap(), b"x");
        assert_eq!(fs.list_dir("/").unwrap().len(), 4);
    }

    #[test]
    fn create_dir_sets_dot_entries_links_and_used_dirs() {
        let mut fs = default_fs();
        let rec = fs.create_dir("/dir").unwrap();
        assert_eq!(rec.op, "create_dir /dir");
        assert_eq!(
            count(&rec, "dir_entry_written"),
            3,
            ". and .. and the entry in /"
        );
        let dir = fs.inode(12).unwrap();
        assert!(dir.is_dir());
        assert_eq!(
            (dir.mode, dir.links_count, dir.size, dir.blocks),
            (0x41ED, 2, 1024, 2)
        );
        assert!(dir.block[1..].iter().all(|&b| b == 0));
        assert_eq!(
            dir_block(&fs, dir.block[0]),
            vec![
                (".".to_string(), 12, 2, 12),
                ("..".to_string(), ROOT_INO, 2, 1012),
            ]
        );
        assert_eq!(fs.inode(ROOT_INO).unwrap().links_count, 4);
        let root_entries = dir_block(&fs, fs.inode(ROOT_INO).unwrap().block[0]);
        assert_eq!(
            root_entries.last().unwrap(),
            &("dir".to_string(), 12, 2, 980)
        );
        assert_eq!(
            counters(&fs),
            (16_233, 1_012, vec![(8_110, 500, 3), (8_123, 512, 0)])
        );
        fs.create_dir("/dir/sub").unwrap();
        assert_eq!(fs.inode(12).unwrap().links_count, 3);
        assert_eq!(fs.inode(ROOT_INO).unwrap().links_count, 4);
        let sub = fs.inode(13).unwrap();
        assert_eq!(sub.links_count, 2);
        assert_eq!(
            dir_block(&fs, sub.block[0])[1],
            ("..".to_string(), 12, 2, 1012)
        );
        assert_eq!(
            counters(&fs),
            (16_232, 1_011, vec![(8_109, 499, 4), (8_123, 512, 0)])
        );
        let info = fs.stat("/dir").unwrap();
        assert!(info.is_dir);
        assert_eq!(info.size, 1024);
        assert_eq!(names(&fs, "/dir"), ["sub"]);
        assert!(names(&fs, "/dir/sub").is_empty());
        fs.create_file("/dir/sub/deep.txt", b"deep").unwrap();
        assert_eq!(fs.read_file("/dir/sub/deep.txt").unwrap(), b"deep");
    }

    #[test]
    fn a_directory_grows_a_block_when_its_blocks_are_full() {
        let mut fs = default_fs();
        fs.create_dir("/names").unwrap();
        // 27-byte names: 36-byte entries, 27 fit after . and .., 28 in a fresh block.
        let entries: Vec<String> = (0..60)
            .map(|i| format!("entry-with-a-longer-name-{i:02}"))
            .collect();
        let mut grew = Vec::new();
        for (i, name) in entries.iter().enumerate() {
            let image = (i == 27 || i == 55).then(|| fs.disk().as_bytes().to_vec());
            let rec = fs.create_file(&format!("/names/{name}"), b"").unwrap();
            if count(&rec, "blocks_allocated") > 0 {
                grew.push(i);
            }
            if let Some(image) = image {
                assert!(
                    rewind(fs.disk().as_bytes(), &rec) == image,
                    "growth by entry {i} does not rewind"
                );
            }
        }
        assert_eq!(grew, [27, 55]);
        let dir = fs.inode(12).unwrap();
        assert_eq!((dir.size, dir.blocks), (3 * 1024, 6));
        assert!(dir.block[..3].iter().all(|&b| b != 0));
        assert_eq!(dir.block[3], 0);
        assert_eq!(dir_block(&fs, dir.block[0]).len(), 2 + 27);
        assert_eq!(dir_block(&fs, dir.block[1]).len(), 28);
        assert_eq!(dir_block(&fs, dir.block[2]).len(), 5);
        assert_eq!(names(&fs, "/names"), entries);
        assert_eq!(fs.stat("/names").unwrap().size, 3 * 1024);
        assert_eq!(
            counters(&fs),
            (
                16_234 - 3,
                1_013 - 61,
                vec![(8_111 - 3, 501 - 61, 3), (8_123, 512, 0)]
            )
        );
    }

    #[test]
    fn inodes_follow_the_parent_group_and_blocks_follow_the_inode_group() {
        let mut fs = small_groups_disk();
        assert_eq!(fs.geometry().groups, 2);
        assert_eq!(
            counters(&fs),
            (16_358, 21, vec![(8_173, 5, 2), (8_185, 16, 0)])
        );
        // Group 0 has inodes 12..=16 free; five files in / take them all.
        for i in 0..5 {
            fs.create_file(&format!("/f{i}"), b"").unwrap();
        }
        assert_eq!(counters(&fs).2[0], (8_173, 0, 2));
        // The goal group (the root's, 0) is full, so /dir takes group 1's first inode.
        fs.create_dir("/dir").unwrap();
        let geo = fs.geometry().clone();
        let first = geo.group(1).first_data;
        assert_eq!(geo.group_of_inode(17), 1);
        let dir = fs.inode(17).unwrap();
        assert!(dir.is_dir());
        assert_eq!(dir.block[0], first);
        // Under /dir: the inode from the parent's group 1, the blocks from the inode's group 1.
        let inner = pattern(3000, 3);
        fs.create_file("/dir/inner.bin", &inner).unwrap();
        assert_eq!(geo.group_of_inode(18), 1);
        let ino = fs.inode(18).unwrap();
        assert_eq!(&ino.block[..3], &[first + 1, first + 2, first + 3]);
        assert!(ino.block[..3].iter().all(|&b| geo.group_of_block(b) == 1));
        assert_eq!(fs.read_file("/dir/inner.bin").unwrap(), inner);
        // In / after group 0 ran out: group 1's next inode, and its block
        // follows the inode's group, not the parent's.
        fs.create_file("/late.txt", b"late").unwrap();
        assert_eq!(geo.group_of_inode(19), 1);
        assert_eq!(geo.group_of_block(fs.inode(19).unwrap().block[0]), 1);
        assert_eq!(
            counters(&fs),
            (16_353, 13, vec![(8_173, 0, 2), (8_180, 13, 1)])
        );
    }

    #[test]
    fn disk_full_on_a_64_block_disk_leaves_the_disk_counters_and_history_as_they_were() {
        let mut fs = tiny_disk();
        assert_eq!(fs.geometry().groups, 1);
        assert_eq!(counters(&fs), (44, 5, vec![(44, 5, 2)]));
        // The count checks: these fail before the operation starts, so
        // nothing is written. 44 data blocks also need a single-indirect
        // block: 45 > 44.
        let before = snapshot(&fs);
        assert_eq!(
            fs.create_file("/big", &pattern(44 * 1024, 4)).unwrap_err(),
            Error::DiskFull
        );
        assert_eq!(snapshot(&fs), before);
        // 43 data + 1 indirect fit exactly.
        let big = pattern(43 * 1024, 4);
        fs.create_file("/big", &big).unwrap();
        assert_eq!(counters(&fs), (0, 4, vec![(0, 4, 2)]));
        let before = snapshot(&fs);
        assert_eq!(fs.create_file("/one", b"1").unwrap_err(), Error::DiskFull);
        assert_eq!(fs.create_dir("/d").unwrap_err(), Error::DiskFull);
        assert_eq!(snapshot(&fs), before);
        // Empty files need only an inode and room in the root block.
        for i in 0..3 {
            fs.create_file(&format!("/{}{i}", "n".repeat(254)), b"")
                .unwrap();
        }
        // Directory growth: the root block has 176 bytes left and one inode
        // is free; a 200-byte name needs 208, so the directory must grow and
        // no block is free. The counts pass (the file needs no block), so the
        // operation runs: the inode and its bytes are written, then
        // `dir_insert`'s `DiskFull` rolls the disk back and restores the
        // cached counters, leaving disk, counters and history as before.
        let before = snapshot(&fs);
        assert_eq!(
            fs.create_file(&format!("/{}", "m".repeat(200)), b"")
                .unwrap_err(),
            Error::DiskFull
        );
        assert_eq!(snapshot(&fs), before);
        // A short name still fits and takes the last inode.
        fs.create_file("/last", b"").unwrap();
        assert_eq!(counters(&fs), (0, 0, vec![(0, 0, 2)]));
        let before = snapshot(&fs);
        assert_eq!(fs.create_file("/more", b"").unwrap_err(), Error::DiskFull);
        assert_eq!(fs.create_dir("/more").unwrap_err(), Error::DiskFull);
        assert_eq!(snapshot(&fs), before);
        assert_eq!(fs.read_file("/big").unwrap(), big);
    }

    #[test]
    fn creates_work_through_the_trait_object() {
        let mut fs: Box<dyn FileSystem> = Box::new(default_fs());
        fs.create_dir("/d").unwrap();
        fs.create_file("/d/t", b"trait").unwrap();
        assert_eq!(fs.read_file("/d/t").unwrap(), b"trait");
        assert_eq!(fs.history().len(), 2);
        assert_eq!(fs.history()[1].op, "create_file /d/t");
    }
}

/// Task 5: overwrite in place, delete, remove_dir, block owners, and the
/// directory and pointer-block annotations (spec sections 4, 5, 7).
mod write_delete_remove {
    use super::default_fs;
    use ext::bitmap;
    use ext::blockmap::{file_blocks, indirect_blocks, read_pointers};
    use ext::dir::entries_in_block;
    use ext::{BlockOwner, BlockRole, ExtFormatOptions, ExtFs, Superblock};
    use fs_core::{Annotation, DateTime, Error, FileSystem, OpRecord, Result};
    use std::ops::Range;

    const BS: usize = 1024;

    /// Bytes that differ from block to block, so a misplaced block shows.
    fn pattern(len: usize, seed: u8) -> Vec<u8> {
        (0..len)
            .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed) ^ (i / BS) as u8)
            .collect()
    }

    fn secs(t: DateTime) -> u32 {
        t.to_unix_seconds() as u32
    }

    /// The inode number of `path`, read straight from the directory blocks.
    fn ino_of(fs: &ExtFs, path: &str) -> u32 {
        let mut ino = 2;
        for part in path.split('/').filter(|p| !p.is_empty()) {
            let dir = fs.inode(ino).unwrap();
            ino = file_blocks(fs.disk(), &dir)
                .into_iter()
                .flat_map(|b| entries_in_block(fs.disk().read(b as usize * BS, BS)).unwrap())
                .find(|(_, e)| e.inode != 0 && e.name == part.as_bytes())
                .map(|(_, e)| e.inode)
                .unwrap_or_else(|| panic!("{part} is not in the directory holding {path}"));
        }
        ino
    }

    fn blocks_of(fs: &ExtFs, path: &str) -> Vec<u32> {
        file_blocks(fs.disk(), &fs.inode(ino_of(fs, path)).unwrap())
    }

    fn block_used(fs: &ExtFs, block: u32) -> bool {
        let geo = fs.geometry();
        let g = geo.group(geo.group_of_block(block));
        let bits = fs.disk().read(g.block_bitmap as usize * BS, BS);
        bitmap::is_set(bits, (block - g.first_block) as usize)
    }

    fn inode_used(fs: &ExtFs, ino: u32) -> bool {
        let geo = fs.geometry();
        let g = geo.group(geo.group_of_inode(ino));
        let bits = fs.disk().read(g.inode_bitmap as usize * BS, BS);
        bitmap::is_set(bits, ((ino - 1) % geo.inodes_per_group) as usize)
    }

    /// The bitmaps, the descriptors, and the superblock (cached and on
    /// disk) tell the same story.
    fn assert_counters_agree(fs: &ExtFs) {
        let geo = fs.geometry();
        let sb = fs.superblock();
        let (mut free_blocks, mut free_inodes) = (0u32, 0u32);
        for (i, gd) in fs.group_descriptors().iter().enumerate() {
            let g = geo.group(i as u32);
            let bits = fs.disk().read(g.block_bitmap as usize * BS, BS);
            let clear = (0..g.block_count as usize)
                .filter(|&b| !bitmap::is_set(bits, b))
                .count();
            assert_eq!(
                clear, gd.free_blocks_count as usize,
                "group {i} block bitmap"
            );
            let bits = fs.disk().read(g.inode_bitmap as usize * BS, BS);
            let clear = (0..geo.inodes_per_group as usize)
                .filter(|&b| !bitmap::is_set(bits, b))
                .count();
            assert_eq!(
                clear, gd.free_inodes_count as usize,
                "group {i} inode bitmap"
            );
            free_blocks += gd.free_blocks_count as u32;
            free_inodes += gd.free_inodes_count as u32;
        }
        assert_eq!(free_blocks, sb.free_blocks_count);
        assert_eq!(free_inodes, sb.free_inodes_count);
        let on_disk = Superblock::decode(fs.disk().read(1024, 1024));
        assert_eq!(on_disk.free_blocks_count, sb.free_blocks_count);
        assert_eq!(on_disk.free_inodes_count, sb.free_inodes_count);
    }

    /// Run one operation, then check that putting every `before` back in
    /// reverse order restores the image exactly (what the timeline does).
    fn assert_rewinds(fs: &mut ExtFs, op: impl FnOnce(&mut ExtFs) -> Result<OpRecord>) -> OpRecord {
        let before = fs.disk().as_bytes().to_vec();
        let rec = op(fs).unwrap();
        let mut image = fs.disk().as_bytes().to_vec();
        assert!(image != before, "{} changed nothing", rec.op);
        for c in rec.changes.iter().rev() {
            image[c.offset..c.offset + c.before.len()].copy_from_slice(&c.before);
        }
        assert!(image == before, "{} does not rewind byte for byte", rec.op);
        rec
    }

    #[test]
    fn overwrite_growing_within_direct_blocks_keeps_the_first_blocks() {
        let mut fs = default_fs();
        fs.create_file("/f.bin", &pattern(3 * BS, 1)).unwrap();
        let before = blocks_of(&fs, "/f.bin");
        let free = fs.superblock().free_blocks_count;
        let later = DateTime::new(2021, 6, 1, 12, 0, 0);
        fs.set_now(later);
        let rec = fs.write_file("/f.bin", &pattern(10 * BS, 2)).unwrap();
        assert_eq!(rec.op, "write_file /f.bin");
        let after = blocks_of(&fs, "/f.bin");
        assert_eq!(after.len(), 10);
        assert_eq!(
            &after[..3],
            &before[..],
            "the existing blocks are reused in order"
        );
        assert!(after.iter().all(|&b| block_used(&fs, b)));
        assert_eq!(fs.superblock().free_blocks_count, free - 7);
        let inode = fs.inode(ino_of(&fs, "/f.bin")).unwrap();
        assert_eq!(inode.size, 10 * 1024);
        assert_eq!(inode.blocks, 20);
        assert_eq!(inode.block[12], 0);
        assert_eq!(inode.mtime, secs(later));
        assert_eq!(inode.ctime, secs(later));
        assert_eq!(fs.superblock().wtime, secs(later));
        assert_eq!(fs.read_file("/f.bin").unwrap(), pattern(10 * BS, 2));
        assert_counters_agree(&fs);
    }

    #[test]
    fn overwrite_growing_into_single_then_double_indirect_reuses_pointer_blocks() {
        let mut fs = default_fs();
        fs.create_file("/f.bin", &pattern(5 * BS, 1)).unwrap();
        let five = blocks_of(&fs, "/f.bin");
        let free = fs.superblock().free_blocks_count;

        fs.write_file("/f.bin", &pattern(20 * BS, 2)).unwrap();
        let twenty = blocks_of(&fs, "/f.bin");
        let inode = fs.inode(ino_of(&fs, "/f.bin")).unwrap();
        assert_eq!(&twenty[..5], &five[..]);
        let ind = inode.block[12];
        assert_ne!(ind, 0);
        assert!(block_used(&fs, ind));
        assert_eq!(indirect_blocks(fs.disk(), &inode), vec![(ind, 1)]);
        assert_eq!(&read_pointers(fs.disk(), ind)[..8], &twenty[12..]);
        assert!(read_pointers(fs.disk(), ind)[8..].iter().all(|&p| p == 0));
        assert_eq!(inode.blocks, 21 * 2);
        // 15 data blocks, then the single-indirect block after them.
        assert_eq!(fs.superblock().free_blocks_count, free - 16);
        assert!(
            twenty[5..].iter().all(|&b| b < ind),
            "the pointer block follows its data"
        );
        assert_eq!(fs.read_file("/f.bin").unwrap(), pattern(20 * BS, 2));

        fs.write_file("/f.bin", &pattern(300 * BS, 3)).unwrap();
        let big = blocks_of(&fs, "/f.bin");
        let inode = fs.inode(ino_of(&fs, "/f.bin")).unwrap();
        assert_eq!(&big[..20], &twenty[..]);
        assert_eq!(inode.block[12], ind, "the single-indirect block is kept");
        assert_ne!(inode.block[13], 0);
        assert_eq!(indirect_blocks(fs.disk(), &inode).len(), 3);
        assert_eq!(inode.blocks, 303 * 2);
        // 280 more data blocks, then the double-indirect block and its one
        // second-level block; the single-indirect block is reused.
        assert_eq!(fs.superblock().free_blocks_count, free - 16 - 282);
        assert_eq!(fs.read_file("/f.bin").unwrap(), pattern(300 * BS, 3));
        assert_counters_agree(&fs);
    }

    #[test]
    fn overwrite_shrinking_from_double_indirect_to_direct_frees_every_pointer_block() {
        let mut fs = default_fs();
        fs.create_file("/big.bin", &pattern(300 * BS, 1)).unwrap();
        let ino = ino_of(&fs, "/big.bin");
        let data = blocks_of(&fs, "/big.bin");
        let old = fs.inode(ino).unwrap();
        let pointers: Vec<u32> = indirect_blocks(fs.disk(), &old)
            .into_iter()
            .map(|(b, _)| b)
            .collect();
        assert_eq!(pointers.len(), 3);
        let free = fs.superblock().free_blocks_count;

        let rec = fs.write_file("/big.bin", &pattern(4 * BS, 2)).unwrap();
        assert!(rec.event_kinds().contains(&"blocks_freed"));
        let inode = fs.inode(ino).unwrap();
        assert_eq!(inode.size, 4 * 1024);
        assert_eq!(inode.blocks, 8);
        assert!(inode.block[4..].iter().all(|&p| p == 0));
        assert_eq!(blocks_of(&fs, "/big.bin"), data[..4].to_vec());
        assert_eq!(fs.superblock().free_blocks_count, free + 296 + 3);
        for &b in data[4..].iter().chain(&pointers) {
            assert!(!block_used(&fs, b), "block {b} is still marked used");
        }
        assert!(data[..4].iter().all(|&b| block_used(&fs, b)));
        // Freed blocks keep their bytes: the old single-indirect block still lists its data.
        assert_eq!(read_pointers(fs.disk(), old.block[12])[0], data[12]);
        assert_eq!(fs.read_file("/big.bin").unwrap(), pattern(4 * BS, 2));
        assert_counters_agree(&fs);
    }

    #[test]
    fn overwrite_shrinking_step_by_step_clears_pointer_tails() {
        let mut fs = default_fs();
        fs.create_file("/big.bin", &pattern(300 * BS, 1)).unwrap();
        let ino = ino_of(&fs, "/big.bin");
        let data = blocks_of(&fs, "/big.bin");
        let first = fs.inode(ino).unwrap();
        let (ind, dind) = (first.block[12], first.block[13]);
        let mid = read_pointers(fs.disk(), dind)[0];
        let free = fs.superblock().free_blocks_count;

        // 270 blocks: the double-indirect tree keeps two of its 32 pointers.
        fs.write_file("/big.bin", &pattern(270 * BS, 2)).unwrap();
        let inode = fs.inode(ino).unwrap();
        assert_eq!((inode.block[12], inode.block[13]), (ind, dind));
        assert_eq!(inode.blocks, 273 * 2);
        let kept = read_pointers(fs.disk(), mid);
        assert_eq!(&kept[..2], &data[268..270]);
        assert!(kept[2..].iter().all(|&p| p == 0));
        assert_eq!(fs.superblock().free_blocks_count, free + 30);
        assert_eq!(fs.read_file("/big.bin").unwrap(), pattern(270 * BS, 2));

        // 100 blocks: the double-indirect tree goes; the single-indirect block keeps 88.
        fs.write_file("/big.bin", &pattern(100 * BS, 3)).unwrap();
        let inode = fs.inode(ino).unwrap();
        assert_eq!((inode.block[12], inode.block[13]), (ind, 0));
        assert_eq!(inode.blocks, 101 * 2);
        let kept = read_pointers(fs.disk(), ind);
        assert_eq!(&kept[..88], &data[12..100]);
        assert!(kept[88..].iter().all(|&p| p == 0));
        assert!(!block_used(&fs, dind) && !block_used(&fs, mid));
        assert_eq!(fs.superblock().free_blocks_count, free + 30 + 170 + 2);
        assert_eq!(fs.read_file("/big.bin").unwrap(), pattern(100 * BS, 3));
        assert_counters_agree(&fs);
    }

    #[test]
    fn overwrite_past_the_free_space_is_disk_full_and_writes_nothing() {
        let mut fs = ExtFs::format(ExtFormatOptions {
            total_blocks: 64,
            ..Default::default()
        })
        .unwrap();
        fs.create_file("/f", b"small").unwrap();
        let image = fs.disk().as_bytes().to_vec();
        let history = fs.history().len();
        let err = fs.write_file("/f", &pattern(64 * BS, 1)).unwrap_err();
        assert_eq!(err, Error::DiskFull);
        assert!(fs.disk().as_bytes() == &image[..], "nothing may be written");
        assert_eq!(fs.history().len(), history);
        assert_eq!(fs.read_file("/f").unwrap(), b"small");
    }

    #[test]
    fn delete_leaves_remnants_sets_dtime_and_clears_the_bits() {
        let mut fs = default_fs();
        let (free_b, free_i) = (
            fs.superblock().free_blocks_count,
            fs.superblock().free_inodes_count,
        );
        let secret = pattern(20 * BS, 9);
        fs.create_file("/secret.bin", &secret).unwrap();
        let ino = ino_of(&fs, "/secret.bin");
        let data = blocks_of(&fs, "/secret.bin");
        let alive = fs.inode(ino).unwrap();
        let later = DateTime::new(2022, 3, 4, 5, 6, 7);
        fs.set_now(later);

        let rec = fs.delete_file("/secret.bin").unwrap();
        assert_eq!(rec.op, "delete_file /secret.bin");
        for kind in ["dir_entry_removed", "blocks_freed", "inode_freed"] {
            assert!(rec.event_kinds().contains(&kind), "no {kind} event");
        }
        let dead = fs.inode(ino).unwrap();
        assert_eq!(dead.dtime, secs(later));
        assert_eq!(dead.links_count, 0);
        assert_eq!(dead.block, alive.block, "block pointers stay (remnants)");
        assert_eq!(
            (dead.mode, dead.size, dead.blocks),
            (alive.mode, alive.size, alive.blocks)
        );
        assert_eq!(fs.disk().read(data[0] as usize * BS, BS), &secret[..BS]);
        assert!(data.iter().all(|&b| !block_used(&fs, b)));
        assert!(!block_used(&fs, alive.block[12]));
        assert!(!inode_used(&fs, ino));
        assert_eq!(fs.superblock().free_blocks_count, free_b);
        assert_eq!(fs.superblock().free_inodes_count, free_i);
        assert_eq!(fs.stat("/secret.bin").unwrap_err(), Error::NotFound);
        assert!(fs
            .list_dir("/")
            .unwrap()
            .iter()
            .all(|e| e.name != "secret.bin"));
        let root = fs.inode(2).unwrap();
        assert_eq!((root.mtime, root.ctime), (secs(later), secs(later)));
        assert_counters_agree(&fs);
    }

    #[test]
    fn remove_dir_refuses_root_files_and_non_empty_directories() {
        let mut fs = default_fs();
        let (free_b, free_i) = (
            fs.superblock().free_blocks_count,
            fs.superblock().free_inodes_count,
        );
        assert_eq!(fs.remove_dir("/").unwrap_err(), Error::InvalidPath);
        assert_eq!(fs.remove_dir("/nope").unwrap_err(), Error::NotFound);
        fs.create_dir("/d").unwrap();
        fs.create_file("/d/f.txt", b"x").unwrap();
        assert_eq!(fs.inode(2).unwrap().links_count, 4);
        assert_eq!(fs.group_descriptors()[0].used_dirs_count, 3);
        assert_eq!(fs.remove_dir("/d/f.txt").unwrap_err(), Error::NotADirectory);
        assert_eq!(fs.remove_dir("/d").unwrap_err(), Error::DirectoryNotEmpty);
        let d_ino = ino_of(&fs, "/d");
        let d_blocks = blocks_of(&fs, "/d");

        fs.delete_file("/d/f.txt").unwrap();
        let rec = fs.remove_dir("/d").unwrap();
        assert_eq!(rec.op, "remove_dir /d");
        assert_eq!(fs.inode(2).unwrap().links_count, 3);
        assert_eq!(fs.group_descriptors()[0].used_dirs_count, 2);
        assert!(!inode_used(&fs, d_ino));
        assert_eq!(fs.inode(d_ino).unwrap().links_count, 0);
        assert!(d_blocks.iter().all(|&b| !block_used(&fs, b)));
        assert_eq!(fs.superblock().free_blocks_count, free_b);
        assert_eq!(fs.superblock().free_inodes_count, free_i);
        let names: Vec<String> = fs
            .list_dir("/")
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, ["lost+found"]);
        assert_counters_agree(&fs);
    }

    /// Delete `/many/{name}` and drop it from `names`.
    fn delete_from_many(fs: &mut ExtFs, names: &mut Vec<String>, name: &str) -> OpRecord {
        let rec = fs.delete_file(&format!("/many/{name}")).unwrap();
        names.retain(|n| n != name);
        rec
    }

    /// The region of the first event of `kind` in `rec`.
    fn event_region(rec: &OpRecord, kind: &str) -> Range<usize> {
        rec.events
            .iter()
            .find(|e| e.kind() == kind)
            .unwrap_or_else(|| panic!("no {kind} event in {}", rec.op))
            .region()
            .unwrap()
    }

    #[test]
    fn removing_names_from_a_two_block_directory_keeps_rec_len_chains_whole() {
        let mut fs = default_fs();
        let free_blocks = fs.superblock().free_blocks_count;
        fs.create_dir("/many").unwrap();
        let mut names: Vec<String> = (0..60).map(|i| format!("file-{i:02}.txt")).collect();
        // Entry 50 ("file-50.txt") is the first that does not fit in block
        // 0, so it lands at offset 0 of a fresh block: captured to pin the
        // event range of a fresh-block insert below.
        let mut fresh_block_rec = None;
        for (i, name) in names.iter().enumerate() {
            let rec = fs
                .create_file(&format!("/many/{name}"), name.as_bytes())
                .unwrap();
            if i == 50 {
                fresh_block_rec = Some(rec);
            }
        }
        let blocks = blocks_of(&fs, "/many");
        assert_eq!(
            blocks.len(),
            2,
            "60 entries of 20 bytes need a second block"
        );
        let entries = |fs: &ExtFs, k: usize| {
            entries_in_block(fs.disk().read(blocks[k] as usize * BS, BS)).unwrap()
        };
        let check = |fs: &ExtFs, names: &[String]| {
            for k in 0..blocks.len() {
                let mut next = 0;
                for (off, e) in entries(fs, k) {
                    assert_eq!(off, next, "block {k}: entries chain by rec_len");
                    next += e.rec_len as usize;
                }
                assert_eq!(next, BS, "block {k}: the last entry runs to the block end");
            }
            let listed: Vec<String> = fs
                .list_dir("/many")
                .unwrap()
                .into_iter()
                .map(|e| e.name)
                .collect();
            assert_eq!(listed, names);
        };
        check(&fs, &names);
        assert_eq!(entries(&fs, 1)[0].1.name, b"file-50.txt");
        // A fresh block's first entry: the event covers only its own bytes,
        // starting at its own (zero) offset, not some predecessor's.
        assert_eq!(
            event_region(&fresh_block_rec.unwrap(), "dir_entry_written"),
            blocks[1] as usize * BS..blocks[1] as usize * BS + 20,
            "fresh-block insert starts at the entry's own offset"
        );

        // Not first in its block: ".." absorbs its 20 bytes.
        let dotdot = entries(&fs, 0)[1].1.rec_len;
        delete_from_many(&mut fs, &mut names, "file-00.txt");
        assert_eq!(entries(&fs, 0)[1].1.rec_len, dotdot + 20);
        check(&fs, &names);
        // Last in block 0: its predecessor runs to the block end again.
        let file48_off = entries(&fs, 0)
            .into_iter()
            .find(|(_, e)| e.name == b"file-48.txt")
            .unwrap()
            .0;
        let rec = delete_from_many(&mut fs, &mut names, "file-49.txt");
        // Only the predecessor's rec_len bytes were written, so the event
        // starts at its (unchanged) offset, not the removed entry's.
        assert_eq!(
            event_region(&rec, "dir_entry_removed").start,
            blocks[0] as usize * BS + file48_off,
            "merge starts at the shrunk predecessor's offset"
        );
        let (off, last) = entries(&fs, 0).pop().unwrap();
        assert_eq!(last.name, b"file-48.txt");
        assert_eq!(off + last.rec_len as usize, BS);
        check(&fs, &names);
        // First in block 1: the entry stays where it is, with inode 0.
        let rec = delete_from_many(&mut fs, &mut names, "file-50.txt");
        let (off, first) = entries(&fs, 1)[0].clone();
        assert_eq!((off, first.inode, first.rec_len), (0, 0, 20));
        // A first-in-block removal only zeroes its own inode field, so the
        // event keeps the entry's own range.
        assert_eq!(
            event_region(&rec, "dir_entry_removed"),
            blocks[1] as usize * BS + off..blocks[1] as usize * BS + off + first.rec_len as usize,
            "first-in-block removal keeps its own range"
        );
        check(&fs, &names);
        // Right after that unused entry: merged into it.
        delete_from_many(&mut fs, &mut names, "file-51.txt");
        let (_, first) = entries(&fs, 1)[0].clone();
        assert_eq!((first.inode, first.rec_len), (0, 40));
        check(&fs, &names);
        delete_from_many(&mut fs, &mut names, "file-25.txt");
        check(&fs, &names);

        // The freed room is reused: the slack after ".." takes a new name,
        // and the directory does not grow.
        let dotdot_off = entries(&fs, 0)[1].0;
        let rec = fs.create_file("/many/new-name.txt", b"new").unwrap();
        // The insert splits the shrunk ".." entry, so the event starts at
        // its (predecessor's) offset, not the new entry's own offset.
        assert_eq!(
            event_region(&rec, "dir_entry_written").start,
            blocks[0] as usize * BS + dotdot_off,
            "split predecessor: the event starts at .. 's offset"
        );
        names.insert(0, "new-name.txt".to_string());
        assert_eq!(blocks_of(&fs, "/many"), blocks);
        check(&fs, &names);

        // Emptied, the directory keeps both blocks until remove_dir frees them.
        for name in names.clone() {
            delete_from_many(&mut fs, &mut names, &name);
        }
        check(&fs, &names);
        assert_eq!(fs.inode(ino_of(&fs, "/many")).unwrap().size, 2 * 1024);
        fs.remove_dir("/many").unwrap();
        assert!(blocks.iter().all(|&b| !block_used(&fs, b)));
        assert_eq!(fs.superblock().free_blocks_count, free_blocks);
        assert_counters_agree(&fs);
    }

    #[test]
    fn file_operations_on_directories_are_is_a_directory() {
        let mut fs = default_fs();
        for p in ["/", "/lost+found"] {
            assert_eq!(
                fs.write_file(p, b"x").unwrap_err(),
                Error::IsADirectory,
                "write {p}"
            );
            assert_eq!(
                fs.read_file(p).unwrap_err(),
                Error::IsADirectory,
                "read {p}"
            );
            assert_eq!(
                fs.delete_file(p).unwrap_err(),
                Error::IsADirectory,
                "delete {p}"
            );
        }
        assert_eq!(fs.write_file("/nope", b"x").unwrap_err(), Error::NotFound);
        assert_eq!(fs.delete_file("/nope").unwrap_err(), Error::NotFound);
        assert!(fs.history().is_empty());
    }

    #[test]
    fn every_write_delete_and_remove_dir_rewinds_byte_for_byte() {
        let mut fs = default_fs();
        fs.create_dir("/d").unwrap();
        fs.create_file("/d/f.bin", &pattern(5 * BS, 1)).unwrap();
        let grow = assert_rewinds(&mut fs, |fs| {
            fs.write_file("/d/f.bin", &pattern(300 * BS, 2))
        });
        let same = assert_rewinds(&mut fs, |fs| {
            fs.write_file("/d/f.bin", &pattern(300 * BS, 3))
        });
        let shrink = assert_rewinds(&mut fs, |fs| fs.write_file("/d/f.bin", &pattern(2 * BS, 4)));
        let delete = assert_rewinds(&mut fs, |fs| fs.delete_file("/d/f.bin"));
        let remove = assert_rewinds(&mut fs, |fs| fs.remove_dir("/d"));
        for kind in [
            "blocks_allocated",
            "indirect_written",
            "data_written",
            "inode_written",
        ] {
            assert!(grow.event_kinds().contains(&kind), "grow has no {kind}");
        }
        assert!(shrink.event_kinds().contains(&"blocks_freed"));
        for kind in [
            "dir_entry_removed",
            "blocks_freed",
            "inode_freed",
            "inode_written",
        ] {
            assert!(delete.event_kinds().contains(&kind), "delete has no {kind}");
        }
        for kind in ["dir_entry_removed", "blocks_freed", "inode_freed"] {
            assert!(
                remove.event_kinds().contains(&kind),
                "remove_dir has no {kind}"
            );
        }
        for rec in [&grow, &same, &shrink, &delete, &remove] {
            for e in &rec.events {
                assert!(e.region().is_some(), "{} event {e} has no region", rec.op);
                assert!(!e.to_string().is_empty());
            }
        }
        let ops: Vec<&str> = fs.history().iter().map(|r| r.op.as_str()).collect();
        assert_eq!(
            ops[2..].to_vec(),
            vec![
                "write_file /d/f.bin",
                "write_file /d/f.bin",
                "write_file /d/f.bin",
                "delete_file /d/f.bin",
                "remove_dir /d",
            ]
        );
    }

    #[test]
    fn write_delete_and_remove_dir_work_through_the_trait_object() {
        let mut fs: Box<dyn FileSystem> = Box::new(default_fs());
        fs.create_dir("/d").unwrap();
        fs.create_file("/d/t.txt", b"one").unwrap();
        fs.write_file("/d/t.txt", b"two, and longer").unwrap();
        assert_eq!(fs.read_file("/d/t.txt").unwrap(), b"two, and longer");
        fs.delete_file("/d/t.txt").unwrap();
        fs.remove_dir("/d").unwrap();
        assert_eq!(fs.list_dir("/").unwrap().len(), 1);
        assert_eq!(fs.history().len(), 5);
    }

    #[test]
    fn block_owners_names_every_directory_data_and_pointer_block() {
        let mut fs = default_fs();
        fs.create_file("/a.txt", &pattern(100, 3)).unwrap();
        fs.create_dir("/docs").unwrap();
        fs.create_file("/docs/big.bin", &pattern(20 * BS, 4))
            .unwrap();
        fs.create_file("/gone.txt", &pattern(3 * BS, 5)).unwrap();
        let gone = blocks_of(&fs, "/gone.txt");
        fs.write_file("/a.txt", &pattern(3 * BS, 6)).unwrap();
        fs.delete_file("/gone.txt").unwrap();
        let owners = fs.block_owners();
        let owner = |inode: u32, path: &str, role: BlockRole| BlockOwner {
            inode,
            path: path.to_string(),
            role,
        };

        for b in blocks_of(&fs, "/") {
            assert_eq!(owners[&b], owner(2, "/", BlockRole::Directory));
        }
        let lost = blocks_of(&fs, "/lost+found");
        assert_eq!(lost.len(), 12);
        for b in lost {
            assert_eq!(owners[&b], owner(11, "/lost+found", BlockRole::Directory));
        }
        let docs = ino_of(&fs, "/docs");
        for b in blocks_of(&fs, "/docs") {
            assert_eq!(owners[&b], owner(docs, "/docs", BlockRole::Directory));
        }
        let big = ino_of(&fs, "/docs/big.bin");
        for b in blocks_of(&fs, "/docs/big.bin") {
            assert_eq!(owners[&b], owner(big, "/docs/big.bin", BlockRole::Data));
        }
        let ind = fs.inode(big).unwrap().block[12];
        assert_eq!(
            owners[&ind],
            owner(big, "/docs/big.bin", BlockRole::Indirect)
        );
        let a = ino_of(&fs, "/a.txt");
        let a_blocks = blocks_of(&fs, "/a.txt");
        assert_eq!(a_blocks.len(), 3);
        for b in a_blocks {
            assert_eq!(owners[&b], owner(a, "/a.txt", BlockRole::Data));
        }
        assert!(
            gone.iter().all(|b| !owners.contains_key(b)),
            "freed blocks have no owner"
        );

        // Every in-use block of every group's data area has exactly one owner.
        let geo = fs.geometry();
        let used: u32 = fs
            .group_descriptors()
            .iter()
            .enumerate()
            .map(|(g, gd)| {
                let g = g as u32;
                geo.group(g).block_count
                    - geo.metadata_blocks_in_group(g)
                    - gd.free_blocks_count as u32
            })
            .sum();
        assert_eq!(owners.len() as u32, used);
        assert!(owners.keys().all(|&b| block_used(&fs, b)));
    }

    #[test]
    fn directory_and_pointer_blocks_are_annotated() {
        let mut fs = default_fs();
        fs.create_file("/hello.txt", b"hi").unwrap();
        fs.create_file("/big.bin", &pattern(20 * BS, 7)).unwrap();
        let find = |notes: &[Annotation], label: &str| -> Annotation {
            notes
                .iter()
                .find(|a| a.label == label)
                .unwrap_or_else(|| panic!("no {label:?} in {notes:#?}"))
                .clone()
        };

        let root_block = fs.inode(2).unwrap().block[0];
        let notes = fs.annotate_sector(root_block as u64);
        assert_eq!(find(&notes, "entry 0 inode").value, "2");
        assert_eq!(find(&notes, "entry 0 name").value, ".");
        assert_eq!(find(&notes, "entry 1 name").value, "..");
        assert_eq!(find(&notes, "entry 2 name").value, "lost+found");
        assert_eq!(find(&notes, "entry 2 file type").value, "2 (directory)");
        let hello = find(&notes, "entry 3 name");
        assert_eq!(hello.value, "hello.txt");
        assert_eq!(hello.range.len(), 9);
        assert_eq!(
            find(&notes, "entry 3 inode").value,
            ino_of(&fs, "/hello.txt").to_string()
        );
        assert_eq!(find(&notes, "entry 3 rec_len").value, "20");
        assert_eq!(find(&notes, "entry 3 name_len").value, "9");
        assert_eq!(find(&notes, "entry 3 file type").value, "1 (file)");
        let mut ranges: Vec<_> = notes.iter().map(|a| a.range.clone()).collect();
        ranges.sort_by_key(|r| r.start);
        assert!(
            ranges.windows(2).all(|w| w[0].end <= w[1].start),
            "annotations overlap"
        );
        assert_eq!(ranges.last().unwrap().end, BS);

        // A merged entry vanishes into its predecessor's slack.
        fs.delete_file("/hello.txt").unwrap();
        let notes = fs.annotate_sector(root_block as u64);
        assert_eq!(find(&notes, "entry 2 rec_len").value, "40");
        assert_eq!(find(&notes, "entry 2 slack").value, "unused (22 bytes)");
        assert_eq!(find(&notes, "entry 3 name").value, "big.bin");
        assert!(notes.iter().all(|a| a.value != "hello.txt"));

        // An unused entry spanning a block is one annotation.
        let lost = blocks_of(&fs, "/lost+found");
        let notes = fs.annotate_sector(lost[1] as u64);
        assert_eq!(notes.len(), 1);
        assert_eq!(
            (notes[0].label.as_str(), notes[0].value.as_str()),
            ("entry 0", "unused (1024 bytes)")
        );
        assert_eq!(notes[0].range, 0..BS);

        // A single-indirect block: one run of pointers, then the unused rest.
        let big = fs.inode(ino_of(&fs, "/big.bin")).unwrap();
        let data = file_blocks(fs.disk(), &big);
        assert!(data[12..20].windows(2).all(|w| w[1] == w[0] + 1));
        let notes = fs.annotate_sector(big.block[12] as u64);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].label, "pointer[0..7]");
        assert_eq!(notes[0].value, format!("blocks {}..{}", data[12], data[19]));
        assert_eq!(notes[0].range, 0..32);
        assert_eq!(notes[1].label, "pointer[8..255]");
        assert_eq!(notes[1].value, "unused");
        assert_eq!(notes[1].range, 32..BS);

        // Plain data blocks carry no annotations.
        assert!(fs.annotate_sector(data[0] as u64).is_empty());
    }

    /// Overwrite inode `ino`'s 128-byte slot through a raw write.
    fn plant_inode(fs: &mut ExtFs, ino: u32, inode: &ext::Inode) {
        let (block, offset) = fs.geometry().inode_location(ino);
        let mut raw = [0u8; 128];
        inode.encode(&mut raw);
        fs.write_raw((block as usize * BS + offset) as u64, &raw)
            .unwrap();
    }

    #[test]
    fn a_fast_symlink_is_never_followed_or_freed() {
        let mut fs = default_fs();
        let victim_data = pattern(BS, 7);
        fs.create_file("/victim.bin", &victim_data).unwrap();
        let victim = blocks_of(&fs, "/victim.bin")[0];
        fs.create_file("/link", b"").unwrap();
        let link = ino_of(&fs, "/link");

        // A fast symlink keeps its target text in `i_block`: a target whose
        // bytes read as the victim's block number (for block 82, "R").
        let mut target = victim.to_le_bytes().to_vec();
        while target.last() == Some(&0) {
            target.pop();
        }
        let mut inode = fs.inode(link).unwrap();
        inode.mode = 0xA1FF;
        inode.size = target.len() as u32;
        inode.block[0] = victim;
        plant_inode(&mut fs, link, &inode);
        assert_eq!(
            fs.disk().read(fs_offset(&fs, link) + 40, target.len()),
            &target[..]
        );

        let before = fs.disk().as_bytes().to_vec();
        let history = fs.history().len();
        let unsupported = Err(Error::Unsupported(
            "symbolic links and device nodes are not supported".into(),
        ));
        assert_eq!(fs.delete_file("/link").map(|_| ()), unsupported);
        assert_eq!(
            fs.write_file("/link", b"overwritten").map(|_| ()),
            unsupported
        );
        assert_eq!(fs.read_file("/link").map(|_| ()), unsupported);
        assert!(
            block_used(&fs, victim),
            "the victim's block stays allocated"
        );
        assert_eq!(fs.read_file("/victim.bin").unwrap(), victim_data);
        assert!(fs.disk().as_bytes() == &before[..], "nothing was written");
        assert_eq!(fs.history().len(), history);

        // Listing and stat still show it, as a file.
        let info = fs.stat("/link").unwrap();
        assert!(!info.is_dir);
        assert_eq!(info.size, target.len() as u64);
        assert!(fs
            .list_dir("/")
            .unwrap()
            .iter()
            .any(|e| e.name == "link" && !e.is_dir));
    }

    /// The byte offset of inode `ino`'s slot.
    fn fs_offset(fs: &ExtFs, ino: u32) -> usize {
        let (block, offset) = fs.geometry().inode_location(ino);
        block as usize * BS + offset
    }

    #[test]
    fn a_file_whose_mapping_stops_short_is_not_written_or_deleted() {
        let mut fs = default_fs();
        fs.create_file("/sparse.bin", &pattern(3 * BS, 3)).unwrap();
        let ino = ino_of(&fs, "/sparse.bin");
        let blocks = blocks_of(&fs, "/sparse.bin");
        assert_eq!(blocks.len(), 3);
        // A hole at logical block 1: `i_size` still claims three blocks.
        let mut inode = fs.inode(ino).unwrap();
        inode.block[1] = 0;
        plant_inode(&mut fs, ino, &inode);

        let before = fs.disk().as_bytes().to_vec();
        let history = fs.history().len();
        assert!(matches!(
            fs.write_file("/sparse.bin", b"short"),
            Err(Error::CorruptImage(_))
        ));
        assert!(matches!(
            fs.write_file("/sparse.bin", &pattern(20 * BS, 4)),
            Err(Error::CorruptImage(_))
        ));
        assert!(matches!(
            fs.delete_file("/sparse.bin"),
            Err(Error::CorruptImage(_))
        ));
        assert!(matches!(
            fs.read_file("/sparse.bin"),
            Err(Error::CorruptImage(_))
        ));
        for block in blocks {
            assert!(block_used(&fs, block), "block {block} stays allocated");
        }
        assert!(fs.disk().as_bytes() == &before[..], "nothing was written");
        assert_eq!(fs.history().len(), history);
    }

    #[test]
    fn a_clock_before_1970_still_writes_a_nonzero_dtime() {
        let mut fs = default_fs();
        fs.create_file("/old.txt", b"old").unwrap();
        let ino = ino_of(&fs, "/old.txt");
        fs.set_now(DateTime::new(1969, 12, 31, 23, 59, 59));
        fs.delete_file("/old.txt").unwrap();
        assert_eq!(fs.inode(ino).unwrap().dtime, 1);
    }

    #[test]
    fn frees_saturate_counters_a_raw_write_left_at_the_maximum() {
        let mut fs = default_fs();
        fs.create_file("/f.bin", &pattern(3 * BS, 1)).unwrap();
        // Group 0's descriptor: free blocks at 12, free inodes at 14.
        fs.write_raw(2 * BS as u64 + 12, &[0xFF, 0xFF, 0xFF, 0xFF])
            .unwrap();
        assert!(fs.corruption().is_none());
        fs.delete_file("/f.bin").unwrap();
        let gd = &fs.group_descriptors()[0];
        assert_eq!(
            (gd.free_blocks_count, gd.free_inodes_count),
            (0xFFFF, 0xFFFF)
        );
    }
}
