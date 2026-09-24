#![cfg(target_arch = "wasm32")]

use fs_emulator_wasm::Volume;
use js_sys::{Array, Object, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

fn get(v: &JsValue, key: &str) -> JsValue {
    Reflect::get(v, &JsValue::from_str(key)).unwrap()
}

fn code(err: JsValue) -> String {
    get(&err, "code").as_string().expect("error has a code")
}

fn obj(pairs: &[(&str, JsValue)]) -> JsValue {
    let o = Object::new();
    for (k, v) in pairs {
        Reflect::set(&o, &JsValue::from_str(k), v).unwrap();
    }
    o.into()
}

fn fresh() -> Volume {
    Volume::format_fat16(JsValue::UNDEFINED).unwrap()
}

#[wasm_bindgen_test]
fn format_and_basic_info() {
    let v = fresh();
    assert_eq!(v.fs_type(), "FAT16");
    assert_eq!(v.sector_size(), 512);
    assert_eq!(v.sector_count(), 32768);
    assert_eq!(v.history_length(), 0);
}

#[wasm_bindgen_test]
fn create_list_read_stat_round_trip() {
    let mut v = fresh();
    let rec = v.create_file("/Hello world.txt", b"hi there").unwrap();
    assert_eq!(
        get(&rec, "op").as_string().unwrap(),
        "create_file /Hello world.txt"
    );
    let changes = Array::from(&get(&rec, "changes"));
    assert!(changes.length() > 0);
    let first = changes.get(0);
    assert!(get(&first, "before").is_instance_of::<Uint8Array>());
    assert!(get(&first, "after").is_instance_of::<Uint8Array>());
    let events = Array::from(&get(&rec, "events"));
    let kinds: Vec<String> = events
        .iter()
        .map(|e| get(&e, "kind").as_string().unwrap())
        .collect();
    assert!(kinds.contains(&"cluster_allocated".to_string()));
    assert!(kinds.contains(&"dir_entry_written".to_string()));
    let e0 = events.get(0);
    assert!(get(&e0, "text").as_string().is_some());
    assert!(!get(&e0, "region").is_null());
    assert!(get(&get(&e0, "region"), "start").as_f64().is_some());

    let list = Array::from(&v.list_dir("/").unwrap());
    assert_eq!(list.length(), 1);
    assert_eq!(
        get(&list.get(0), "name").as_string().unwrap(),
        "Hello world.txt"
    );
    assert_eq!(get(&list.get(0), "isDir").as_bool(), Some(false));
    assert_eq!(v.read_file("/HELLO WORLD.TXT").unwrap(), b"hi there");
    let st = v.stat("/Hello world.txt").unwrap();
    assert_eq!(get(&st, "size").as_f64(), Some(8.0));
    assert!(!get(&st, "created").is_null());
    assert_eq!(v.history_length(), 1);
}

#[wasm_bindgen_test]
fn errors_carry_codes() {
    let mut v = fresh();
    assert_eq!(code(v.read_file("/nope").unwrap_err()), "NotFound");
    v.create_file("/A", b"").unwrap();
    assert_eq!(code(v.create_file("/a", b"").unwrap_err()), "AlreadyExists");
    assert_eq!(code(v.history_at(5).unwrap_err()), "BadArgument");
    assert_eq!(code(v.sector(1 << 30).unwrap_err()), "BadArgument");
    let small = obj(&[("totalSectors", JsValue::from(2048u32))]);
    assert_eq!(
        code(Volume::format_fat16(small).unwrap_err()),
        "InvalidGeometry"
    );
    let tiny = obj(&[
        ("totalSectors", JsValue::from(2048u32)),
        ("enforceFat16Range", JsValue::FALSE),
    ]);
    assert!(Volume::format_fat16(tiny).is_ok());
    let bad = obj(&[("totalSectors", JsValue::from_str("lots"))]);
    assert_eq!(code(Volume::format_fat16(bad).unwrap_err()), "BadArgument");
    let err = v.read_file("/nope").unwrap_err();
    assert!(err.is_instance_of::<js_sys::Error>());
    assert_eq!(get(&err, "message").as_string().unwrap(), "not found");
    let huge = obj(&[
        ("totalSectors", JsValue::from(4_000_000u32)),
        ("sectorsPerCluster", JsValue::from(64u32)),
    ]);
    assert_eq!(code(Volume::format_fat16(huge).unwrap_err()), "BadArgument");
    assert_eq!(
        code(Volume::format_fat16(obj(&[("totalSector", JsValue::from(2048u32))])).unwrap_err()),
        "BadArgument"
    );
}

#[wasm_bindgen_test]
fn history_layout_bytes_and_time() {
    let mut v = fresh();
    v.create_dir("/D").unwrap();
    v.create_file("/D/F", b"1").unwrap();
    v.delete_file("/D/F").unwrap();
    v.remove_dir("/D").unwrap();
    assert_eq!(v.history_length(), 4);
    assert_eq!(
        get(&v.history_at(3).unwrap(), "op").as_string().unwrap(),
        "remove_dir /D"
    );

    let layout = Array::from(&v.layout().unwrap());
    assert_eq!(layout.length(), 5);
    assert_eq!(
        get(&layout.get(1), "kind").as_string().unwrap(),
        "allocationTable"
    );
    assert_eq!(
        get(&get(&layout.get(1), "sectors"), "start").as_f64(),
        Some(1.0)
    );

    let boot = v.sector(0).unwrap();
    assert_eq!(&boot[510..], &[0x55, 0xAA]);
    let ann = Array::from(&v.annotate_sector(0).unwrap());
    assert!(ann.length() > 10);
    assert_eq!(
        get(&ann.get(2), "label").as_string().unwrap(),
        "bytes per sector"
    );
    assert_eq!(
        Array::from(&v.annotate_sector(1 << 30).unwrap()).length(),
        0
    );

    let img = v.image();
    assert_eq!(img.len(), 32768 * 512);
    let again = Volume::from_image(img.clone()).unwrap();
    assert_eq!(Array::from(&again.list_dir("/").unwrap()).length(), 0);
    assert_eq!(again.history_length(), 0);
    assert_eq!(
        code(Volume::from_image(img[..100].to_vec()).unwrap_err()),
        "Unsupported"
    );

    let mut t = fresh();
    let now = obj(&[
        ("year", JsValue::from(2026u32)),
        ("month", JsValue::from(9u32)),
        ("day", JsValue::from(21u32)),
        ("hour", JsValue::from(8u32)),
        ("minute", JsValue::from(30u32)),
        ("second", JsValue::from(0u32)),
    ]);
    t.set_now(now).unwrap();
    t.create_file("/T", b"").unwrap();
    let created = get(&t.stat("/T").unwrap(), "created");
    assert_eq!(get(&created, "year").as_f64(), Some(2026.0));
    assert_eq!(
        code(t.set_now(JsValue::from_str("noon")).unwrap_err()),
        "BadArgument"
    );
}

#[wasm_bindgen_test]
fn fat_inspection() {
    let mut v = fresh();
    v.create_file("/A very long file name.txt", b"x").unwrap();

    let entries = Array::from(&v.fat_entries(0).unwrap());
    assert_eq!(
        get(&entries.get(0), "kind").as_string().unwrap(),
        "reserved"
    );
    assert_eq!(
        get(&entries.get(2), "kind").as_string().unwrap(),
        "endOfChain"
    );
    assert_eq!(get(&entries.get(3), "kind").as_string().unwrap(), "free");
    assert_eq!(Array::from(&v.fat_entries(2).unwrap()).length(), 0);

    let chain = Array::from(&v.cluster_chain(2).unwrap());
    assert_eq!(chain.length(), 1);
    assert_eq!(chain.get(0).as_f64(), Some(2.0));
    assert_eq!(code(v.cluster_chain(0).unwrap_err()), "CorruptImage");

    let raw = Array::from(&v.raw_dir_entries("/").unwrap());
    assert_eq!(get(&raw.get(0), "kind").as_string().unwrap(), "lfn");
    assert_eq!(get(&raw.get(0), "isLast").as_bool(), Some(true));
    assert_eq!(get(&raw.get(1), "kind").as_string().unwrap(), "lfn");
    assert_eq!(get(&raw.get(2), "kind").as_string().unwrap(), "short");
    assert_eq!(
        get(&raw.get(2), "name").as_string().unwrap(),
        "AVERYL~1.TXT"
    );
    assert_eq!(get(&raw.get(3), "kind").as_string().unwrap(), "free");
    assert_eq!(
        code(v.raw_dir_entries("/A very long file name.txt").unwrap_err()),
        "NotADirectory"
    );

    let bs = v.boot_sector().unwrap();
    assert_eq!(get(&bs, "fsType").as_string().unwrap(), "FAT16");
    assert_eq!(get(&bs, "bytesPerSector").as_f64(), Some(512.0));
    let g = v.geometry().unwrap();
    assert_eq!(get(&g, "variant").as_string().unwrap(), "fat16");
    assert_eq!(get(&g, "clusterCount").as_f64(), Some(8167.0));

    let owners = v.cluster_owners().unwrap();
    let list = Array::from(&owners);
    assert_eq!(list.length(), 1);
    assert_eq!(
        get(&list.get(0), "path").as_string().unwrap(),
        "/A very long file name.txt"
    );
    assert_eq!(get(&list.get(0), "cluster").as_f64(), Some(2.0));

    let data_sector = get(&g, "firstDataSector").as_f64().unwrap() as u32;
    let with = v.annotate_sector_with(data_sector, owners).unwrap();
    let plain = v.annotate_sector(data_sector).unwrap();
    assert_eq!(
        js_sys::JSON::stringify(&with).unwrap(),
        js_sys::JSON::stringify(&plain).unwrap()
    );
    assert_eq!(
        get(&Array::from(&plain).get(0), "value")
            .as_string()
            .unwrap(),
        "data of /A very long file name.txt"
    );
    assert_eq!(
        code(
            v.annotate_sector_with(0, JsValue::from_str("nope"))
                .unwrap_err()
        ),
        "BadArgument"
    );
}

#[wasm_bindgen_test]
fn serializer_contract_null_and_bytes() {
    let mut v = fresh();
    v.create_file("/A.TXT", b"a").unwrap();
    v.create_file("/B.TXT", b"b").unwrap();
    v.delete_file("/B.TXT").unwrap();
    // RawEntry::Deleted carries bytes inside a tagged enum variant.
    let raw = Array::from(&v.raw_dir_entries("/").unwrap());
    let deleted = raw.get(1);
    assert_eq!(get(&deleted, "kind").as_string().unwrap(), "deleted");
    let bytes = get(&deleted, "bytes");
    assert!(bytes.is_instance_of::<Uint8Array>());
    assert_eq!(Uint8Array::from(bytes).length(), 32);
    // Option::None must serialize as null, not undefined: zero A.TXT's access date on disk and re-import.
    let mut img = v.image();
    let root = get(&v.geometry().unwrap(), "firstRootDirSector")
        .as_f64()
        .unwrap() as usize
        * 512;
    img[root + 18] = 0;
    img[root + 19] = 0;
    let again = Volume::from_image(img).unwrap();
    let st = again.stat("/A.TXT").unwrap();
    let accessed = get(&st, "accessed");
    assert!(accessed.is_null(), "expected null, got {:?}", accessed);
    assert!(!accessed.is_undefined());
}

#[wasm_bindgen_test]
fn raw_write_and_read_round_trip() {
    let mut v = fresh();
    let rec = v.write_raw(0x1000, &[1, 2, 3]).unwrap();
    assert_eq!(get(&rec, "op").as_string().unwrap(), "write_raw 0x1000 +3");
    let changes = Array::from(&get(&rec, "changes"));
    assert_eq!(changes.length(), 1);
    let c0 = changes.get(0);
    assert_eq!(get(&c0, "offset").as_f64(), Some(4096.0));
    let before = get(&c0, "before");
    let after = get(&c0, "after");
    assert!(before.is_instance_of::<Uint8Array>());
    assert!(after.is_instance_of::<Uint8Array>());
    assert_eq!(Uint8Array::from(before).to_vec(), vec![0, 0, 0]);
    assert_eq!(Uint8Array::from(after).to_vec(), vec![1, 2, 3]);
    let events = Array::from(&get(&rec, "events"));
    assert_eq!(events.length(), 1);
    let e0 = events.get(0);
    assert_eq!(get(&e0, "kind").as_string().unwrap(), "raw_write");
    assert_eq!(
        get(&e0, "text").as_string().unwrap(),
        "wrote 3 raw bytes at 0x1000"
    );
    let region = get(&e0, "region");
    assert_eq!(get(&region, "start").as_f64(), Some(4096.0));
    assert_eq!(get(&region, "end").as_f64(), Some(4099.0));
    assert_eq!(v.read_raw(0x1000, 3).unwrap(), vec![1, 2, 3]);
    assert_eq!(&v.sector(8).unwrap()[..3], &[1, 2, 3]);
    assert_eq!(v.history_length(), 1);
    assert_eq!(
        get(&v.history_at(0).unwrap(), "op").as_string().unwrap(),
        "write_raw 0x1000 +3"
    );
}

#[wasm_bindgen_test]
fn raw_errors_carry_codes() {
    let mut v = fresh();
    let disk_len: u32 = 32768 * 512;
    let err = v.write_raw(u32::MAX, &[1]).unwrap_err();
    assert_eq!(code(err.clone()), "OutOfBounds");
    assert!(get(&err, "message")
        .as_string()
        .unwrap()
        .starts_with("out of bounds:"));
    assert_eq!(
        code(v.write_raw(disk_len - 1, &[1, 2]).unwrap_err()),
        "OutOfBounds"
    );
    assert_eq!(code(v.read_raw(u32::MAX, 1).unwrap_err()), "BadArgument");
    assert_eq!(code(v.read_raw(0, u32::MAX).unwrap_err()), "BadArgument");
    assert_eq!(code(v.read_raw(disk_len, 1).unwrap_err()), "BadArgument");
    assert_eq!(v.read_raw(disk_len - 1, 1).unwrap().len(), 1);
    assert!(v.read_raw(disk_len, 0).unwrap().is_empty());
    assert_eq!(v.history_length(), 0);
}

#[wasm_bindgen_test]
fn corruption_is_null_until_sector_zero_breaks_and_clears_when_repaired() {
    let mut v = fresh();
    v.create_file("/A", b"a").unwrap();
    assert!(v.corruption().unwrap().is_null());
    let saved = v.sector(0).unwrap();
    v.write_raw(0, &vec![0u8; 512]).unwrap();
    let msg = v.corruption().unwrap();
    assert!(msg.is_string(), "expected a message, got {:?}", msg);
    assert!(msg.as_string().unwrap().starts_with("corrupt image:"));
    let err = v.list_dir("/").unwrap_err();
    assert_eq!(code(err.clone()), "CorruptImage");
    assert!(get(&err, "message")
        .as_string()
        .unwrap()
        .contains("boot sector no longer parses after a raw write"));
    assert_eq!(code(v.raw_dir_entries("/").unwrap_err()), "CorruptImage");
    assert_eq!(Array::from(&v.layout().unwrap()).length(), 5);
    assert_eq!(
        get(&v.boot_sector().unwrap(), "fsType")
            .as_string()
            .unwrap(),
        "FAT16"
    );
    assert_eq!(&v.sector(0).unwrap()[510..], &[0, 0]);
    v.write_raw(0, &saved).unwrap();
    assert!(v.corruption().unwrap().is_null());
    assert_eq!(v.read_file("/A").unwrap(), b"a");
    assert_eq!(v.history_length(), 3);
}

fn fresh_ext() -> Volume {
    Volume::format_ext2(JsValue::UNDEFINED).unwrap()
}

fn names(list: &JsValue) -> Vec<String> {
    Array::from(list)
        .iter()
        .map(|e| get(&e, "name").as_string().unwrap())
        .collect()
}

fn message(err: &JsValue) -> String {
    get(err, "message")
        .as_string()
        .expect("error has a message")
}

#[wasm_bindgen_test]
fn ext2_format_and_basic_info() {
    let v = fresh_ext();
    assert_eq!(v.fs_type(), "ext2");
    assert_eq!(v.sector_size(), 1024);
    assert_eq!(v.sector_count(), 16384);
    assert_eq!(v.history_length(), 0);
    assert!(v.corruption().unwrap().is_null());
    assert_eq!(v.block_group_count().unwrap(), 2);
    assert_eq!(v.read_raw(1080, 2).unwrap(), vec![0x53, 0xEF]);
    assert_eq!(names(&v.list_dir("/").unwrap()), vec!["lost+found"]);
    let lf = Array::from(&v.list_dir("/").unwrap()).get(0);
    assert_eq!(get(&lf, "isDir").as_bool(), Some(true));
    assert_eq!(
        Volume::format_ext2(JsValue::NULL).unwrap().sector_count(),
        16384
    );
}

#[wasm_bindgen_test]
fn ext2_layout_names_every_region_of_both_groups() {
    let v = fresh_ext();
    let layout = Array::from(&v.layout().unwrap());
    assert_eq!(
        names(&layout),
        vec![
            "boot block",
            "superblock",
            "group descriptors (group 0)",
            "block bitmap (group 0)",
            "inode bitmap (group 0)",
            "inode table (group 0)",
            "data (group 0)",
            "backup superblock (group 1)",
            "group descriptors (group 1)",
            "block bitmap (group 1)",
            "inode bitmap (group 1)",
            "inode table (group 1)",
            "data (group 1)",
        ]
    );
    let kinds: Vec<String> = layout
        .iter()
        .map(|r| get(&r, "kind").as_string().unwrap())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "boot",
            "boot",
            "metadata",
            "allocationTable",
            "allocationTable",
            "metadata",
            "data",
            "boot",
            "metadata",
            "allocationTable",
            "allocationTable",
            "metadata",
            "data",
        ]
    );
    let span = |i: u32| {
        let s = get(&layout.get(i), "sectors");
        (
            get(&s, "start").as_f64().unwrap(),
            get(&s, "end").as_f64().unwrap(),
        )
    };
    assert_eq!(span(0), (0.0, 1.0));
    assert_eq!(span(5), (5.0, 69.0));
    assert_eq!(span(6), (69.0, 8193.0));
    assert_eq!(span(7), (8193.0, 8194.0));
    assert_eq!(span(12), (8261.0, 16384.0));
    assert!(Array::from(&v.annotate_sector(1).unwrap()).length() > 10);
}

#[wasm_bindgen_test]
fn ext2_create_read_and_image_round_trip() {
    let mut v = fresh_ext();
    let rec = v.create_file("/hello.txt", b"hi there").unwrap();
    assert_eq!(
        get(&rec, "op").as_string().unwrap(),
        "create_file /hello.txt"
    );
    let kinds: Vec<String> = Array::from(&get(&rec, "events"))
        .iter()
        .map(|e| get(&e, "kind").as_string().unwrap())
        .collect();
    for kind in ["inode_allocated", "data_written", "dir_entry_written"] {
        assert!(kinds.contains(&kind.to_string()), "{kind} in {kinds:?}");
    }
    assert_eq!(v.read_file("/hello.txt").unwrap(), b"hi there");
    // ext names are case-sensitive, unlike FAT's.
    assert_eq!(code(v.read_file("/HELLO.TXT").unwrap_err()), "NotFound");
    assert_eq!(
        get(&v.stat("/hello.txt").unwrap(), "size").as_f64(),
        Some(8.0)
    );
    assert_eq!(
        names(&v.list_dir("/").unwrap()),
        vec!["lost+found", "hello.txt"]
    );
    assert_eq!(v.history_length(), 1);

    let img = v.image();
    assert_eq!(img.len(), 16384 * 1024);
    let again = Volume::from_image(img).unwrap();
    assert_eq!(again.fs_type(), "ext2");
    assert_eq!(again.read_file("/hello.txt").unwrap(), b"hi there");
    assert_eq!(again.history_length(), 0);
}

#[wasm_bindgen_test]
fn detection_prefers_ext_and_refuses_unknown_images() {
    let mut v = fresh_ext();
    // A boot signature in ext's boot block does not make the image FAT.
    v.write_raw(510, &[0x55, 0xAA]).unwrap();
    let img = v.image();
    assert_eq!(&img[510..512], &[0x55, 0xAA]);
    assert_eq!(Volume::from_image(img).unwrap().fs_type(), "ext2");
    assert_eq!(
        Volume::from_image(fresh().image()).unwrap().fs_type(),
        "FAT16"
    );
    for bytes in [vec![0u8; 100], vec![0u8; 4096]] {
        let err = Volume::from_image(bytes).unwrap_err();
        assert_eq!(code(err.clone()), "Unsupported");
        assert_eq!(
            message(&err),
            "unsupported: no recognisable filesystem signature"
        );
    }
    // The magic routes a too-short image to the ext parser, which refuses it.
    let mut short = vec![0u8; 1082];
    short[1080] = 0x53;
    short[1081] = 0xEF;
    assert_eq!(code(Volume::from_image(short).unwrap_err()), "CorruptImage");
}

#[wasm_bindgen_test]
fn family_methods_throw_the_other_familys_code() {
    let e = fresh_ext();
    let err = e.fat_entries(0).unwrap_err();
    assert_eq!(code(err.clone()), "NotFat");
    assert_eq!(message(&err), "not a FAT volume");
    assert_eq!(code(e.boot_sector().unwrap_err()), "NotFat");
    assert_eq!(code(e.geometry().unwrap_err()), "NotFat");
    assert_eq!(code(e.cluster_chain(2).unwrap_err()), "NotFat");
    assert_eq!(code(e.raw_dir_entries("/").unwrap_err()), "NotFat");
    assert_eq!(code(e.cluster_owners().unwrap_err()), "NotFat");

    let f = fresh();
    let err = f.block_group_count().unwrap_err();
    assert_eq!(code(err.clone()), "NotExt");
    assert_eq!(message(&err), "not an ext volume");
}

#[wasm_bindgen_test]
fn format_ext2_options_reach_the_superblock_and_bad_ones_throw() {
    let custom = obj(&[
        ("totalBlocks", JsValue::from(8192u32)),
        ("inodesPerGroup", JsValue::from(64u32)),
        ("label", JsValue::from_str("teach")),
        (
            "uuid",
            JsValue::from_str("01234567-89ab-CDEF-0123-456789abcdef"),
        ),
    ]);
    let v = Volume::format_ext2(custom).unwrap();
    assert_eq!(v.sector_count(), 8192);
    assert_eq!(v.block_group_count().unwrap(), 1);
    assert_eq!(v.read_raw(1024 + 40, 4).unwrap(), vec![64, 0, 0, 0]); // s_inodes_per_group
    assert_eq!(
        v.read_raw(1024 + 104, 16).unwrap(), // s_uuid
        vec![
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab,
            0xcd, 0xef
        ]
    );
    assert_eq!(
        v.read_raw(1024 + 120, 16).unwrap(), // s_volume_name
        b"teach\0\0\0\0\0\0\0\0\0\0\0".to_vec()
    );
    let bare = obj(&[(
        "uuid",
        JsValue::from_str("0123456789abcdef0123456789abcdef"),
    )]);
    assert!(Volume::format_ext2(bare).is_ok());
    let sixteen = obj(&[("label", JsValue::from_str("sixteen-bytes!!!"))]);
    assert!(Volume::format_ext2(sixteen).is_ok());

    let fails =
        |key: &str, value: JsValue| code(Volume::format_ext2(obj(&[(key, value)])).unwrap_err());
    assert_eq!(
        fails("uuid", JsValue::from_str("not-a-uuid")),
        "BadArgument"
    );
    assert_eq!(
        fails("uuid", JsValue::from_str("0123456789abcdef0123456789abcde")),
        "BadArgument"
    );
    assert_eq!(
        fails("label", JsValue::from_str("seventeen-bytes!!")),
        "BadArgument"
    );
    assert_eq!(fails("totalBlock", JsValue::from(8192u32)), "BadArgument");
    assert_eq!(
        fails("totalBlocks", JsValue::from_str("lots")),
        "BadArgument"
    );
    let err = Volume::format_ext2(obj(&[("totalBlocks", JsValue::from(262_145u32))])).unwrap_err();
    assert_eq!(code(err.clone()), "BadArgument");
    assert_eq!(
        message(&err),
        "volume of 268436480 bytes exceeds the 268435456-byte limit"
    );
    assert_eq!(
        fails("totalBlocks", JsValue::from(32u32)),
        "InvalidGeometry"
    );
    assert_eq!(
        fails("inodesPerGroup", JsValue::from(20u32)),
        "InvalidGeometry"
    );
}
