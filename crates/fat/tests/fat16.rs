use fat::{FatEntry, FatFs, FormatOptions};
use fs_core::{DateTime, Error, FileSystem};

fn default_fs() -> FatFs {
    FatFs::format(FormatOptions::default()).unwrap()
}

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

fn free_clusters(fs: &FatFs) -> usize {
    fs.fat_entries(0)
        .iter()
        .filter(|e| **e == FatEntry::Free)
        .count()
}

#[test]
fn formatted_volume_is_a_valid_fat16() {
    let fs = default_fs();
    let boot = fs.boot_sector();
    assert_eq!(boot.bytes_per_sector, 512);
    assert_eq!(&boot.fs_type, b"FAT16   ");
    assert_eq!(&fs.disk().sector(0)[510..], &[0x55, 0xAA]);
    let g = fs.geometry();
    assert!((4085..65525).contains(&g.cluster_count));
    let entries = fs.fat_entries(0);
    assert_eq!(entries[0], FatEntry::Reserved);
    assert!(entries[2..].iter().all(|e| *e == FatEntry::Free));
    let layout = fs.layout();
    assert_eq!(layout.first().unwrap().sectors.start, 0);
    assert_eq!(layout.last().unwrap().sectors.end, g.total_sectors);
    for pair in layout.windows(2) {
        assert_eq!(pair[0].sectors.end, pair[1].sectors.start);
    }
}

#[test]
fn files_of_every_size_shape_round_trip() {
    let mut fs = default_fs();
    let cs = fs.geometry().cluster_size();
    let cases = [0usize, 1, 100, cs - 1, cs, cs + 1, cs * 2, cs * 3 + 17];
    for (i, len) in cases.iter().enumerate() {
        let data: Vec<u8> = (0..*len).map(|b| (b * 7 + i) as u8).collect();
        fs.create_file(&format!("/F{i}.BIN"), &data).unwrap();
        assert_eq!(fs.read_file(&format!("/F{i}.BIN")).unwrap(), data);
        assert_eq!(fs.stat(&format!("/F{i}.BIN")).unwrap().size, *len as u64);
    }
    assert_eq!(fs.list_dir("/").unwrap().len(), cases.len());
}

#[test]
fn overwrite_frees_and_reallocates() {
    let mut fs = default_fs();
    let cs = fs.geometry().cluster_size();
    let all = free_clusters(&fs);
    fs.create_file("/F", &vec![1; cs * 4]).unwrap();
    assert_eq!(free_clusters(&fs), all - 4);
    fs.write_file("/F", &vec![2; cs]).unwrap();
    assert_eq!(free_clusters(&fs), all - 1);
    fs.write_file("/F", &vec![3; cs * 6]).unwrap();
    assert_eq!(free_clusters(&fs), all - 6);
    assert_eq!(fs.read_file("/F").unwrap(), vec![3; cs * 6]);
}

#[test]
fn delete_leaves_data_but_frees_everything_else() {
    let mut fs = default_fs();
    let all = free_clusters(&fs);
    fs.create_file("/SECRET.TXT", b"still here").unwrap();
    let first = fs.cluster_chain(2).unwrap()[0];
    fs.delete_file("/SECRET.TXT").unwrap();
    assert_eq!(free_clusters(&fs), all);
    let off = fs.geometry().cluster_offset(first);
    assert_eq!(fs.disk().read(off, 10), b"still here");
    assert_eq!(fs.read_file("/SECRET.TXT"), Err(Error::NotFound));
}

#[test]
fn nested_directories_and_removal_rules() {
    let mut fs = default_fs();
    fs.create_dir("/A").unwrap();
    fs.create_dir("/A/B").unwrap();
    fs.create_dir("/A/B/C").unwrap();
    fs.create_file("/A/B/C/deep.txt", b"deep").unwrap();
    assert_eq!(fs.read_file("/a/b/c/DEEP.TXT").unwrap(), b"deep");
    assert_eq!(fs.remove_dir("/A").unwrap_err(), Error::DirectoryNotEmpty);
    fs.delete_file("/A/B/C/deep.txt").unwrap();
    fs.remove_dir("/A/B/C").unwrap();
    fs.remove_dir("/A/B").unwrap();
    fs.remove_dir("/A").unwrap();
    assert_eq!(fs.list_dir("/").unwrap(), Vec::new());
    assert_eq!(free_clusters(&fs), fs.geometry().cluster_count as usize);
}

#[test]
fn directory_growth_and_root_limits() {
    let mut fs = tiny_fs();
    fs.create_dir("/D").unwrap();
    for i in 0..40 {
        fs.create_file(&format!("/D/F{i}"), b"").unwrap();
    }
    assert_eq!(fs.list_dir("/D").unwrap().len(), 40);
    assert!(fs.cluster_chain(2).unwrap().len() >= 3);
    for i in 0..15 {
        fs.create_file(&format!("/R{i}"), b"").unwrap();
    }
    assert_eq!(
        fs.create_file("/R15", b"").unwrap_err(),
        Error::DirectoryFull
    );
}

#[test]
fn long_names_and_case_insensitivity() {
    let mut fs = default_fs();
    let long = "A very long file name that needs several LFN entries.txt";
    fs.create_file(&format!("/{long}"), b"long").unwrap();
    assert_eq!(fs.list_dir("/").unwrap()[0].name, long);
    assert_eq!(
        fs.read_file(&format!("/{}", long.to_uppercase())).unwrap(),
        b"long"
    );
    assert_eq!(fs.read_file("/AVERYL~1.TXT").unwrap(), b"long");
    for i in 1..=3 {
        fs.create_file(&format!("/A very long name number {i}.txt"), b"")
            .unwrap();
        assert!(fs.stat(&format!("/AVERYL~{}.TXT", i + 1)).is_ok());
    }
    fs.create_file("/héllo wörld.txt", b"u").unwrap();
    assert_eq!(fs.read_file("/HÉLLO WÖRLD.TXT").unwrap(), b"u");
}

#[test]
fn disk_full_leaks_nothing() {
    let mut fs = tiny_fs();
    let cs = fs.geometry().cluster_size();
    let free = free_clusters(&fs);
    fs.create_file("/A", &vec![1; (free - 2) * cs]).unwrap();
    assert_eq!(
        fs.create_file("/B", &vec![2; 3 * cs]).unwrap_err(),
        Error::DiskFull
    );
    assert_eq!(free_clusters(&fs), 2);
    assert_eq!(fs.list_dir("/").unwrap().len(), 1);
    fs.create_file("/B", &vec![2; 2 * cs]).unwrap();
    assert_eq!(free_clusters(&fs), 0);
}

#[test]
fn export_and_reimport_preserve_everything() {
    let mut fs = default_fs();
    fs.set_now(DateTime::new(2026, 9, 21, 15, 0, 0));
    fs.create_dir("/DOCS").unwrap();
    fs.create_file("/DOCS/Notes for later.txt", b"remember")
        .unwrap();
    let image = fs.disk().as_bytes().to_vec();
    let again = FatFs::from_image(image).unwrap();
    assert_eq!(
        again.read_file("/DOCS/Notes for later.txt").unwrap(),
        b"remember"
    );
    let info = again.stat("/DOCS/Notes for later.txt").unwrap();
    assert_eq!(info.modified, Some(DateTime::new(2026, 9, 21, 15, 0, 0)));
    assert!(again.history().is_empty());
}

#[test]
fn from_image_rejects_wrong_signature_and_fat12() {
    let fs = default_fs();
    let mut bad = fs.disk().as_bytes().to_vec();
    bad[510] = 0;
    assert!(matches!(
        FatFs::from_image(bad),
        Err(Error::CorruptImage(_))
    ));
    let mut fat12 = tiny_fs().disk().as_bytes().to_vec();
    fat12[54..62].copy_from_slice(b"FAT12   ");
    assert!(matches!(
        FatFs::from_image(fat12),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn every_mutating_op_records_changes_events_and_history() {
    let mut fs = default_fs();
    let records = vec![
        fs.create_dir("/D").unwrap(),
        fs.create_file("/D/F", b"1").unwrap(),
        fs.write_file("/D/F", b"22").unwrap(),
        fs.delete_file("/D/F").unwrap(),
        fs.remove_dir("/D").unwrap(),
        fs.write_raw(0x4000, b"raw").unwrap(),
    ];
    for r in &records {
        assert!(!r.changes.is_empty(), "{} recorded no byte changes", r.op);
        assert!(!r.events.is_empty(), "{} recorded no events", r.op);
        for e in &r.events {
            assert!(e.region().is_some());
            assert!(!e.to_string().is_empty());
        }
    }
    assert!(records[0].event_kinds().contains(&"cluster_allocated"));
    assert!(records[2].event_kinds().contains(&"cluster_freed"));
    assert!(records[3].event_kinds().contains(&"dir_entry_deleted"));
    assert_eq!(records[5].op, "write_raw 0x4000 +3");
    assert_eq!(records[5].event_kinds(), vec!["raw_write"]);
    assert_eq!(fs.history().len(), 6);
    assert_eq!(fs.history()[4].op, "remove_dir /D");
    assert_eq!(fs.history()[5].op, "write_raw 0x4000 +3");
    assert!(fs.list_dir("/").is_ok());
    assert_eq!(fs.history().len(), 6);
    let sectors = records[1].changed_sectors(512);
    assert!(
        sectors.len() >= 3,
        "create_file should touch FAT, directory and data sectors"
    );
}

#[test]
fn usable_as_a_trait_object() {
    let mut fs: Box<dyn FileSystem> = Box::new(default_fs());
    fs.create_file("/T", b"trait").unwrap();
    assert_eq!(fs.read_file("/T").unwrap(), b"trait");
    assert_eq!(fs.fs_type(), "FAT16");
}

#[test]
fn raw_write_rewinds_byte_for_byte_from_its_record() {
    let mut fs = default_fs();
    fs.create_file("/F", b"hello world").unwrap();
    let image_before = fs.disk().as_bytes().to_vec();
    let rec = fs.write_raw(0x1234, b"RAW").unwrap();
    assert_eq!(rec.changes.len(), 1);
    assert_eq!(rec.changes[0].offset, 0x1234);
    assert_eq!(rec.changes[0].before, vec![0, 0, 0]);
    assert_eq!(rec.changes[0].after, b"RAW".to_vec());
    assert_ne!(fs.disk().as_bytes(), &image_before[..]);
    // Rewinding is what the explorer's timeline does: put `before` back.
    let mut rewound = fs.disk().as_bytes().to_vec();
    for change in rec.changes.iter().rev() {
        rewound[change.offset..change.offset + change.before.len()].copy_from_slice(&change.before);
    }
    assert_eq!(rewound, image_before);
}

#[test]
fn raw_write_into_a_file_cluster_changes_what_read_file_returns() {
    let mut fs = default_fs();
    fs.create_file("/F", b"hello world").unwrap();
    let first = fs.cluster_chain(2).unwrap()[0];
    let off = fs.geometry().cluster_offset(first);
    fs.write_raw(off as u64, b"HELLO").unwrap();
    assert_eq!(fs.read_file("/F").unwrap(), b"HELLO world");
    assert_eq!(fs.stat("/F").unwrap().size, 11);
    assert_eq!(fs.history().len(), 2);
}

#[test]
fn clobbering_the_boot_sector_matches_from_image_judgement() {
    let mut fs = default_fs();
    fs.create_file("/A", b"a").unwrap();
    fs.write_raw(510, &[0, 0]).unwrap();
    assert!(matches!(fs.list_dir("/"), Err(Error::CorruptImage(_))));
    assert!(matches!(fs.corruption(), Some(Error::CorruptImage(_))));
    assert!(matches!(
        FatFs::from_image(fs.disk().as_bytes().to_vec()),
        Err(Error::CorruptImage(_))
    ));
    assert_eq!(fs.layout().len(), 5);
    assert!(!fs.annotate_sector(0).is_empty());
    fs.write_raw(510, &[0x55, 0xAA]).unwrap();
    assert!(fs.corruption().is_none());
    assert_eq!(fs.list_dir("/").unwrap().len(), 1);
    let again = FatFs::from_image(fs.disk().as_bytes().to_vec()).unwrap();
    assert_eq!(again.read_file("/A").unwrap(), b"a");
}

#[test]
fn raw_write_out_of_bounds_via_the_trait() {
    let mut fs: Box<dyn FileSystem> = Box::new(default_fs());
    assert!(matches!(
        fs.write_raw(u64::MAX, &[0]),
        Err(Error::OutOfBounds { .. })
    ));
    let len = fs.disk().len() as u64;
    assert_eq!(
        fs.write_raw(len, &[0]).unwrap_err(),
        Error::OutOfBounds {
            offset: len,
            len: 1,
            disk_len: len
        }
    );
    assert!(fs.history().is_empty());
    fs.write_raw(len - 1, &[7]).unwrap();
    assert_eq!(fs.history().len(), 1);
    assert_eq!(fs.disk().read(len as usize - 1, 1), &[7]);
}
