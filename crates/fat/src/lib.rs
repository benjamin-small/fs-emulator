//! Byte-accurate FAT16 filesystem on an in-memory disk.

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
pub use fs::{FatFs, Located};
pub use table::FatEntry;
