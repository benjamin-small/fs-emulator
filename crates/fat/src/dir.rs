//! A directory as a sequence of 32-byte slots, whether it lives in the fixed
//! root region or in a cluster chain.

use crate::boot_sector::Geometry;
use crate::dir_entry::{DELETED, ENTRY_SIZE, FREE};
use crate::events::FatEvent;
use crate::table::{self, FatEntry};
use fs_core::{Disk, Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirLocation {
    Root,
    /// First cluster of the directory's chain.
    Cluster(u32),
}

impl DirLocation {
    /// A `..` entry (or any entry) with cluster 0 means the root.
    pub fn from_cluster(cluster: u32) -> DirLocation {
        if cluster == 0 {
            DirLocation::Root
        } else {
            DirLocation::Cluster(cluster)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub index: usize,
    /// Absolute byte offset on disk.
    pub offset: usize,
}

/// Every slot of the directory, in order.
pub fn slots(disk: &Disk, geo: &Geometry, loc: DirLocation) -> Result<Vec<Slot>> {
    match loc {
        DirLocation::Root => Ok((0..geo.root_entries)
            .map(|i| Slot {
                index: i,
                offset: geo.root_dir_offset() + i * ENTRY_SIZE,
            })
            .collect()),
        DirLocation::Cluster(start) => {
            let per_cluster = geo.cluster_size() / ENTRY_SIZE;
            let mut out = Vec::new();
            for cluster in table::chain(disk, geo, start)? {
                let base = geo.cluster_offset(cluster);
                for i in 0..per_cluster {
                    out.push(Slot {
                        index: out.len(),
                        offset: base + i * ENTRY_SIZE,
                    });
                }
            }
            Ok(out)
        }
    }
}

/// Index of the first slot of a run of `count` consecutive free or deleted
/// slots, if one exists.
pub fn find_free_run(
    disk: &Disk,
    geo: &Geometry,
    loc: DirLocation,
    count: usize,
) -> Result<Option<usize>> {
    let mut run_start = None;
    let mut run = 0;
    for slot in slots(disk, geo, loc)? {
        let first = disk.read(slot.offset, 1)[0];
        if first == FREE || first == DELETED {
            if run == 0 {
                run_start = Some(slot.index);
            }
            run += 1;
            if run == count {
                return Ok(run_start);
            }
        } else {
            run = 0;
            run_start = None;
        }
    }
    Ok(None)
}

/// Append one zeroed cluster to a cluster-chain directory. The root
/// directory has a fixed size on FAT16, so it returns `DirectoryFull`.
pub fn grow(disk: &mut Disk, geo: &Geometry, loc: DirLocation, dir_name: &str) -> Result<()> {
    let start = match loc {
        DirLocation::Root => return Err(Error::DirectoryFull),
        DirLocation::Cluster(c) => c,
    };
    let existing = table::chain(disk, geo, start)?;
    let new = table::allocate_chain(disk, geo, 1)?[0];
    let last = *existing.last().expect("chain is never empty");
    table::write_entry(disk, geo, last, FatEntry::Next(new));
    disk.fill(geo.cluster_offset(new), geo.cluster_size(), 0);
    disk.event(Box::new(FatEvent::DirectoryGrown {
        dir: dir_name.to_string(),
        cluster: new,
        range: geo.cluster_range(new),
    }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot_sector::{BootSector, FormatOptions};
    use crate::table;
    use fs_core::{Disk, Error};

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
    fn root_slots_cover_the_fixed_region() {
        let (disk, geo) = tiny();
        let s = slots(&disk, &geo, DirLocation::Root).unwrap();
        assert_eq!(s.len(), 16);
        assert_eq!(
            s[0],
            Slot {
                index: 0,
                offset: geo.root_dir_offset()
            }
        );
        assert_eq!(s[15].offset, geo.root_dir_offset() + 15 * ENTRY_SIZE);
    }

    #[test]
    fn cluster_slots_follow_the_chain() {
        let (mut disk, geo) = tiny();
        let clusters = table::allocate_chain(&mut disk, &geo, 2).unwrap();
        let s = slots(&disk, &geo, DirLocation::Cluster(clusters[0])).unwrap();
        assert_eq!(s.len(), 2 * 512 / ENTRY_SIZE);
        assert_eq!(s[16].offset, geo.cluster_offset(clusters[1]));
        assert_eq!(s[16].index, 16);
        assert_eq!(DirLocation::from_cluster(0), DirLocation::Root);
        assert_eq!(DirLocation::from_cluster(5), DirLocation::Cluster(5));
    }

    #[test]
    fn find_free_run_counts_deleted_slots_and_needs_contiguity() {
        let (mut disk, geo) = tiny();
        let base = geo.root_dir_offset();
        disk.write(base, b"USED       ");
        disk.write(base + ENTRY_SIZE, &[DELETED]);
        disk.write(base + 2 * ENTRY_SIZE, b"USED2      ");
        assert_eq!(
            find_free_run(&disk, &geo, DirLocation::Root, 1).unwrap(),
            Some(1)
        );
        assert_eq!(
            find_free_run(&disk, &geo, DirLocation::Root, 2).unwrap(),
            Some(3)
        );
        assert_eq!(
            find_free_run(&disk, &geo, DirLocation::Root, 13).unwrap(),
            Some(3)
        );
        assert_eq!(
            find_free_run(&disk, &geo, DirLocation::Root, 14).unwrap(),
            None
        );
    }

    #[test]
    fn grow_adds_a_zeroed_cluster_to_the_chain_but_not_to_root() {
        let (mut disk, geo) = tiny();
        let first = table::allocate_chain(&mut disk, &geo, 1).unwrap()[0];
        disk.fill(geo.cluster_offset(first), 512, 0xEE);
        disk.begin_op("t");
        grow(&mut disk, &geo, DirLocation::Cluster(first), "/SUB").unwrap();
        let rec = disk.end_op();
        let chain = table::chain(&disk, &geo, first).unwrap();
        assert_eq!(chain.len(), 2);
        assert!(disk
            .read(geo.cluster_offset(chain[1]), 512)
            .iter()
            .all(|&b| b == 0));
        assert!(rec.event_kinds().contains(&"directory_grown"));
        assert_eq!(
            grow(&mut disk, &geo, DirLocation::Root, "/"),
            Err(Error::DirectoryFull)
        );
    }
}
