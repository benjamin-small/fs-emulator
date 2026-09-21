//! Filesystem-neutral descriptions of where things live on disk, so a UI can
//! draw any filesystem's regions and annotate any sector's bytes.

use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    Boot,
    Metadata,
    AllocationTable,
    Directory,
    Data,
    Reserved,
    Other,
}

/// A contiguous run of sectors with one purpose, e.g. "FAT 0".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub name: String,
    pub sectors: Range<u64>,
    pub kind: RegionKind,
}

/// A label for a byte range inside one sector, e.g. bytes 11..13 of the boot
/// sector are `bytes per sector = 512`. `range` is relative to the sector start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    pub range: Range<usize>,
    pub label: String,
    pub value: String,
}
