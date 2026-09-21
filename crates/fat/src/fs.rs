//! `FatFs`: a FAT16 volume on an in-memory disk. The disk bytes are the only
//! state; every method re-reads what it needs from them.

use crate::boot_sector::{BootSector, FormatOptions, Geometry};
use fs_core::{DateTime, Disk, Error, OpRecord, Region, RegionKind, Result};

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
}
