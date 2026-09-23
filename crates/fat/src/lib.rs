//! Byte-accurate FAT filesystems on an in-memory disk. FAT16 is implemented;
//! FAT32's seams (`FatVariant`, `u32` clusters, the entry codec) are in place,
//! and `docs/ROADMAP.md` lists what remains.

pub mod boot_sector;
pub mod dir;
pub mod dir_entry;
pub mod events;
pub mod fs;
pub mod name;
pub mod table;

pub use boot_sector::{BootSector, FatVariant, FormatOptions, Geometry};
pub use dir_entry::{LfnEntry, ShortEntry};
pub use events::{EntryKind, FatEvent};
pub use fs::{ClusterOwner, FatFs, RawEntry};
pub use table::FatEntry;
