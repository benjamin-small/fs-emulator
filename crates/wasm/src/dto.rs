//! Plain-data mirrors of the core types, shaped for JavaScript: camelCase
//! fields, tagged unions on `kind`, `Option` as `null`, bytes as `Uint8Array`.
//! Nothing here touches wasm-bindgen, so it all runs under `cargo test`.

use fat::FatVariant;
use fs_core::{Event, RegionKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
        DateTime {
            year: d.year,
            month: d.month,
            day: d.day,
            hour: d.hour,
            minute: d.minute,
            second: d.second,
        }
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
        Range {
            start: r.start,
            end: r.end,
        }
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
        ByteChange {
            offset: c.offset,
            before: c.before.clone(),
            after: c.after.clone(),
        }
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
        EventRecord {
            kind: e.kind().to_string(),
            text: e.to_string(),
            region: e.region().map(Into::into),
        }
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
            events: r
                .events
                .iter()
                .map(|e| EventRecord::from_event(e.as_ref()))
                .collect(),
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
            sectors: Range64 {
                start: r.sectors.start,
                end: r.sectors.end,
            },
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
        Annotation {
            range: a.range.into(),
            label: a.label,
            value: a.value,
        }
    }
}

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
            volume_label: o
                .volume_label
                .map(|l| pad_label(&l))
                .unwrap_or(d.volume_label),
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
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
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
            fat::RawEntry::Deleted { bytes } => RawEntry::Deleted {
                bytes: bytes.to_vec(),
            },
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
        .map(|(&cluster, o)| ClusterOwner {
            cluster,
            path: o.path.clone(),
            is_dir: o.is_dir,
            first_cluster: o.first_cluster,
        })
        .collect()
}

pub fn owners_from_list(list: Vec<ClusterOwner>) -> BTreeMap<u32, fat::ClusterOwner> {
    list.into_iter()
        .map(|o| {
            (
                o.cluster,
                fat::ClusterOwner {
                    path: o.path,
                    is_dir: o.is_dir,
                    first_cluster: o.first_cluster,
                },
            )
        })
        .collect()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use fat::{FatFs, FormatOptions as FatFormat};
    use std::collections::BTreeMap;
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
        assert_eq!(
            dto,
            DateTime {
                year: 2026,
                month: 9,
                day: 21,
                hour: 1,
                minute: 2,
                second: 3
            }
        );
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
        assert_eq!(
            dto.created,
            Some(DateTime::from(fs_core::DateTime::default()))
        );
        assert_eq!(dto.modified, None);
    }

    #[test]
    fn op_record_maps_changes_and_events() {
        let mut core = fs_core::OpRecord::new("create_file /A");
        core.changes.push(fs_core::ByteChange {
            offset: 5,
            before: vec![0, 0],
            after: vec![1, 2],
        });
        core.events.push(Box::new(Dummy));
        let dto = OpRecord::from(&core);
        assert_eq!(dto.op, "create_file /A");
        assert_eq!(
            dto.changes,
            vec![ByteChange {
                offset: 5,
                before: vec![0, 0],
                after: vec![1, 2]
            }]
        );
        assert_eq!(
            dto.events,
            vec![EventRecord {
                kind: "dummy".into(),
                text: "did a thing".into(),
                region: Some(Range { start: 10, end: 12 })
            }]
        );
    }

    #[test]
    fn region_and_annotation_map() {
        let r = Region::from(fs_core::Region {
            name: "FAT 0".into(),
            sectors: 1..33,
            kind: RegionKind::AllocationTable,
        });
        assert_eq!(
            r,
            Region {
                name: "FAT 0".into(),
                sectors: Range64 { start: 1, end: 33 },
                kind: "allocationTable".into()
            }
        );
        assert_eq!(region_kind_name(RegionKind::Boot), "boot");
        assert_eq!(region_kind_name(RegionKind::Other), "other");
        let a = Annotation::from(fs_core::Annotation {
            range: 11..13,
            label: "bytes per sector".into(),
            value: "512".into(),
        });
        assert_eq!(a.range, Range { start: 11, end: 13 });
        assert_eq!(a.label, "bytes per sector");
    }

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
        impl SerializeStruct for &mut KeyCollector {
            type Ok = ();
            type Error = KeyErr;
            fn serialize_field<T: ?Sized + Serialize>(
                &mut self,
                key: &'static str,
                _: &T,
            ) -> Result<(), KeyErr> {
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
        impl Serializer for &mut KeyCollector {
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
            fn serialize_none(self) -> Result<(), KeyErr> {
                Err(KeyErr)
            }
            fn serialize_some<T: ?Sized + Serialize>(self, _: &T) -> Result<(), KeyErr> {
                Err(KeyErr)
            }
            fn serialize_unit(self) -> Result<(), KeyErr> {
                Err(KeyErr)
            }
            fn serialize_unit_struct(self, _: &'static str) -> Result<(), KeyErr> {
                Err(KeyErr)
            }
            fn serialize_unit_variant(
                self,
                _: &'static str,
                _: u32,
                _: &'static str,
            ) -> Result<(), KeyErr> {
                Err(KeyErr)
            }
            fn serialize_newtype_struct<T: ?Sized + Serialize>(
                self,
                _: &'static str,
                _: &T,
            ) -> Result<(), KeyErr> {
                Err(KeyErr)
            }
            fn serialize_newtype_variant<T: ?Sized + Serialize>(
                self,
                _: &'static str,
                _: u32,
                _: &'static str,
                _: &T,
            ) -> Result<(), KeyErr> {
                Err(KeyErr)
            }
            fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, KeyErr> {
                Err(KeyErr)
            }
            fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, KeyErr> {
                Err(KeyErr)
            }
            fn serialize_tuple_struct(
                self,
                _: &'static str,
                _: usize,
            ) -> Result<Self::SerializeTupleStruct, KeyErr> {
                Err(KeyErr)
            }
            fn serialize_tuple_variant(
                self,
                _: &'static str,
                _: u32,
                _: &'static str,
                _: usize,
            ) -> Result<Self::SerializeTupleVariant, KeyErr> {
                Err(KeyErr)
            }
            fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, KeyErr> {
                Err(KeyErr)
            }
            fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, KeyErr> {
                Ok(self)
            }
            fn serialize_struct_variant(
                self,
                _: &'static str,
                _: u32,
                _: &'static str,
                _: usize,
            ) -> Result<Self::SerializeStructVariant, KeyErr> {
                Err(KeyErr)
            }
        }
        let dto = EntryInfo {
            name: "x".into(),
            is_dir: true,
            size: 0,
            created: None,
            modified: None,
            accessed: None,
        };
        let mut keys = KeyCollector(Vec::new());
        dto.serialize(&mut keys).unwrap();
        assert_eq!(
            keys.0,
            vec!["name", "isDir", "size", "created", "modified", "accessed"]
        );
    }

    #[test]
    fn format_options_default_to_the_core_defaults() {
        let dto = FormatOptions::default();
        assert_eq!(FatFormat::from(dto), FatFormat::default());
        let dto = FormatOptions {
            total_sectors: Some(8192),
            sectors_per_cluster: Some(1),
            volume_label: Some("teach".into()),
            enforce_fat16_range: Some(false),
            ..Default::default()
        };
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
        assert_eq!(
            FatEntry::from(fat::FatEntry::Next(9)),
            FatEntry::Next { cluster: 9 }
        );
        assert_eq!(
            FatEntry::from(fat::FatEntry::EndOfChain),
            FatEntry::EndOfChain
        );
        let mut fs = FatFs::format(FatFormat::default()).unwrap();
        fs.create_file("/My File.txt", b"abc").unwrap();
        fs.create_file("/B", b"").unwrap();
        fs.delete_file("/B").unwrap();
        let raw: Vec<RawEntry> = fs
            .raw_dir_entries("/")
            .unwrap()
            .into_iter()
            .map(Into::into)
            .collect();
        assert!(
            matches!(&raw[0], RawEntry::Lfn { order: 1, is_last: true, text, .. } if text == "My File.txt")
        );
        assert!(
            matches!(&raw[1], RawEntry::Short { name, size: 3, is_dir: false, first_cluster: 2, .. } if name == "MYFILE~1.TXT")
        );
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
        assert_eq!(
            list[0],
            ClusterOwner {
                cluster: 2,
                path: "/D".into(),
                is_dir: true,
                first_cluster: 2
            }
        );
        assert_eq!(list[1].path, "/D/F");
        assert_eq!(list[1].first_cluster, 3);
        let back: BTreeMap<u32, fat::ClusterOwner> = owners_from_list(list);
        assert_eq!(back, map);
    }
}
