# WASM Wrapper Crate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `crates/wasm` crate exposing the emulator to browsers as one wasm-bindgen `Volume` class with typed plain-object results, plus a Vite demo page and CI coverage.

**Architecture:** `dto.rs` holds serde mirrors of every core type and is testable natively; `volume.rs` is the only file that touches wasm-bindgen and delegates to `fs_core::FileSystem` / `fat::FatFs`; `error.rs` maps `fs_core::Error` to a JS `Error` with a `code`; `types.rs` ships hand-written TypeScript. `web/demo` is a plain Vite + TypeScript page importing the wasm-pack output.

**Tech Stack:** Rust 2021, wasm-bindgen 0.2, serde 1 + serde_bytes 0.11 + serde-wasm-bindgen 0.6, js-sys 0.3, wasm-bindgen-test 0.3, wasm-pack 0.15, Vite + TypeScript with vite-plugin-wasm and vite-plugin-top-level-await, pnpm.

**Spec:** `docs/superpowers/specs/2026-09-21-wasm-wrapper-design.md`

## Global Constraints

- `fs-core` and `fat` gain no dependencies; only `crates/wasm` depends on wasm-bindgen/serde/js-sys.
- Crate package name `fs-emulator-wasm`, lib name `fs_emulator_wasm`, `crate-type = ["cdylib", "rlib"]`.
- JS field names are camelCase; enum-like values are tagged unions on `kind`; `Option` serializes as `null`; byte vectors serialize as `Uint8Array`; `u64` serializes as a JS number.
- Serialization uses `serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true).serialize_missing_as_null(true)` (not `json_compatible()`, which would turn bytes into plain arrays).
- Every fallible method returns `Result<_, JsValue>`; the error is a `js_sys::Error` with a `code` property: the `fs_core::Error` variant name, or `NotFat` / `BadArgument` for wrapper-level failures.
- Numbers crossing the boundary are `u32`/`usize`/`u8`, never `u64` (which would become a BigInt).
- `tests/volume.rs` starts with `#![cfg(target_arch = "wasm32")]` so `cargo test --workspace` on the host skips it.
- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo build --workspace --target wasm32-unknown-unknown` clean before each commit; wasm tests via `wasm-pack test --node crates/wasm`.
- Conventional commit messages, no attribution lines.

## File Structure

```
Cargo.toml                          add "crates/wasm" member
.gitignore                          add crates/wasm/pkg/, web/demo/node_modules/, web/demo/dist/
crates/wasm/Cargo.toml
crates/wasm/README.md               build/test/demo commands
crates/wasm/src/lib.rs              modules, re-export Volume, panic hook start fn
crates/wasm/src/dto.rs              serde mirrors + From conversions (native-testable)
crates/wasm/src/error.rs            code_of(), to_js(), js_error()
crates/wasm/src/types.rs            typescript_custom_section
crates/wasm/src/volume.rs           #[wasm_bindgen] Volume
crates/wasm/tests/volume.rs         wasm-bindgen-test boundary tests (wasm32 only)
web/demo/package.json, pnpm-lock.yaml, vite.config.ts, tsconfig.json, index.html, src/main.ts
.github/workflows/ci.yml            add the wasm job
```

---

### Task 1: Crate scaffold and core DTOs

**Files:**
- Create: `crates/wasm/Cargo.toml`, `crates/wasm/src/lib.rs`, `crates/wasm/src/dto.rs`
- Modify: `Cargo.toml` (workspace members), `.gitignore`

**Interfaces:**
- Consumes: `fs_core::{DateTime, EntryInfo, ByteChange, Event, OpRecord, Region, RegionKind, Annotation}`
- Produces: `dto::{DateTime, EntryInfo, Range, Range64, ByteChange, EventRecord, OpRecord, Region, Annotation}` with `From` conversions; `dto::region_kind_name(RegionKind) -> &'static str`; `dto::EventRecord::from_event(&dyn Event)`; `impl From<&fs_core::OpRecord> for dto::OpRecord`.

- [ ] **Step 1: Scaffold**

`crates/wasm/Cargo.toml`:
```toml
[package]
name = "fs-emulator-wasm"
description = "wasm-bindgen wrapper exposing the fs-emulator filesystems to a browser UI"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[lib]
name = "fs_emulator_wasm"
crate-type = ["cdylib", "rlib"]

[dependencies]
fs-core = { path = "../fs-core" }
fat = { path = "../fat" }
wasm-bindgen = "0.2"
serde = { version = "1", features = ["derive"] }
serde_bytes = "0.11"
serde-wasm-bindgen = "0.6"
js-sys = "0.3"
console_error_panic_hook = "0.1"

[dev-dependencies]
wasm-bindgen-test = "0.3"
```

Root `Cargo.toml`: `members = ["crates/fs-core", "crates/fat", "crates/wasm"]`.

`.gitignore` append:
```
crates/wasm/pkg/
web/demo/node_modules/
web/demo/dist/
```

`crates/wasm/src/lib.rs`:
```rust
//! Browser bindings for the emulator: one `Volume` class over the
//! `FileSystem` trait plus FAT-specific inspection, with plain JS objects
//! crossing the boundary.

pub mod dto;
```

- [ ] **Step 2: Failing tests**

`crates/wasm/src/dto.rs` tests module (gated off wasm32 so `wasm-pack test` only sees the wasm-bindgen tests):
```rust
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use fs_core::{Event, RegionKind};
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
        assert_eq!(dto, DateTime { year: 2026, month: 9, day: 21, hour: 1, minute: 2, second: 3 });
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
        assert_eq!(dto.created, Some(DateTime::from(fs_core::DateTime::default())));
        assert_eq!(dto.modified, None);
    }

    #[test]
    fn op_record_maps_changes_and_events() {
        let mut core = fs_core::OpRecord::new("create_file /A");
        core.changes.push(fs_core::ByteChange { offset: 5, before: vec![0, 0], after: vec![1, 2] });
        core.events.push(Box::new(Dummy));
        let dto = OpRecord::from(&core);
        assert_eq!(dto.op, "create_file /A");
        assert_eq!(dto.changes, vec![ByteChange { offset: 5, before: vec![0, 0], after: vec![1, 2] }]);
        assert_eq!(
            dto.events,
            vec![EventRecord { kind: "dummy".into(), text: "did a thing".into(), region: Some(Range { start: 10, end: 12 }) }]
        );
    }

    #[test]
    fn region_and_annotation_map() {
        let r = Region::from(fs_core::Region { name: "FAT 0".into(), sectors: 1..33, kind: RegionKind::AllocationTable });
        assert_eq!(r, Region { name: "FAT 0".into(), sectors: Range64 { start: 1, end: 33 }, kind: "allocationTable".into() });
        assert_eq!(region_kind_name(RegionKind::Boot), "boot");
        assert_eq!(region_kind_name(RegionKind::Other), "other");
        let a = Annotation::from(fs_core::Annotation { range: 11..13, label: "bytes per sector".into(), value: "512".into() });
        assert_eq!(a.range, Range { start: 11, end: 13 });
        assert_eq!(a.label, "bytes per sector");
    }

}
```

Add this test to the same module. It checks the serde field names without pulling in serde_json, using a tiny Serializer that records struct keys:

```rust
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
        impl<'a> SerializeStruct for &'a mut KeyCollector {
            type Ok = ();
            type Error = KeyErr;
            fn serialize_field<T: ?Sized + Serialize>(&mut self, key: &'static str, _: &T) -> Result<(), KeyErr> {
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
        impl<'a> Serializer for &'a mut KeyCollector {
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
            fn serialize_none(self) -> Result<(), KeyErr> { Err(KeyErr) }
            fn serialize_some<T: ?Sized + Serialize>(self, _: &T) -> Result<(), KeyErr> { Err(KeyErr) }
            fn serialize_unit(self) -> Result<(), KeyErr> { Err(KeyErr) }
            fn serialize_unit_struct(self, _: &'static str) -> Result<(), KeyErr> { Err(KeyErr) }
            fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<(), KeyErr> { Err(KeyErr) }
            fn serialize_newtype_struct<T: ?Sized + Serialize>(self, _: &'static str, _: &T) -> Result<(), KeyErr> { Err(KeyErr) }
            fn serialize_newtype_variant<T: ?Sized + Serialize>(self, _: &'static str, _: u32, _: &'static str, _: &T) -> Result<(), KeyErr> { Err(KeyErr) }
            fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, KeyErr> { Err(KeyErr) }
            fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, KeyErr> { Err(KeyErr) }
            fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeTupleStruct, KeyErr> { Err(KeyErr) }
            fn serialize_tuple_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> Result<Self::SerializeTupleVariant, KeyErr> { Err(KeyErr) }
            fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, KeyErr> { Err(KeyErr) }
            fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, KeyErr> { Ok(self) }
            fn serialize_struct_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> Result<Self::SerializeStructVariant, KeyErr> { Err(KeyErr) }
        }
        let dto = EntryInfo { name: "x".into(), is_dir: true, size: 0, created: None, modified: None, accessed: None };
        let mut keys = KeyCollector(Vec::new());
        dto.serialize(&mut keys).unwrap();
        assert_eq!(keys.0, vec!["name", "isDir", "size", "created", "modified", "accessed"]);
    }
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p fs-emulator-wasm`
Expected: compile error (`DateTime` etc. not defined).

- [ ] **Step 4: Implement**

`crates/wasm/src/dto.rs` (above the tests):
```rust
//! Plain-data mirrors of the core types, shaped for JavaScript: camelCase
//! fields, tagged unions on `kind`, `Option` as `null`, bytes as `Uint8Array`.
//! Nothing here touches wasm-bindgen, so it all runs under `cargo test`.

use fs_core::{Event, RegionKind};
use serde::{Deserialize, Serialize};

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
        DateTime { year: d.year, month: d.month, day: d.day, hour: d.hour, minute: d.minute, second: d.second }
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
        Range { start: r.start, end: r.end }
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
        ByteChange { offset: c.offset, before: c.before.clone(), after: c.after.clone() }
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
        EventRecord { kind: e.kind().to_string(), text: e.to_string(), region: e.region().map(Into::into) }
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
            events: r.events.iter().map(|e| EventRecord::from_event(e.as_ref())).collect(),
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
            sectors: Range64 { start: r.sectors.start, end: r.sectors.end },
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
        Annotation { range: a.range.into(), label: a.label, value: a.value }
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p fs-emulator-wasm && cargo clippy --workspace --all-targets -- -D warnings && cargo build --workspace --target wasm32-unknown-unknown`
Expected: 5 tests pass, no warnings, wasm32 build clean.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock .gitignore crates/wasm
git commit -m "feat(wasm): crate scaffold and core DTOs"
```

---

### Task 2: FAT DTOs and FormatOptions

**Files:**
- Modify: `crates/wasm/src/dto.rs`

**Interfaces:**
- Consumes: `fat::{FormatOptions, BootSector, Geometry, FatVariant, FatEntry, RawEntry, ClusterOwner}`, `fat::dir_entry::unpack`, `fat::name::from_ucs2`
- Produces: `dto::FormatOptions` (Deserialize, Default, `impl From<dto::FormatOptions> for fat::FormatOptions`), `dto::pad_label(&str) -> [u8; 11]`, `dto::BootSector: From<&fat::BootSector>`, `dto::Geometry: From<&fat::Geometry>`, `dto::FatEntry: From<fat::FatEntry>`, `dto::RawEntry: From<fat::RawEntry>`, `dto::ClusterOwner` (Serialize + Deserialize), `dto::owners_to_list(&BTreeMap<u32, fat::ClusterOwner>) -> Vec<ClusterOwner>`, `dto::owners_from_list(Vec<ClusterOwner>) -> BTreeMap<u32, fat::ClusterOwner>`.

- [ ] **Step 1: Failing tests (append inside the tests module)**

```rust
    use fat::{FatFs, FormatOptions as FatFormat};
    use std::collections::BTreeMap;

    #[test]
    fn format_options_default_to_the_core_defaults() {
        let dto = FormatOptions::default();
        assert_eq!(FatFormat::from(dto), FatFormat::default());
        let dto = FormatOptions { total_sectors: Some(8192), sectors_per_cluster: Some(1), volume_label: Some("teach".into()), enforce_fat16_range: Some(false), ..Default::default() };
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
        assert_eq!(FatEntry::from(fat::FatEntry::Next(9)), FatEntry::Next { cluster: 9 });
        assert_eq!(FatEntry::from(fat::FatEntry::EndOfChain), FatEntry::EndOfChain);
        let mut fs = FatFs::format(FatFormat::default()).unwrap();
        fs.create_file("/My File.txt", b"abc").unwrap();
        fs.create_file("/B", b"").unwrap();
        fs.delete_file("/B").unwrap();
        let raw: Vec<RawEntry> = fs.raw_dir_entries("/").unwrap().into_iter().map(Into::into).collect();
        assert!(matches!(&raw[0], RawEntry::Lfn { order: 1, is_last: true, text, .. } if text == "My File.txt"));
        assert!(matches!(&raw[1], RawEntry::Short { name, size: 3, is_dir: false, first_cluster: 2, .. } if name == "MYFILE~1.TXT"));
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
        assert_eq!(list[0], ClusterOwner { cluster: 2, path: "/D".into(), is_dir: true, first_cluster: 2 });
        assert_eq!(list[1].path, "/D/F");
        assert_eq!(list[1].first_cluster, 3);
        let back: BTreeMap<u32, fat::ClusterOwner> = owners_from_list(list);
        assert_eq!(back, map);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p fs-emulator-wasm`
Expected: compile error (`FormatOptions` not defined in dto).

- [ ] **Step 3: Implement (append to dto.rs above the tests)**

```rust
use fat::FatVariant;
use std::collections::BTreeMap;

/// Every field optional; absent fields take `fat::FormatOptions::default()`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
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
            volume_label: o.volume_label.map(|l| pad_label(&l)).unwrap_or(d.volume_label),
            volume_id: o.volume_id.unwrap_or(d.volume_id),
            enforce_fat16_range: o.enforce_fat16_range.unwrap_or(d.enforce_fat16_range),
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
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
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
            fat::RawEntry::Deleted { bytes } => RawEntry::Deleted { bytes: bytes.to_vec() },
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
        .map(|(&cluster, o)| ClusterOwner { cluster, path: o.path.clone(), is_dir: o.is_dir, first_cluster: o.first_cluster })
        .collect()
}

pub fn owners_from_list(list: Vec<ClusterOwner>) -> BTreeMap<u32, fat::ClusterOwner> {
    list.into_iter()
        .map(|o| (o.cluster, fat::ClusterOwner { path: o.path, is_dir: o.is_dir, first_cluster: o.first_cluster }))
        .collect()
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p fs-emulator-wasm && cargo clippy --workspace --all-targets -- -D warnings`
Expected: 10 tests pass, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/wasm
git commit -m "feat(wasm): FAT DTOs, FormatOptions, and cluster-owner lists"
```

---

### Task 3: Errors, TypeScript types, and the generic `Volume` surface

**Files:**
- Create: `crates/wasm/src/error.rs`, `crates/wasm/src/types.rs`, `crates/wasm/src/volume.rs`, `crates/wasm/tests/volume.rs`
- Modify: `crates/wasm/src/lib.rs`

**Interfaces:**
- Consumes: everything in `dto`, `fs_core::FileSystem`, `fat::FatFs::{format, from_image}`
- Produces: `error::{code_of(&fs_core::Error) -> &'static str, to_js(fs_core::Error) -> JsValue, js_error(code, message) -> JsValue}`; `volume::Volume` with `Inner`, `fs()`, `fs_mut()`, `fat()`, `to_value()`, and the generic JS methods listed in the spec; `lib::init` start function.

- [ ] **Step 1: Failing native test for error codes**

`crates/wasm/src/error.rs` tests:
```rust
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use fs_core::Error;

    #[test]
    fn every_variant_has_its_own_code() {
        let all = [
            Error::NotFound, Error::AlreadyExists, Error::InvalidPath, Error::InvalidName, Error::DiskFull,
            Error::DirectoryFull, Error::NotADirectory, Error::IsADirectory, Error::DirectoryNotEmpty,
            Error::FileTooLarge, Error::InvalidGeometry("x".into()), Error::CorruptImage("x".into()),
            Error::Unsupported("x".into()),
        ];
        let codes: Vec<&str> = all.iter().map(code_of).collect();
        assert_eq!(codes[0], "NotFound");
        assert_eq!(codes[11], "CorruptImage");
        let mut unique = codes.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), all.len());
    }
}
```

- [ ] **Step 2: Failing boundary tests**

`crates/wasm/tests/volume.rs`:
```rust
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
    assert_eq!(get(&rec, "op").as_string().unwrap(), "create_file /Hello world.txt");
    let changes = Array::from(&get(&rec, "changes"));
    assert!(changes.length() > 0);
    let first = changes.get(0);
    assert!(get(&first, "before").is_instance_of::<Uint8Array>());
    assert!(get(&first, "after").is_instance_of::<Uint8Array>());
    let events = Array::from(&get(&rec, "events"));
    let kinds: Vec<String> = events.iter().map(|e| get(&e, "kind").as_string().unwrap()).collect();
    assert!(kinds.contains(&"cluster_allocated".to_string()));
    assert!(kinds.contains(&"dir_entry_written".to_string()));
    let e0 = events.get(0);
    assert!(get(&e0, "text").as_string().is_some());
    assert!(!get(&e0, "region").is_null());
    assert!(get(&get(&e0, "region"), "start").as_f64().is_some());

    let list = Array::from(&v.list_dir("/").unwrap());
    assert_eq!(list.length(), 1);
    assert_eq!(get(&list.get(0), "name").as_string().unwrap(), "Hello world.txt");
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
    assert_eq!(code(Volume::format_fat16(small).unwrap_err()), "InvalidGeometry");
    let tiny = obj(&[("totalSectors", JsValue::from(2048u32)), ("enforceFat16Range", JsValue::FALSE)]);
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
    assert_eq!(get(&v.history_at(3).unwrap(), "op").as_string().unwrap(), "remove_dir /D");

    let layout = Array::from(&v.layout().unwrap());
    assert_eq!(layout.length(), 5);
    assert_eq!(get(&layout.get(1), "kind").as_string().unwrap(), "allocationTable");
    assert_eq!(get(&get(&layout.get(1), "sectors"), "start").as_f64(), Some(1.0));

    let boot = v.sector(0).unwrap();
    assert_eq!(&boot[510..], &[0x55, 0xAA]);
    let ann = Array::from(&v.annotate_sector(0).unwrap());
    assert!(ann.length() > 10);
    assert_eq!(get(&ann.get(2), "label").as_string().unwrap(), "bytes per sector");
    assert_eq!(Array::from(&v.annotate_sector(1 << 30).unwrap()).length(), 0);

    let img = v.image();
    assert_eq!(img.len(), 32768 * 512);
    let again = Volume::from_image(&img).unwrap();
    assert_eq!(Array::from(&again.list_dir("/").unwrap()).length(), 0);
    assert_eq!(again.history_length(), 0);
    assert_eq!(code(Volume::from_image(&img[..100]).unwrap_err()), "CorruptImage");

    let mut t = fresh();
    let now = obj(&[
        ("year", JsValue::from(2026u32)), ("month", JsValue::from(9u32)), ("day", JsValue::from(21u32)),
        ("hour", JsValue::from(8u32)), ("minute", JsValue::from(30u32)), ("second", JsValue::from(0u32)),
    ]);
    t.set_now(now).unwrap();
    t.create_file("/T", b"").unwrap();
    let created = get(&t.stat("/T").unwrap(), "created");
    assert_eq!(get(&created, "year").as_f64(), Some(2026.0));
    assert_eq!(code(t.set_now(JsValue::from_str("noon")).unwrap_err()), "BadArgument");
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p fs-emulator-wasm error` then `wasm-pack test --node crates/wasm`
Expected: compile errors (`code_of`, `Volume` missing).

- [ ] **Step 4: Implement**

`crates/wasm/src/error.rs`:
```rust
//! Core errors become JS `Error`s carrying a `code` property so a UI can
//! switch on the failure kind without parsing messages.

use wasm_bindgen::JsValue;

pub fn code_of(err: &fs_core::Error) -> &'static str {
    use fs_core::Error::*;
    match err {
        NotFound => "NotFound",
        AlreadyExists => "AlreadyExists",
        InvalidPath => "InvalidPath",
        InvalidName => "InvalidName",
        DiskFull => "DiskFull",
        DirectoryFull => "DirectoryFull",
        NotADirectory => "NotADirectory",
        IsADirectory => "IsADirectory",
        DirectoryNotEmpty => "DirectoryNotEmpty",
        FileTooLarge => "FileTooLarge",
        InvalidGeometry(_) => "InvalidGeometry",
        CorruptImage(_) => "CorruptImage",
        Unsupported(_) => "Unsupported",
    }
}

/// A JS `Error` whose `message` is `message` and whose `code` property is `code`.
pub fn js_error(code: &str, message: &str) -> JsValue {
    let e = js_sys::Error::new(message);
    let _ = js_sys::Reflect::set(&e, &JsValue::from_str("code"), &JsValue::from_str(code));
    e.into()
}

pub fn to_js(err: fs_core::Error) -> JsValue {
    js_error(code_of(&err), &err.to_string())
}
```

`crates/wasm/src/types.rs`:
```rust
//! TypeScript declarations for every object the `Volume` methods return or
//! accept. wasm-bindgen appends this block to the generated `.d.ts`.

use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(typescript_custom_section)]
const TYPES: &str = r#"
export interface DateTime { year: number; month: number; day: number; hour: number; minute: number; second: number; }
export interface EntryInfo { name: string; isDir: boolean; size: number; created: DateTime | null; modified: DateTime | null; accessed: DateTime | null; }
export interface Range { start: number; end: number; }
export interface ByteChange { offset: number; before: Uint8Array; after: Uint8Array; }
export interface EventRecord { kind: string; text: string; region: Range | null; }
export interface OpRecord { op: string; changes: ByteChange[]; events: EventRecord[]; }
export type RegionKind = "boot" | "metadata" | "allocationTable" | "directory" | "data" | "reserved" | "other";
export interface Region { name: string; sectors: Range; kind: RegionKind; }
export interface Annotation { range: Range; label: string; value: string; }
export interface FormatOptions { bytesPerSector?: number; sectorsPerCluster?: number; totalSectors?: number; fatCount?: number; rootEntries?: number; reservedSectors?: number; volumeLabel?: string; volumeId?: number; enforceFat16Range?: boolean; }
export interface BootSector { oemName: string; bytesPerSector: number; sectorsPerCluster: number; reservedSectors: number; fatCount: number; rootEntries: number; totalSectors: number; media: number; sectorsPerFat: number; sectorsPerTrack: number; heads: number; hiddenSectors: number; driveNumber: number; bootSignature: number; volumeId: number; volumeLabel: string; fsType: string; }
export interface Geometry { variant: "fat16"; bytesPerSector: number; sectorsPerCluster: number; reservedSectors: number; fatCount: number; sectorsPerFat: number; rootEntries: number; rootDirSectors: number; firstRootDirSector: number; firstDataSector: number; totalSectors: number; clusterCount: number; }
export type FatEntry = { kind: "free" } | { kind: "next"; cluster: number } | { kind: "endOfChain" } | { kind: "bad" } | { kind: "reserved" };
export type RawEntry =
  | { kind: "free" }
  | { kind: "deleted"; bytes: Uint8Array }
  | { kind: "short"; name: string; attr: number; firstCluster: number; size: number; created: DateTime | null; modified: DateTime | null; accessed: DateTime | null; isDir: boolean }
  | { kind: "lfn"; order: number; isLast: boolean; checksum: number; text: string };
export interface ClusterOwner { cluster: number; path: string; isDir: boolean; firstCluster: number; }
export interface FsError extends Error { code: string; }
"#;
```

`crates/wasm/src/volume.rs` (generic surface; Task 4 appends the FAT methods):
```rust
//! The one class a UI talks to. Generic operations go through the
//! `FileSystem` trait; FAT-specific inspection is gated on the inner variant.

use crate::dto;
use crate::error::{js_error, to_js};
use fat::FatFs;
use fs_core::FileSystem;
use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;

enum Inner {
    Fat(FatFs),
}

#[wasm_bindgen]
pub struct Volume {
    inner: Inner,
}

fn to_value<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    let serializer = Serializer::new().serialize_maps_as_objects(true).serialize_missing_as_null(true);
    value.serialize(&serializer).map_err(|e| js_error("BadArgument", &e.to_string()))
}

fn from_value<T: serde::de::DeserializeOwned>(value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value).map_err(|e| js_error("BadArgument", &e.to_string()))
}

impl Volume {
    fn fs(&self) -> &dyn FileSystem {
        match &self.inner {
            Inner::Fat(f) => f,
        }
    }

    fn fs_mut(&mut self) -> &mut dyn FileSystem {
        match &mut self.inner {
            Inner::Fat(f) => f,
        }
    }

    /// The FAT volume, or a `NotFat` error once other filesystems exist.
    fn fat(&self) -> Result<&FatFs, JsValue> {
        match &self.inner {
            Inner::Fat(f) => Ok(f),
        }
    }

    fn op(&mut self, result: fs_core::Result<fs_core::OpRecord>) -> Result<JsValue, JsValue> {
        let record = result.map_err(to_js)?;
        to_value(&dto::OpRecord::from(&record))
    }
}

#[wasm_bindgen]
impl Volume {
    /// Format a fresh FAT16 volume. `options` may be `undefined`.
    #[wasm_bindgen(js_name = formatFat16)]
    pub fn format_fat16(
        #[wasm_bindgen(unchecked_param_type = "FormatOptions | undefined")] options: JsValue,
    ) -> Result<Volume, JsValue> {
        let opts: dto::FormatOptions = if options.is_undefined() || options.is_null() {
            dto::FormatOptions::default()
        } else {
            from_value(options)?
        };
        let fs = FatFs::format(opts.into()).map_err(to_js)?;
        Ok(Volume { inner: Inner::Fat(fs) })
    }

    #[wasm_bindgen(js_name = fromImage)]
    pub fn from_image(bytes: &[u8]) -> Result<Volume, JsValue> {
        let fs = FatFs::from_image(bytes.to_vec()).map_err(to_js)?;
        Ok(Volume { inner: Inner::Fat(fs) })
    }

    #[wasm_bindgen(js_name = fsType)]
    pub fn fs_type(&self) -> String {
        self.fs().fs_type().to_string()
    }

    #[wasm_bindgen(js_name = setNow)]
    pub fn set_now(&mut self, #[wasm_bindgen(unchecked_param_type = "DateTime")] now: JsValue) -> Result<(), JsValue> {
        let now: dto::DateTime = from_value(now)?;
        self.fs_mut().set_now(now.into());
        Ok(())
    }

    #[wasm_bindgen(js_name = createFile, unchecked_return_type = "OpRecord")]
    pub fn create_file(&mut self, path: &str, data: &[u8]) -> Result<JsValue, JsValue> {
        let r = self.fs_mut().create_file(path, data);
        self.op(r)
    }

    #[wasm_bindgen(js_name = writeFile, unchecked_return_type = "OpRecord")]
    pub fn write_file(&mut self, path: &str, data: &[u8]) -> Result<JsValue, JsValue> {
        let r = self.fs_mut().write_file(path, data);
        self.op(r)
    }

    #[wasm_bindgen(js_name = readFile)]
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, JsValue> {
        self.fs().read_file(path).map_err(to_js)
    }

    #[wasm_bindgen(js_name = deleteFile, unchecked_return_type = "OpRecord")]
    pub fn delete_file(&mut self, path: &str) -> Result<JsValue, JsValue> {
        let r = self.fs_mut().delete_file(path);
        self.op(r)
    }

    #[wasm_bindgen(js_name = createDir, unchecked_return_type = "OpRecord")]
    pub fn create_dir(&mut self, path: &str) -> Result<JsValue, JsValue> {
        let r = self.fs_mut().create_dir(path);
        self.op(r)
    }

    #[wasm_bindgen(js_name = removeDir, unchecked_return_type = "OpRecord")]
    pub fn remove_dir(&mut self, path: &str) -> Result<JsValue, JsValue> {
        let r = self.fs_mut().remove_dir(path);
        self.op(r)
    }

    #[wasm_bindgen(js_name = listDir, unchecked_return_type = "EntryInfo[]")]
    pub fn list_dir(&self, path: &str) -> Result<JsValue, JsValue> {
        let entries: Vec<dto::EntryInfo> = self.fs().list_dir(path).map_err(to_js)?.into_iter().map(Into::into).collect();
        to_value(&entries)
    }

    #[wasm_bindgen(unchecked_return_type = "EntryInfo")]
    pub fn stat(&self, path: &str) -> Result<JsValue, JsValue> {
        to_value(&dto::EntryInfo::from(self.fs().stat(path).map_err(to_js)?))
    }

    #[wasm_bindgen(unchecked_return_type = "Region[]")]
    pub fn layout(&self) -> Result<JsValue, JsValue> {
        let regions: Vec<dto::Region> = self.fs().layout().into_iter().map(Into::into).collect();
        to_value(&regions)
    }

    #[wasm_bindgen(js_name = annotateSector, unchecked_return_type = "Annotation[]")]
    pub fn annotate_sector(&self, sector: u32) -> Result<JsValue, JsValue> {
        let list: Vec<dto::Annotation> = self.fs().annotate_sector(sector as u64).into_iter().map(Into::into).collect();
        to_value(&list)
    }

    #[wasm_bindgen(js_name = historyLength)]
    pub fn history_length(&self) -> usize {
        self.fs().history().len()
    }

    #[wasm_bindgen(js_name = historyAt, unchecked_return_type = "OpRecord")]
    pub fn history_at(&self, index: usize) -> Result<JsValue, JsValue> {
        let history = self.fs().history();
        let record = history
            .get(index)
            .ok_or_else(|| js_error("BadArgument", &format!("history index {index} is out of range (length {})", history.len())))?;
        to_value(&dto::OpRecord::from(record))
    }

    #[wasm_bindgen(js_name = sectorSize)]
    pub fn sector_size(&self) -> usize {
        self.fs().disk().sector_size()
    }

    #[wasm_bindgen(js_name = sectorCount)]
    pub fn sector_count(&self) -> u32 {
        self.fs().disk().sector_count() as u32
    }

    /// A copy of one sector's bytes.
    pub fn sector(&self, n: u32) -> Result<Vec<u8>, JsValue> {
        let disk = self.fs().disk();
        if n as u64 >= disk.sector_count() {
            return Err(js_error("BadArgument", &format!("sector {n} is out of range (count {})", disk.sector_count())));
        }
        Ok(disk.sector(n as u64).to_vec())
    }

    /// A copy of the whole disk image.
    pub fn image(&self) -> Vec<u8> {
        self.fs().disk().as_bytes().to_vec()
    }
}
```

`crates/wasm/src/lib.rs`:
```rust
//! Browser bindings for the emulator: one `Volume` class over the
//! `FileSystem` trait plus FAT-specific inspection, with plain JS objects
//! crossing the boundary.

pub mod dto;
pub mod error;
pub mod types;
pub mod volume;

pub use volume::Volume;

use wasm_bindgen::prelude::wasm_bindgen;

/// Runs once when the module loads so any panic reaches the console with a message.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p fs-emulator-wasm && cargo clippy --workspace --all-targets -- -D warnings && cargo build --workspace --target wasm32-unknown-unknown && wasm-pack test --node crates/wasm`
Expected: 11 native tests pass; the 4 wasm tests pass under node; no warnings. If `unchecked_param_type` is rejected, the installed wasm-bindgen is older than 0.2.96: pin `wasm-bindgen = "0.2.100"` in Cargo.toml and rerun.

- [ ] **Step 6: Commit**

```bash
git add crates/wasm Cargo.lock
git commit -m "feat(wasm): Volume class with generic filesystem operations and typed errors"
```

---

### Task 4: FAT inspection methods and README

**Files:**
- Modify: `crates/wasm/src/volume.rs`, `crates/wasm/tests/volume.rs`
- Create: `crates/wasm/README.md`

**Interfaces:**
- Consumes: `FatFs::{boot_sector, geometry, fat_entries, cluster_chain, raw_dir_entries, cluster_owners, annotate_sector_with}`, `dto::{BootSector, Geometry, FatEntry, RawEntry, ClusterOwner, owners_to_list, owners_from_list}`
- Produces: JS methods `bootSector`, `geometry`, `fatEntries`, `clusterChain`, `rawDirEntries`, `clusterOwners`, `annotateSectorWith`.

- [ ] **Step 1: Failing test (append to tests/volume.rs)**

```rust
#[wasm_bindgen_test]
fn fat_inspection() {
    let mut v = fresh();
    v.create_file("/A very long file name.txt", b"x").unwrap();

    let entries = Array::from(&v.fat_entries(0).unwrap());
    assert_eq!(get(&entries.get(0), "kind").as_string().unwrap(), "reserved");
    assert_eq!(get(&entries.get(2), "kind").as_string().unwrap(), "endOfChain");
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
    assert_eq!(get(&raw.get(2), "name").as_string().unwrap(), "AVERYL~1.TXT");
    assert_eq!(get(&raw.get(3), "kind").as_string().unwrap(), "free");
    assert_eq!(code(v.raw_dir_entries("/A very long file name.txt").unwrap_err()), "NotADirectory");

    let bs = v.boot_sector().unwrap();
    assert_eq!(get(&bs, "fsType").as_string().unwrap(), "FAT16");
    assert_eq!(get(&bs, "bytesPerSector").as_f64(), Some(512.0));
    let g = v.geometry().unwrap();
    assert_eq!(get(&g, "variant").as_string().unwrap(), "fat16");
    assert_eq!(get(&g, "clusterCount").as_f64(), Some(8167.0));

    let owners = v.cluster_owners().unwrap();
    let list = Array::from(&owners);
    assert_eq!(list.length(), 1);
    assert_eq!(get(&list.get(0), "path").as_string().unwrap(), "/A very long file name.txt");
    assert_eq!(get(&list.get(0), "cluster").as_f64(), Some(2.0));

    let data_sector = get(&g, "firstDataSector").as_f64().unwrap() as u32;
    let with = v.annotate_sector_with(data_sector, owners).unwrap();
    let plain = v.annotate_sector(data_sector).unwrap();
    assert_eq!(js_sys::JSON::stringify(&with).unwrap(), js_sys::JSON::stringify(&plain).unwrap());
    assert_eq!(get(&Array::from(&plain).get(0), "value").as_string().unwrap(), "data of /A very long file name.txt");
    assert_eq!(code(v.annotate_sector_with(0, JsValue::from_str("nope")).unwrap_err()), "BadArgument");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `wasm-pack test --node crates/wasm`
Expected: compile error (`fat_entries` missing).

- [ ] **Step 3: Implement (append a second `#[wasm_bindgen] impl Volume` block to volume.rs)**

```rust
/// FAT-specific inspection. Each throws `code === "NotFat"` on a non-FAT volume.
#[wasm_bindgen]
impl Volume {
    #[wasm_bindgen(js_name = bootSector, unchecked_return_type = "BootSector")]
    pub fn boot_sector(&self) -> Result<JsValue, JsValue> {
        to_value(&dto::BootSector::from(self.fat()?.boot_sector()))
    }

    #[wasm_bindgen(unchecked_return_type = "Geometry")]
    pub fn geometry(&self) -> Result<JsValue, JsValue> {
        to_value(&dto::Geometry::from(self.fat()?.geometry()))
    }

    /// Every entry of one FAT copy, indexed by cluster; empty for a copy that does not exist.
    #[wasm_bindgen(js_name = fatEntries, unchecked_return_type = "FatEntry[]")]
    pub fn fat_entries(&self, fat: u8) -> Result<JsValue, JsValue> {
        let list: Vec<dto::FatEntry> = self.fat()?.fat_entries(fat).into_iter().map(Into::into).collect();
        to_value(&list)
    }

    #[wasm_bindgen(js_name = clusterChain, unchecked_return_type = "number[]")]
    pub fn cluster_chain(&self, start: u32) -> Result<JsValue, JsValue> {
        let chain = self.fat()?.cluster_chain(start).map_err(to_js)?;
        to_value(&chain)
    }

    #[wasm_bindgen(js_name = rawDirEntries, unchecked_return_type = "RawEntry[]")]
    pub fn raw_dir_entries(&self, path: &str) -> Result<JsValue, JsValue> {
        let list: Vec<dto::RawEntry> = self.fat()?.raw_dir_entries(path).map_err(to_js)?.into_iter().map(Into::into).collect();
        to_value(&list)
    }

    /// One full directory-tree walk; reuse the result with `annotateSectorWith`.
    #[wasm_bindgen(js_name = clusterOwners, unchecked_return_type = "ClusterOwner[]")]
    pub fn cluster_owners(&self) -> Result<JsValue, JsValue> {
        to_value(&dto::owners_to_list(&self.fat()?.cluster_owners()))
    }

    #[wasm_bindgen(js_name = annotateSectorWith, unchecked_return_type = "Annotation[]")]
    pub fn annotate_sector_with(
        &self,
        sector: u32,
        #[wasm_bindgen(unchecked_param_type = "ClusterOwner[]")] owners: JsValue,
    ) -> Result<JsValue, JsValue> {
        let owners: Vec<dto::ClusterOwner> = from_value(owners)?;
        let map = dto::owners_from_list(owners);
        let list: Vec<dto::Annotation> =
            self.fat()?.annotate_sector_with(sector as u64, &map).into_iter().map(Into::into).collect();
        to_value(&list)
    }
}
```

`crates/wasm/README.md`:
````markdown
# fs-emulator-wasm

Browser bindings for the emulator. One class, `Volume`, exposes the
filesystem-agnostic operations plus FAT-specific inspection; results are plain
JS objects with TypeScript types in the generated `.d.ts`.

```ts
import { Volume } from "fs-emulator-wasm";

const vol = Volume.formatFat16(undefined);
const record = vol.createFile("/Hello world.txt", new TextEncoder().encode("hi"));
for (const e of record.events) console.log(e.kind, e.text);
console.log(vol.listDir("/"));
```

Errors are `Error` objects with a `code` property (`NotFound`, `DiskFull`,
`CorruptImage`, ..., plus `NotFat` and `BadArgument`).

## Build and test

```
wasm-pack build crates/wasm --target bundler   # writes crates/wasm/pkg
wasm-pack test --node crates/wasm              # boundary tests
cargo test -p fs-emulator-wasm                 # native DTO tests
```

## Demo

```
cd web/demo && pnpm install && pnpm dev
```
````

- [ ] **Step 4: Run tests**

Run: `cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && wasm-pack test --node crates/wasm`
Expected: all pass (5 wasm tests).

- [ ] **Step 5: Commit**

```bash
git add crates/wasm
git commit -m "feat(wasm): FAT inspection methods and README"
```

---

### Task 5: Demo page and CI job

**Files:**
- Create: `web/demo/package.json`, `web/demo/vite.config.ts`, `web/demo/tsconfig.json`, `web/demo/index.html`, `web/demo/src/main.ts`, `web/demo/pnpm-lock.yaml` (generated)
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: the `fs-emulator-wasm` package from `crates/wasm/pkg` built by `wasm-pack build crates/wasm --target bundler`.

- [ ] **Step 1: Build the package and scaffold the demo**

Run `wasm-pack build crates/wasm --target bundler` first so `crates/wasm/pkg` exists.

`web/demo/package.json`:
```json
{
  "name": "fs-emulator-demo",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc --noEmit && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "fs-emulator-wasm": "file:../../crates/wasm/pkg"
  },
  "devDependencies": {
    "typescript": "^5.6.0",
    "vite": "^6.0.0",
    "vite-plugin-top-level-await": "^1.4.4",
    "vite-plugin-wasm": "^3.4.1"
  }
}
```

`web/demo/vite.config.ts`:
```ts
import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

export default defineConfig({
  plugins: [wasm(), topLevelAwait()],
  build: { target: "esnext" },
  optimizeDeps: { exclude: ["fs-emulator-wasm"] },
});
```

`web/demo/tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "lib": ["ES2022", "DOM"],
    "skipLibCheck": true,
    "types": ["vite/client"]
  },
  "include": ["src", "vite.config.ts"]
}
```

`web/demo/index.html`:
```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>fs-emulator demo</title>
    <style>
      body { font-family: system-ui, sans-serif; margin: 1rem; display: grid; gap: 1rem; grid-template-columns: 1fr 1fr; }
      section { border: 1px solid #ccc; border-radius: 6px; padding: 0.75rem; }
      h2 { margin-top: 0; font-size: 1rem; }
      table { border-collapse: collapse; width: 100%; }
      td, th { text-align: left; padding: 2px 6px; border-bottom: 1px solid #eee; font-size: 0.9rem; }
      pre { font-family: ui-monospace, monospace; font-size: 0.8rem; line-height: 1.4; }
      .hex span { padding: 0 1px; }
      .hex span.hl { background: #ffe58a; }
      #status { grid-column: 1 / -1; color: #a00; min-height: 1.2em; }
      textarea { width: 100%; }
      ul { padding-left: 1.2rem; }
    </style>
  </head>
  <body>
    <div id="status"></div>
    <section>
      <h2>Actions</h2>
      <label>Path <input id="path" value="/Hello world.txt" size="30" /></label>
      <textarea id="content" rows="4">Hello from the browser</textarea>
      <div>
        <button id="add">Add file</button>
        <button id="overwrite">Overwrite</button>
        <button id="delete">Delete</button>
        <button id="mkdir">Mkdir</button>
      </div>
      <div style="margin-top: 0.5rem">
        <label>Load image <input id="load" type="file" /></label>
        <a id="export" href="#" download="volume.img">Export image</a>
      </div>
    </section>
    <section>
      <h2>Root listing</h2>
      <div id="listing"></div>
    </section>
    <section>
      <h2>Last operation</h2>
      <div id="lastop"></div>
    </section>
    <section>
      <h2>Sector view</h2>
      <label>Sector <input id="sector" type="number" value="0" min="0" /></label>
      <span id="region"></span>
      <pre id="hex" class="hex"></pre>
      <ul id="annotations"></ul>
    </section>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

`web/demo/src/main.ts`:
```ts
import { Volume } from "fs-emulator-wasm";
import type { Annotation, ClusterOwner, DateTime, OpRecord } from "fs-emulator-wasm";

const $ = <T extends HTMLElement>(id: string): T => document.getElementById(id) as T;

let volume = Volume.formatFat16(undefined);
let owners: ClusterOwner[] = volume.clusterOwners();

function status(text: string): void {
  $("status").textContent = text;
}

function fail(e: unknown): void {
  const err = e as { code?: string; message?: string };
  status(`${err.code ?? "Error"}: ${err.message ?? String(e)}`);
}

function esc(s: string): string {
  return s.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c] ?? c);
}

function fmtTime(t: DateTime | null): string {
  if (!t) return "";
  const p = (n: number) => String(n).padStart(2, "0");
  return `${t.year}-${p(t.month)}-${p(t.day)} ${p(t.hour)}:${p(t.minute)}:${p(t.second)}`;
}

function renderListing(): void {
  const rows = volume
    .listDir("/")
    .map((e) => `<tr><td>${esc(e.name)}</td><td>${e.isDir ? "dir" : "file"}</td><td>${e.size}</td><td>${fmtTime(e.modified)}</td></tr>`)
    .join("");
  $("listing").innerHTML = `<table><tr><th>Name</th><th>Type</th><th>Size</th><th>Modified</th></tr>${rows}</table>`;
}

function changedSectors(rec: OpRecord): number[] {
  const size = volume.sectorSize();
  const sectors = new Set<number>();
  for (const c of rec.changes) {
    const first = Math.floor(c.offset / size);
    const last = Math.floor((c.offset + Math.max(c.after.length, 1) - 1) / size);
    for (let s = first; s <= last; s++) sectors.add(s);
  }
  return [...sectors].sort((a, b) => a - b);
}

function renderLastOp(): void {
  const n = volume.historyLength();
  if (n === 0) {
    $("lastop").textContent = "no operations yet";
    return;
  }
  const rec = volume.historyAt(n - 1);
  const items = rec.events.map((e) => `<li><code>${esc(e.kind)}</code> ${esc(e.text)}</li>`).join("");
  $("lastop").innerHTML = `<h3>${esc(rec.op)}</h3><p>sectors changed: ${changedSectors(rec).join(", ")}</p><ul>${items}</ul>`;
}

function renderSector(): void {
  const n = Number($<HTMLInputElement>("sector").value) || 0;
  const region = volume.layout().find((r) => n >= r.sectors.start && n < r.sectors.end);
  $("region").textContent = region ? `${region.name} (${region.kind})` : "out of range";
  let bytes: Uint8Array;
  let annotations: Annotation[];
  try {
    bytes = volume.sector(n);
    annotations = volume.annotateSectorWith(n, owners);
  } catch (e) {
    fail(e);
    return;
  }
  const lines: string[] = [];
  for (let row = 0; row < bytes.length; row += 16) {
    const cells: string[] = [];
    for (let i = row; i < row + 16 && i < bytes.length; i++) {
      cells.push(`<span data-i="${i}">${bytes[i].toString(16).padStart(2, "0")}</span>`);
    }
    lines.push(`${row.toString(16).padStart(4, "0")}  ${cells.join(" ")}`);
  }
  $("hex").innerHTML = lines.join("\n");
  $("annotations").innerHTML = annotations
    .map((a, i) => `<li data-a="${i}"><b>${a.range.start}..${a.range.end}</b> ${esc(a.label)}: ${esc(a.value)}</li>`)
    .join("");
  const spans = $("hex").querySelectorAll<HTMLSpanElement>("span");
  $("annotations").querySelectorAll<HTMLLIElement>("li").forEach((li, i) => {
    const { start, end } = annotations[i].range;
    li.addEventListener("mouseenter", () => spans.forEach((s) => s.classList.toggle("hl", Number(s.dataset.i) >= start && Number(s.dataset.i) < end)));
    li.addEventListener("mouseleave", () => spans.forEach((s) => s.classList.remove("hl")));
  });
}

function refresh(): void {
  owners = volume.clusterOwners();
  renderListing();
  renderLastOp();
  renderSector();
  const blob = new Blob([volume.image()], { type: "application/octet-stream" });
  $<HTMLAnchorElement>("export").href = URL.createObjectURL(blob);
}

function mutate(action: () => void): void {
  try {
    action();
    status("");
  } catch (e) {
    fail(e);
  }
  refresh();
}

const path = () => $<HTMLInputElement>("path").value;
const content = () => new TextEncoder().encode($<HTMLTextAreaElement>("content").value);

$("add").addEventListener("click", () => mutate(() => volume.createFile(path(), content())));
$("overwrite").addEventListener("click", () => mutate(() => volume.writeFile(path(), content())));
$("delete").addEventListener("click", () => mutate(() => volume.deleteFile(path())));
$("mkdir").addEventListener("click", () => mutate(() => volume.createDir(path())));
$("sector").addEventListener("input", renderSector);
$<HTMLInputElement>("load").addEventListener("change", async (ev) => {
  const file = (ev.target as HTMLInputElement).files?.[0];
  if (!file) return;
  const bytes = new Uint8Array(await file.arrayBuffer());
  mutate(() => {
    volume = Volume.fromImage(bytes);
  });
});

refresh();
```

- [ ] **Step 2: Install and build the demo**

Run:
```bash
cd web/demo && pnpm install && pnpm build
```
Expected: `tsc --noEmit` passes against the generated types and Vite emits `dist/`. If `vite-plugin-wasm` rejects the installed Vite major, pin `vite` to the latest major it supports (its README lists them) and rerun. Commit the generated `pnpm-lock.yaml`.

- [ ] **Step 3: Manual check in a browser**

Run `pnpm dev` in `web/demo` and open the printed URL. Verify: the root listing is empty; Add file creates `Hello world.txt` and the Last operation pane lists `cluster_allocated`, `data_written`, and `dir_entry_written` events with changed sectors; sector 0 shows boot-sector annotations and hovering one highlights bytes; typing the first data sector number shows `data of /Hello world.txt`; Delete removes the file and the listing updates; Export downloads a 16 MiB `volume.img`; Load of that file restores the empty state. Note the outcome in the commit body if anything deviates.

- [ ] **Step 4: Add the CI job**

Append to `.github/workflows/ci.yml` under `jobs:`:
```yaml
  wasm:
    name: wasm-pack test, build, demo
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - uses: Swatinem/rust-cache@v2
      - uses: taiki-e/install-action@v2
        with:
          tool: wasm-pack
      - uses: pnpm/action-setup@v4
        with:
          version: 10
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm
          cache-dependency-path: web/demo/pnpm-lock.yaml
      - run: wasm-pack test --node crates/wasm
      - run: wasm-pack build crates/wasm --target bundler
      - run: pnpm install --frozen-lockfile
        working-directory: web/demo
      - run: pnpm build
        working-directory: web/demo
```

- [ ] **Step 5: Final check and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && wasm-pack test --node crates/wasm && (cd web/demo && pnpm build)`
Expected: all clean.

```bash
git add web/demo/package.json web/demo/pnpm-lock.yaml web/demo/vite.config.ts web/demo/tsconfig.json web/demo/index.html web/demo/src .github/workflows/ci.yml
git commit -m "feat(demo): Vite demo page and wasm CI job"
```
