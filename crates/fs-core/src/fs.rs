//! The interface a UI programs against. Constructors (`format`, `from_image`)
//! live on the concrete filesystem types because their options differ.

use crate::layout::{Annotation, Region};
use crate::{DateTime, Disk, EntryInfo, OpRecord, Result};

pub trait FileSystem {
    /// `"FAT16"`, later `"FAT32"`, `"ext2"`.
    fn fs_type(&self) -> &'static str;

    /// `AlreadyExists` if the path is present.
    fn create_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord>;
    /// Overwrite an existing file; `NotFound` if absent.
    fn write_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord>;
    fn read_file(&self, path: &str) -> Result<Vec<u8>>;
    fn delete_file(&mut self, path: &str) -> Result<OpRecord>;
    fn create_dir(&mut self, path: &str) -> Result<OpRecord>;
    /// `DirectoryNotEmpty` unless empty.
    fn remove_dir(&mut self, path: &str) -> Result<OpRecord>;
    /// Excludes `.` and `..`.
    fn list_dir(&self, path: &str) -> Result<Vec<EntryInfo>>;
    fn stat(&self, path: &str) -> Result<EntryInfo>;
    /// The timestamp subsequent operations stamp onto entries.
    fn set_now(&mut self, now: DateTime);

    fn disk(&self) -> &Disk;
    fn layout(&self) -> Vec<Region>;
    fn annotate_sector(&self, sector: u64) -> Vec<Annotation>;
    fn history(&self) -> &[OpRecord];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DateTime, Disk, EntryInfo, Error, OpRecord, Result};

    struct NullFs {
        disk: Disk,
    }

    impl FileSystem for NullFs {
        fn fs_type(&self) -> &'static str {
            "null"
        }
        fn create_file(&mut self, _: &str, _: &[u8]) -> Result<OpRecord> {
            Err(Error::Unsupported("null".into()))
        }
        fn write_file(&mut self, _: &str, _: &[u8]) -> Result<OpRecord> {
            Err(Error::Unsupported("null".into()))
        }
        fn read_file(&self, _: &str) -> Result<Vec<u8>> {
            Err(Error::NotFound)
        }
        fn delete_file(&mut self, _: &str) -> Result<OpRecord> {
            Err(Error::NotFound)
        }
        fn create_dir(&mut self, _: &str) -> Result<OpRecord> {
            Err(Error::Unsupported("null".into()))
        }
        fn remove_dir(&mut self, _: &str) -> Result<OpRecord> {
            Err(Error::NotFound)
        }
        fn list_dir(&self, _: &str) -> Result<Vec<EntryInfo>> {
            Ok(Vec::new())
        }
        fn stat(&self, _: &str) -> Result<EntryInfo> {
            Err(Error::NotFound)
        }
        fn set_now(&mut self, _: DateTime) {}
        fn disk(&self) -> &Disk {
            &self.disk
        }
        fn layout(&self) -> Vec<Region> {
            use crate::layout::RegionKind;
            vec![Region {
                name: "all".into(),
                sectors: 0..1,
                kind: RegionKind::Other,
            }]
        }
        fn annotate_sector(&self, _: u64) -> Vec<Annotation> {
            Vec::new()
        }
        fn history(&self) -> &[OpRecord] {
            &[]
        }
    }

    #[test]
    fn trait_is_object_safe() {
        let fs: Box<dyn FileSystem> = Box::new(NullFs {
            disk: Disk::new(512, 1),
        });
        assert_eq!(fs.fs_type(), "null");
        assert_eq!(fs.layout()[0].kind, crate::layout::RegionKind::Other);
        assert_eq!(fs.list_dir("/").unwrap(), Vec::new());
    }
}
