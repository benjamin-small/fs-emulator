//! Filesystem-agnostic core: the virtual disk, change tracking, shared types,
//! and the `FileSystem` trait that every emulated filesystem implements.

pub mod disk;
pub mod error;
pub mod fs;
pub mod layout;
pub mod path;
pub mod trace;
pub mod types;

pub use disk::{finish_op, raw_write, raw_write_op, run_op, Disk};
pub use error::{Error, Result};
pub use fs::FileSystem;
pub use layout::{Annotation, Region, RegionKind};
pub use trace::{ByteChange, Event, OpRecord, RawWrite};
pub use types::{DateTime, EntryInfo};
