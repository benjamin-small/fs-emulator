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
    let again = Volume::from_image(&img).unwrap();
    assert_eq!(Array::from(&again.list_dir("/").unwrap()).length(), 0);
    assert_eq!(again.history_length(), 0);
    assert_eq!(
        code(Volume::from_image(&img[..100]).unwrap_err()),
        "CorruptImage"
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
