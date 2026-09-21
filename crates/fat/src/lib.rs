//! Byte-accurate FAT16 filesystem on an in-memory disk.

pub mod boot_sector;

pub use boot_sector::{BootSector, FatVariant, FormatOptions, Geometry};
