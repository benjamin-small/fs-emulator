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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
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
}
