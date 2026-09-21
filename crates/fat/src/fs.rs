//! `FatFs`: a FAT16 volume on an in-memory disk. The disk bytes are the only
//! state; every method re-reads what it needs from them.

use crate::boot_sector::{BootSector, FormatOptions, Geometry};
use crate::dir::{self, DirLocation};
use crate::dir_entry::{self, attr, LfnEntry, ShortEntry, DELETED, ENTRY_SIZE, FREE};
use crate::name;
use crate::table;
use fs_core::path;
use fs_core::{DateTime, Disk, EntryInfo, Error, OpRecord, Region, RegionKind, Result};

pub struct FatFs {
    disk: Disk,
    boot: BootSector,
    geo: Geometry,
    now: DateTime,
    history: Vec<OpRecord>,
}

impl FatFs {
    /// A freshly formatted, empty volume.
    pub fn format(opts: FormatOptions) -> Result<FatFs> {
        let boot = BootSector::from_options(&opts)?;
        let geo = boot.geometry()?;
        let mut disk = Disk::new(geo.bytes_per_sector, geo.total_sectors);
        disk.write(0, &boot.to_bytes());
        for fat in 0..geo.fat_count {
            // Entry 0 holds the media descriptor (0xFFF8); entry 1 is reserved (0xFFFF).
            disk.write(geo.fat_offset(fat), &[0xF8, 0xFF, 0xFF, 0xFF]);
        }
        Ok(FatFs {
            disk,
            boot,
            geo,
            now: DateTime::default(),
            history: Vec::new(),
        })
    }

    /// Wrap an existing FAT16 image. Extra trailing bytes are dropped; a
    /// shorter image than the boot sector describes is rejected.
    pub fn from_image(mut bytes: Vec<u8>) -> Result<FatFs> {
        if bytes.len() < 512 {
            return Err(Error::CorruptImage(
                "image is shorter than one sector".into(),
            ));
        }
        let boot = BootSector::parse(&bytes[..512])?;
        let geo = boot.geometry()?;
        let expected = geo.total_sectors as usize * geo.bytes_per_sector;
        if bytes.len() < expected {
            return Err(Error::CorruptImage(format!(
                "image is {} bytes but the boot sector describes {expected}",
                bytes.len()
            )));
        }
        bytes.truncate(expected);
        let disk = Disk::from_bytes(geo.bytes_per_sector, bytes)?;
        Ok(FatFs {
            disk,
            boot,
            geo,
            now: DateTime::default(),
            history: Vec::new(),
        })
    }

    pub fn boot_sector(&self) -> &BootSector {
        &self.boot
    }

    pub fn geometry(&self) -> &Geometry {
        &self.geo
    }

    pub fn fs_type(&self) -> &'static str {
        "FAT16"
    }

    pub fn disk(&self) -> &Disk {
        &self.disk
    }

    pub fn set_now(&mut self, now: DateTime) {
        self.now = now;
    }

    pub fn now(&self) -> DateTime {
        self.now
    }

    pub fn history(&self) -> &[OpRecord] {
        &self.history
    }

    pub fn layout(&self) -> Vec<Region> {
        let g = &self.geo;
        let mut regions = vec![Region {
            name: "reserved (boot sector)".into(),
            sectors: 0..g.reserved_sectors,
            kind: RegionKind::Boot,
        }];
        for fat in 0..g.fat_count {
            let start = g.reserved_sectors + fat as u64 * g.sectors_per_fat;
            regions.push(Region {
                name: format!("FAT {fat}"),
                sectors: start..start + g.sectors_per_fat,
                kind: RegionKind::AllocationTable,
            });
        }
        regions.push(Region {
            name: "root directory".into(),
            sectors: g.first_root_dir_sector..g.first_data_sector,
            kind: RegionKind::Directory,
        });
        regions.push(Region {
            name: "data".into(),
            sectors: g.first_data_sector..g.total_sectors,
            kind: RegionKind::Data,
        });
        regions
    }

    /// Run `body` inside a recorded operation. On success the record is
    /// appended to history and returned; on failure it is discarded.
    #[allow(dead_code)]
    fn run_op(
        &mut self,
        op: String,
        body: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<OpRecord> {
        self.disk.begin_op(op);
        let result = body(self);
        let record = self.disk.end_op();
        result?;
        self.history.push(record.clone());
        Ok(record)
    }

    /// `/` for the root, otherwise `/A/B` from components.
    #[allow(dead_code)]
    fn dir_display_name(parts: &[String]) -> String {
        if parts.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", parts.join("/"))
        }
    }

    /// All in-use entries of a directory in slot order, with long names
    /// attached. Includes `.` and `..`; excludes the volume label.
    fn scan_dir(&self, loc: DirLocation) -> Result<Vec<Located>> {
        let mut out = Vec::new();
        let mut pending: Vec<(usize, LfnEntry)> = Vec::new();
        for slot in dir::slots(&self.disk, &self.geo, loc)? {
            let bytes = self.disk.read(slot.offset, ENTRY_SIZE);
            match bytes[0] {
                FREE => break,
                DELETED => {
                    pending.clear();
                    continue;
                }
                _ => {}
            }
            if attr::is_lfn(bytes[11]) {
                let lfn = LfnEntry::parse(bytes);
                if lfn.is_last() {
                    pending.clear();
                    pending.push((slot.index, lfn));
                } else if let Some((_, prev)) = pending.last() {
                    if prev.order() == lfn.order() + 1 && prev.checksum == lfn.checksum {
                        pending.push((slot.index, lfn));
                    } else {
                        pending.clear();
                    }
                }
                continue;
            }
            let entry = ShortEntry::parse(bytes);
            if entry.is_volume_label() {
                pending.clear();
                continue;
            }
            let long_name = assemble_lfn(&pending, &entry.name);
            let lfn_slots = if long_name.is_some() {
                pending.iter().map(|(i, _)| *i).collect()
            } else {
                Vec::new()
            };
            pending.clear();
            out.push(Located {
                dir: loc,
                slot: slot.index,
                lfn_slots,
                entry,
                long_name,
            });
        }
        Ok(out)
    }

    /// Case-insensitive lookup against both the long and the short name.
    fn find_in_dir(&self, loc: DirLocation, name: &str) -> Result<Option<Located>> {
        Ok(self.scan_dir(loc)?.into_iter().find(|l| {
            !l.entry.is_dot_entry()
                && (names_match(&l.entry.display_name(), name)
                    || l.long_name
                        .as_deref()
                        .is_some_and(|ln| names_match(ln, name)))
        }))
    }

    /// Walk directory components from the root.
    fn resolve_dir(&self, parts: &[String]) -> Result<DirLocation> {
        let mut loc = DirLocation::Root;
        for part in parts {
            let found = self.find_in_dir(loc, part)?.ok_or(Error::NotFound)?;
            if !found.entry.is_dir() {
                return Err(Error::NotADirectory);
            }
            loc = DirLocation::from_cluster(found.entry.first_cluster());
        }
        Ok(loc)
    }

    /// `Ok(None)` for the root, `Err(NotFound)` if the path is absent.
    fn resolve(&self, path: &str) -> Result<Option<Located>> {
        let parts = path::parse(path)?;
        let Some((name, parent)) = parts.split_last() else {
            return Ok(None);
        };
        let loc = self.resolve_dir(parent)?;
        self.find_in_dir(loc, name)?
            .ok_or(Error::NotFound)
            .map(Some)
    }

    fn read_chain_data(&self, first: u32, size: usize) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(size);
        if first == 0 || size == 0 {
            return Ok(out);
        }
        for cluster in table::chain(&self.disk, &self.geo, first)? {
            if out.len() >= size {
                break;
            }
            let take = (size - out.len()).min(self.geo.cluster_size());
            out.extend_from_slice(self.disk.read(self.geo.cluster_offset(cluster), take));
        }
        if out.len() < size {
            return Err(Error::CorruptImage(format!(
                "file claims {size} bytes but its cluster chain holds only {}",
                out.len()
            )));
        }
        Ok(out)
    }

    pub fn list_dir(&self, path: &str) -> Result<Vec<EntryInfo>> {
        let parts = path::parse(path)?;
        let loc = self.resolve_dir(&parts)?;
        Ok(self
            .scan_dir(loc)?
            .into_iter()
            .filter(|l| !l.entry.is_dot_entry())
            .map(|l| l.info())
            .collect())
    }

    pub fn stat(&self, path: &str) -> Result<EntryInfo> {
        match self.resolve(path)? {
            Some(located) => Ok(located.info()),
            None => Ok(EntryInfo {
                name: "/".into(),
                is_dir: true,
                size: 0,
                created: None,
                modified: None,
                accessed: None,
            }),
        }
    }

    pub fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        let located = self.resolve(path)?.ok_or(Error::IsADirectory)?;
        if located.entry.is_dir() {
            return Err(Error::IsADirectory);
        }
        self.read_chain_data(located.entry.first_cluster(), located.entry.size as usize)
    }
}

/// A directory entry found on disk, with where it lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub dir: DirLocation,
    /// Slot index of the short entry.
    pub slot: usize,
    /// Slot indices of the long-name entries that precede it, in disk order.
    pub lfn_slots: Vec<usize>,
    pub entry: ShortEntry,
    pub long_name: Option<String>,
}

impl Located {
    pub fn name(&self) -> String {
        self.long_name
            .clone()
            .unwrap_or_else(|| self.entry.display_name())
    }

    pub fn info(&self) -> EntryInfo {
        EntryInfo {
            name: self.name(),
            is_dir: self.entry.is_dir(),
            size: self.entry.size as u64,
            created: dir_entry::unpack(self.entry.create_date, self.entry.create_time),
            modified: dir_entry::unpack(self.entry.write_date, self.entry.write_time),
            accessed: dir_entry::unpack(self.entry.access_date, 0),
        }
    }
}

/// Reassemble a long name from the LFN entries collected before a short
/// entry, or `None` if they are incomplete or do not match its checksum.
fn assemble_lfn(pending: &[(usize, LfnEntry)], short_name: &[u8; 11]) -> Option<String> {
    let (_, last) = pending.last()?;
    let (_, first) = &pending[0];
    if last.order() != 1 || !first.is_last() || first.order() as usize != pending.len() {
        return None;
    }
    let sum = dir_entry::lfn_checksum(short_name);
    if pending.iter().any(|(_, e)| e.checksum != sum) {
        return None;
    }
    let mut units = Vec::with_capacity(pending.len() * 13);
    for (_, e) in pending.iter().rev() {
        units.extend(e.chars());
    }
    Some(name::from_ucs2(&units))
}

fn names_match(a: &str, b: &str) -> bool {
    a.to_uppercase() == b.to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table;
    use fs_core::{Error, RegionKind};

    #[test]
    fn format_writes_boot_sector_and_fat_headers() {
        let fs = FatFs::format(FormatOptions::default()).unwrap();
        assert_eq!(fs.fs_type(), "FAT16");
        assert_eq!(fs.disk().len(), 32768 * 512);
        assert_eq!(
            BootSector::parse(fs.disk().sector(0)).unwrap(),
            *fs.boot_sector()
        );
        for fat in 0..2 {
            let off = fs.geometry().fat_offset(fat);
            assert_eq!(fs.disk().read(off, 4), &[0xF8, 0xFF, 0xFF, 0xFF]);
        }
        assert_eq!(
            table::count_free(fs.disk(), fs.geometry()),
            fs.geometry().cluster_count
        );
        assert!(fs.history().is_empty());
        assert_eq!(fs.now(), DateTime::default());
    }

    #[test]
    fn layout_regions_are_contiguous_and_cover_the_disk() {
        let fs = FatFs::format(FormatOptions::default()).unwrap();
        let regions = fs.layout();
        let names: Vec<&str> = regions.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "reserved (boot sector)",
                "FAT 0",
                "FAT 1",
                "root directory",
                "data"
            ]
        );
        assert_eq!(regions[0].kind, RegionKind::Boot);
        assert_eq!(regions[1].kind, RegionKind::AllocationTable);
        assert_eq!(regions[3].kind, RegionKind::Directory);
        assert_eq!(regions[4].kind, RegionKind::Data);
        let mut next = 0;
        for r in &regions {
            assert_eq!(r.sectors.start, next);
            next = r.sectors.end;
        }
        assert_eq!(next, 32768);
    }

    #[test]
    fn from_image_round_trips_and_validates() {
        let fs = FatFs::format(FormatOptions::default()).unwrap();
        let image = fs.disk().as_bytes().to_vec();
        let again = FatFs::from_image(image.clone()).unwrap();
        assert_eq!(again.geometry(), fs.geometry());
        assert_eq!(again.disk().as_bytes(), fs.disk().as_bytes());

        let mut bad = image.clone();
        bad[510] = 0;
        assert!(matches!(
            FatFs::from_image(bad),
            Err(Error::CorruptImage(_))
        ));
        assert!(matches!(
            FatFs::from_image(vec![0; 100]),
            Err(Error::CorruptImage(_))
        ));
        let short = image[..image.len() - 512].to_vec();
        assert!(matches!(
            FatFs::from_image(short),
            Err(Error::CorruptImage(_))
        ));
    }

    #[test]
    fn set_now_is_stored() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let t = DateTime::new(2026, 9, 21, 1, 2, 3);
        fs.set_now(t);
        assert_eq!(fs.now(), t);
    }

    use crate::dir_entry::{attr, lfn_checksum, LfnEntry, ShortEntry, DELETED, ENTRY_SIZE};
    use crate::name;

    fn write_root_slot(fs: &mut FatFs, slot: usize, bytes: &[u8; 32]) {
        let off = fs.geo.root_dir_offset() + slot * ENTRY_SIZE;
        fs.disk.write(off, bytes);
    }

    fn short(name: &[u8; 11], attributes: u8) -> ShortEntry {
        ShortEntry::new(*name, attributes, &DateTime::new(2026, 9, 21, 10, 0, 0))
    }

    #[test]
    fn empty_root_lists_nothing_and_stat_root_works() {
        let fs = FatFs::format(FormatOptions::default()).unwrap();
        assert_eq!(fs.list_dir("/").unwrap(), Vec::new());
        let root = fs.stat("/").unwrap();
        assert_eq!(root.name, "/");
        assert!(root.is_dir);
        assert_eq!(fs.stat("/NOPE"), Err(Error::NotFound));
        assert_eq!(fs.list_dir("/NOPE"), Err(Error::NotFound));
        assert_eq!(fs.list_dir("relative"), Err(Error::InvalidPath));
    }

    #[test]
    fn short_entries_are_listed_with_display_names_and_timestamps() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let mut e = short(b"README  TXT", attr::ARCHIVE);
        e.size = 5;
        write_root_slot(&mut fs, 0, &e.to_bytes());
        write_root_slot(
            &mut fs,
            1,
            &short(b"LABEL      ", attr::VOLUME_ID).to_bytes(),
        );
        write_root_slot(
            &mut fs,
            2,
            &short(b"DOCS       ", attr::DIRECTORY).to_bytes(),
        );
        let list = fs.list_dir("/").unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "README.TXT");
        assert_eq!(list[0].size, 5);
        assert!(!list[0].is_dir);
        assert_eq!(list[0].modified, Some(DateTime::new(2026, 9, 21, 10, 0, 0)));
        assert_eq!(list[0].accessed, Some(DateTime::new(2026, 9, 21, 0, 0, 0)));
        assert_eq!(list[1].name, "DOCS");
        assert!(list[1].is_dir);
        assert_eq!(fs.stat("/readme.txt").unwrap().name, "README.TXT");
    }

    #[test]
    fn long_names_are_reassembled_when_valid() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let short_name = *b"MYFILE~1TXT";
        let sum = lfn_checksum(&short_name);
        let chunks = name::lfn_chunks("My File.txt");
        write_root_slot(
            &mut fs,
            0,
            &LfnEntry::new(1, true, sum, &chunks[0]).to_bytes(),
        );
        write_root_slot(&mut fs, 1, &short(&short_name, attr::ARCHIVE).to_bytes());
        // A two-entry name: 14 A's, written highest order first.
        let long = "A".repeat(14);
        let short2 = *b"AAAAAA~1   ";
        let sum2 = lfn_checksum(&short2);
        let chunks2 = name::lfn_chunks(&long);
        write_root_slot(
            &mut fs,
            2,
            &LfnEntry::new(2, true, sum2, &chunks2[1]).to_bytes(),
        );
        write_root_slot(
            &mut fs,
            3,
            &LfnEntry::new(1, false, sum2, &chunks2[0]).to_bytes(),
        );
        write_root_slot(&mut fs, 4, &short(&short2, attr::ARCHIVE).to_bytes());
        let list = fs.list_dir("/").unwrap();
        assert_eq!(list[0].name, "My File.txt");
        assert_eq!(list[1].name, long);
        assert_eq!(fs.stat("/my file.TXT").unwrap().name, "My File.txt");
        assert_eq!(fs.stat("/MYFILE~1.TXT").unwrap().name, "My File.txt");
    }

    #[test]
    fn orphaned_or_mismatched_lfn_entries_are_ignored() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let chunks = name::lfn_chunks("Wrong.txt");
        // Checksum does not match the short name that follows.
        write_root_slot(
            &mut fs,
            0,
            &LfnEntry::new(1, true, 0x11, &chunks[0]).to_bytes(),
        );
        write_root_slot(&mut fs, 1, &short(b"REAL    TXT", attr::ARCHIVE).to_bytes());
        // A deleted LFN entry followed by a short entry.
        let mut deleted =
            LfnEntry::new(1, true, lfn_checksum(b"OTHER   TXT"), &chunks[0]).to_bytes();
        deleted[0] = DELETED;
        write_root_slot(&mut fs, 2, &deleted);
        write_root_slot(&mut fs, 3, &short(b"OTHER   TXT", attr::ARCHIVE).to_bytes());
        let names: Vec<String> = fs
            .list_dir("/")
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, vec!["REAL.TXT", "OTHER.TXT"]);
    }

    #[test]
    fn read_file_follows_the_chain_and_respects_size() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let clusters = table::allocate_chain(&mut fs.disk, &fs.geo, 2).unwrap();
        let cs = fs.geo.cluster_size();
        fs.disk.fill(fs.geo.cluster_offset(clusters[0]), cs, b'x');
        fs.disk.fill(fs.geo.cluster_offset(clusters[1]), cs, b'y');
        let mut e = short(b"DATA    BIN", attr::ARCHIVE);
        e.set_first_cluster(clusters[0]);
        e.size = (cs + 3) as u32;
        write_root_slot(&mut fs, 0, &e.to_bytes());
        let mut d = short(b"SUB        ", attr::DIRECTORY);
        d.set_first_cluster(clusters[1]);
        write_root_slot(&mut fs, 1, &d.to_bytes());
        write_root_slot(&mut fs, 2, &short(b"EMPTY   TXT", attr::ARCHIVE).to_bytes());

        let data = fs.read_file("/DATA.BIN").unwrap();
        assert_eq!(data.len(), cs + 3);
        assert!(data[..cs].iter().all(|&b| b == b'x'));
        assert_eq!(&data[cs..], b"yyy");
        assert_eq!(fs.read_file("/EMPTY.TXT").unwrap(), Vec::<u8>::new());
        assert_eq!(fs.read_file("/SUB"), Err(Error::IsADirectory));
        assert_eq!(fs.read_file("/"), Err(Error::IsADirectory));
        assert_eq!(fs.read_file("/MISSING"), Err(Error::NotFound));
        assert_eq!(fs.list_dir("/DATA.BIN"), Err(Error::NotADirectory));
    }

    #[test]
    fn nested_directory_is_resolved_through_its_cluster() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let c = table::allocate_chain(&mut fs.disk, &fs.geo, 1).unwrap()[0];
        let mut d = short(b"SUB        ", attr::DIRECTORY);
        d.set_first_cluster(c);
        write_root_slot(&mut fs, 0, &d.to_bytes());
        let base = fs.geo.cluster_offset(c);
        let mut dot = short(b".          ", attr::DIRECTORY);
        dot.set_first_cluster(c);
        fs.disk.write(base, &dot.to_bytes());
        fs.disk.write(
            base + ENTRY_SIZE,
            &short(b"..         ", attr::DIRECTORY).to_bytes(),
        );
        fs.disk.write(
            base + 2 * ENTRY_SIZE,
            &short(b"INNER   TXT", attr::ARCHIVE).to_bytes(),
        );
        let names: Vec<String> = fs
            .list_dir("/SUB")
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, vec!["INNER.TXT"]);
        assert_eq!(fs.stat("/sub/inner.txt").unwrap().name, "INNER.TXT");
    }
}
