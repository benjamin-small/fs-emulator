//! Filesystem-agnostic core: the virtual disk, change tracking, shared types,
//! and the `FileSystem` trait that every emulated filesystem implements.

pub mod disk;
pub mod error;
pub mod path;
pub mod trace;
pub mod types;

pub use disk::Disk;
pub use error::{Error, Result};
pub use trace::{ByteChange, Event, OpRecord};
pub use types::{DateTime, EntryInfo};
