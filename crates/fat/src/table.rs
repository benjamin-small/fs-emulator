//! The file allocation table: one entry per cluster saying whether it is free,
//! which cluster follows it, or that it ends a chain.

use std::fmt;

use crate::boot_sector::{FatVariant, Geometry};
use crate::events::FatEvent;
use fs_core::{Disk, Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatEntry {
    Free,
    Next(u32),
    EndOfChain,
    Bad,
    Reserved,
}

impl fmt::Display for FatEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FatEntry::Free => write!(f, "free"),
            FatEntry::Next(n) => write!(f, "next -> {n}"),
            FatEntry::EndOfChain => write!(f, "end of chain"),
            FatEntry::Bad => write!(f, "bad"),
            FatEntry::Reserved => write!(f, "reserved"),
        }
    }
}

pub fn decode16(raw: u16) -> FatEntry {
    match raw {
        0x0000 => FatEntry::Free,
        0x0002..=0xFFEF => FatEntry::Next(raw as u32),
        0xFFF7 => FatEntry::Bad,
        0xFFF8..=0xFFFF => FatEntry::EndOfChain,
        _ => FatEntry::Reserved,
    }
}

pub fn encode16(entry: FatEntry) -> u16 {
    match entry {
        FatEntry::Free => 0x0000,
        FatEntry::Next(n) => n as u16,
        FatEntry::EndOfChain => 0xFFFF,
        FatEntry::Bad => 0xFFF7,
        FatEntry::Reserved => 0x0001,
    }
}

/// Decode a raw FAT entry according to the volume's variant, so a future
/// FAT32 addition cannot silently be read with the FAT16 width.
fn decode(geo: &Geometry, raw: &[u8]) -> FatEntry {
    match geo.variant {
        FatVariant::Fat16 => decode16(u16::from_le_bytes([raw[0], raw[1]])),
    }
}

/// Encode a FAT entry according to the volume's variant.
fn encode(geo: &Geometry, entry: FatEntry) -> Vec<u8> {
    match geo.variant {
        FatVariant::Fat16 => encode16(entry).to_le_bytes().to_vec(),
    }
}

pub fn read_entry_from(disk: &Disk, geo: &Geometry, fat: u8, cluster: u32) -> FatEntry {
    let off = geo.fat_entry_offset(fat, cluster);
    let b = disk.read(off, 2);
    decode(geo, b)
}

/// Read from the first FAT, which is the one drivers trust.
pub fn read_entry(disk: &Disk, geo: &Geometry, cluster: u32) -> FatEntry {
    read_entry_from(disk, geo, 0, cluster)
}

/// Write the entry into every FAT copy.
pub fn write_entry(disk: &mut Disk, geo: &Geometry, cluster: u32, value: FatEntry) {
    let raw = encode(geo, value);
    for fat in 0..geo.fat_count {
        let off = geo.fat_entry_offset(fat, cluster);
        disk.write(off, &raw);
        disk.event(Box::new(FatEvent::FatEntrySet {
            fat,
            cluster,
            value,
            range: off..off + 2,
        }));
    }
}

pub fn count_free(disk: &Disk, geo: &Geometry) -> u32 {
    (2..=geo.max_cluster())
        .filter(|&c| read_entry(disk, geo, c) == FatEntry::Free)
        .count() as u32
}

/// Allocate `count` free clusters and link them into a chain. Fails with
/// `DiskFull` before touching the disk if there are not enough.
pub fn allocate_chain(disk: &mut Disk, geo: &Geometry, count: u32) -> Result<Vec<u32>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let free: Vec<u32> = (2..=geo.max_cluster())
        .filter(|&c| read_entry(disk, geo, c) == FatEntry::Free)
        .take(count as usize)
        .collect();
    if free.len() < count as usize {
        return Err(Error::DiskFull);
    }
    for (i, &cluster) in free.iter().enumerate() {
        let value = match free.get(i + 1) {
            Some(&next) => FatEntry::Next(next),
            None => FatEntry::EndOfChain,
        };
        write_entry(disk, geo, cluster, value);
        disk.event(Box::new(FatEvent::ClusterAllocated {
            cluster,
            range: geo.cluster_range(cluster),
        }));
    }
    Ok(free)
}

/// Every cluster in the chain starting at `start`, in order.
pub fn chain(disk: &Disk, geo: &Geometry, start: u32) -> Result<Vec<u32>> {
    let mut out = Vec::new();
    let mut current = start;
    loop {
        if !geo.is_valid_cluster(current) {
            return Err(Error::CorruptImage(format!(
                "cluster chain points to invalid cluster {current}"
            )));
        }
        if out.len() as u32 > geo.cluster_count {
            return Err(Error::CorruptImage(format!(
                "cluster chain starting at {start} loops"
            )));
        }
        out.push(current);
        match read_entry(disk, geo, current) {
            FatEntry::Next(next) => current = next,
            FatEntry::EndOfChain => return Ok(out),
            other => {
                return Err(Error::CorruptImage(format!(
                    "cluster {current} inside a chain is marked {other}"
                )));
            }
        }
    }
}

pub fn free_chain(disk: &mut Disk, geo: &Geometry, start: u32) -> Result<()> {
    for cluster in chain(disk, geo, start)? {
        write_entry(disk, geo, cluster, FatEntry::Free);
        disk.event(Box::new(FatEvent::ClusterFreed {
            cluster,
            range: geo.cluster_range(cluster),
        }));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot_sector::{BootSector, FormatOptions};
    use fs_core::{Disk, Error};

    #[test]
    fn decode_covers_every_range() {
        assert_eq!(decode16(0x0000), FatEntry::Free);
        assert_eq!(decode16(0x0001), FatEntry::Reserved);
        assert_eq!(decode16(0x0002), FatEntry::Next(2));
        assert_eq!(decode16(0xFFEF), FatEntry::Next(0xFFEF));
        assert_eq!(decode16(0xFFF0), FatEntry::Reserved);
        assert_eq!(decode16(0xFFF7), FatEntry::Bad);
        assert_eq!(decode16(0xFFF8), FatEntry::EndOfChain);
        assert_eq!(decode16(0xFFFF), FatEntry::EndOfChain);
    }

    #[test]
    fn encode_round_trips() {
        for e in [
            FatEntry::Free,
            FatEntry::Next(77),
            FatEntry::EndOfChain,
            FatEntry::Bad,
            FatEntry::Reserved,
        ] {
            assert_eq!(decode16(encode16(e)), e);
        }
        assert_eq!(encode16(FatEntry::EndOfChain), 0xFFFF);
    }

    #[test]
    fn display() {
        assert_eq!(FatEntry::Next(9).to_string(), "next -> 9");
        assert_eq!(FatEntry::EndOfChain.to_string(), "end of chain");
    }

    fn tiny() -> (Disk, Geometry) {
        let opts = FormatOptions {
            total_sectors: 64,
            sectors_per_cluster: 1,
            root_entries: 16,
            enforce_fat16_range: false,
            ..Default::default()
        };
        let geo = BootSector::from_options(&opts).unwrap().geometry().unwrap();
        let mut disk = Disk::new(geo.bytes_per_sector, geo.total_sectors);
        for fat in 0..geo.fat_count {
            disk.write(geo.fat_offset(fat), &[0xF8, 0xFF, 0xFF, 0xFF]);
        }
        (disk, geo)
    }

    #[test]
    fn write_entry_mirrors_to_every_fat_and_emits_events() {
        let (mut disk, geo) = tiny();
        disk.begin_op("t");
        write_entry(&mut disk, &geo, 2, FatEntry::EndOfChain);
        let rec = disk.end_op();
        assert_eq!(read_entry_from(&disk, &geo, 0, 2), FatEntry::EndOfChain);
        assert_eq!(read_entry_from(&disk, &geo, 1, 2), FatEntry::EndOfChain);
        assert_eq!(rec.event_kinds(), vec!["fat_entry_set", "fat_entry_set"]);
        assert_eq!(rec.changes.len(), 2);
    }

    #[test]
    fn allocate_links_clusters_and_chain_follows_them() {
        let (mut disk, geo) = tiny();
        disk.begin_op("t");
        let a = allocate_chain(&mut disk, &geo, 3).unwrap();
        assert_eq!(a, vec![2, 3, 4]);
        assert_eq!(read_entry(&disk, &geo, 2), FatEntry::Next(3));
        assert_eq!(read_entry(&disk, &geo, 4), FatEntry::EndOfChain);
        assert_eq!(chain(&disk, &geo, 2).unwrap(), vec![2, 3, 4]);
        let b = allocate_chain(&mut disk, &geo, 1).unwrap();
        assert_eq!(b, vec![5]);
        assert_eq!(
            allocate_chain(&mut disk, &geo, 0).unwrap(),
            Vec::<u32>::new()
        );
        let kinds = disk.end_op().event_kinds();
        assert_eq!(
            kinds.iter().filter(|k| **k == "cluster_allocated").count(),
            4
        );
    }

    #[test]
    fn allocate_is_atomic_when_disk_is_full() {
        let (mut disk, geo) = tiny();
        let free = count_free(&disk, &geo);
        assert_eq!(free, geo.cluster_count);
        assert_eq!(
            allocate_chain(&mut disk, &geo, free + 1),
            Err(Error::DiskFull)
        );
        assert_eq!(count_free(&disk, &geo), free);
        allocate_chain(&mut disk, &geo, free).unwrap();
        assert_eq!(count_free(&disk, &geo), 0);
    }

    #[test]
    fn free_chain_releases_every_cluster() {
        let (mut disk, geo) = tiny();
        allocate_chain(&mut disk, &geo, 3).unwrap();
        disk.begin_op("t");
        free_chain(&mut disk, &geo, 2).unwrap();
        assert_eq!(
            disk.end_op()
                .event_kinds()
                .iter()
                .filter(|k| **k == "cluster_freed")
                .count(),
            3
        );
        assert_eq!(count_free(&disk, &geo), geo.cluster_count);
    }

    #[test]
    fn chain_detects_corruption() {
        let (mut disk, geo) = tiny();
        write_entry(&mut disk, &geo, 2, FatEntry::Next(3));
        write_entry(&mut disk, &geo, 3, FatEntry::Next(2));
        assert!(matches!(chain(&disk, &geo, 2), Err(Error::CorruptImage(_))));
        write_entry(&mut disk, &geo, 3, FatEntry::Free);
        assert!(matches!(chain(&disk, &geo, 2), Err(Error::CorruptImage(_))));
        assert!(matches!(chain(&disk, &geo, 0), Err(Error::CorruptImage(_))));
        assert!(matches!(
            chain(&disk, &geo, geo.max_cluster() + 1),
            Err(Error::CorruptImage(_))
        ));
    }
}
