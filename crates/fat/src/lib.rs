//! Byte-accurate FAT16 filesystem on an in-memory disk.

pub mod boot_sector;
pub mod dir_entry;
pub mod name;

pub use boot_sector::{BootSector, FatVariant, FormatOptions, Geometry};
pub use dir_entry::{LfnEntry, ShortEntry};
