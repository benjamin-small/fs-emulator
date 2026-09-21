//! Filesystem-agnostic core: the virtual disk, change tracking, shared types,
//! and the `FileSystem` trait that every emulated filesystem implements.

pub mod error;
pub mod path;
pub mod types;

pub use error::{Error, Result};
pub use types::{DateTime, EntryInfo};
