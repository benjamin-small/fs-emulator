# FAT16 Emulator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Rust workspace with a filesystem-agnostic core crate (`fs-core`) and a byte-accurate FAT16 implementation (`fat`) that records every byte change and semantic event per operation.

**Architecture:** `fs-core` owns the in-memory `Disk` (sector-addressed `Vec<u8>` with change journaling), the `OpRecord`/`Event` trace types, shared types, and the `FileSystem` trait. `fat` builds the FAT16 layers on top: boot sector, FAT table, directory entries, names, directories, and the `FatFs` type that implements the trait. The disk bytes are the only state; nothing is cached in parsed structs.

**Tech Stack:** Rust 2021, cargo workspace, zero external dependencies. Tests use built-in `#[test]`.

**Spec:** `docs/superpowers/specs/2026-09-21-fat16-emulator-design.md`

## Global Constraints

- Zero external dependencies in both crates.
- `std` only for `Vec`, `String`, `fmt`, collections. No I/O, no `SystemTime`, no threads. Both crates must build for `wasm32-unknown-unknown`.
- Cluster numbers are `u32` everywhere (FAT32 seam).
- All disk mutations go through `Disk::write` or `Disk::fill` so they are journaled.
- Default `DateTime` is 1980-01-01 00:00:00.
- Rust edition 2021. Run `cargo fmt` and `cargo clippy --all-targets` before each commit; fix warnings.
- Commit messages: conventional style (`feat:`, `test:`, `chore:`), no attribution lines.

## File Structure

```
Cargo.toml                         workspace root
.gitignore                         target/
crates/fs-core/Cargo.toml
crates/fs-core/src/lib.rs          module list + re-exports
crates/fs-core/src/error.rs        Error enum, Result alias
crates/fs-core/src/types.rs        DateTime, EntryInfo
crates/fs-core/src/path.rs         parse, split_parent
crates/fs-core/src/trace.rs        Event trait, ByteChange, OpRecord
crates/fs-core/src/disk.rs         Disk with journaling
crates/fs-core/src/layout.rs       Region, RegionKind, Annotation
crates/fs-core/src/fs.rs           FileSystem trait
crates/fat/Cargo.toml
crates/fat/src/lib.rs              module list + re-exports
crates/fat/src/boot_sector.rs      FormatOptions, BootSector, Geometry, FatVariant
crates/fat/src/dir_entry.rs        ShortEntry, LfnEntry, attr, date/time packing, checksum
crates/fat/src/name.rs             8.3 validation, short-name generation, UCS-2, LFN chunks
crates/fat/src/table.rs            FatEntry, read/write, allocate, chain, free
crates/fat/src/events.rs           FatEvent, EntryKind
crates/fat/src/dir.rs              DirLocation, Slot, slots, find_free_run, grow
crates/fat/src/fs.rs               FatFs: format, from_image, ops, inspection, trait impl
crates/fat/tests/fat16.rs          integration tests
crates/fat/tests/mount_macos.rs    #[ignore] hdiutil validation
README.md
```

---

### Task 1: Workspace scaffold and fs-core error, types, path

**Files:**
- Create: `Cargo.toml`, `crates/fs-core/Cargo.toml`, `crates/fs-core/src/lib.rs`, `crates/fs-core/src/error.rs`, `crates/fs-core/src/types.rs`, `crates/fs-core/src/path.rs`, `crates/fat/Cargo.toml`, `crates/fat/src/lib.rs`

**Interfaces:**
- Produces: `fs_core::Error`, `fs_core::Result<T>`, `fs_core::DateTime::new(year,month,day,hour,minute,second)`, `fs_core::EntryInfo`, `fs_core::path::parse(&str) -> Result<Vec<String>>`, `fs_core::path::split_parent(&str) -> Result<(Vec<String>, String)>`

- [ ] **Step 1: Create the workspace and crate manifests**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/fs-core", "crates/fat"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
```

`crates/fs-core/Cargo.toml`:
```toml
[package]
name = "fs-core"
description = "Filesystem-agnostic core: virtual disk, change tracking, and the FileSystem trait"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
```

`crates/fat/Cargo.toml`:
```toml
[package]
name = "fat"
description = "Byte-accurate FAT16 filesystem emulator on an in-memory disk"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
fs-core = { path = "../fs-core" }
```

`crates/fat/src/lib.rs` (temporary, filled in Task 4):
```rust
//! Byte-accurate FAT16 filesystem on an in-memory disk.
```

`crates/fs-core/src/lib.rs`:
```rust
//! Filesystem-agnostic core: the virtual disk, change tracking, shared types,
//! and the `FileSystem` trait that every emulated filesystem implements.

pub mod error;
pub mod path;
pub mod types;

pub use error::{Error, Result};
pub use types::{DateTime, EntryInfo};
```

- [ ] **Step 2: Write failing tests for error display, DateTime default, and path parsing**

`crates/fs-core/src/error.rs` (tests only for now; the module body is written in Step 4):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_human_readable() {
        assert_eq!(Error::NotFound.to_string(), "not found");
        assert_eq!(
            Error::InvalidGeometry("too small".into()).to_string(),
            "invalid geometry: too small"
        );
    }

    #[test]
    fn implements_std_error() {
        fn takes(_: &dyn std::error::Error) {}
        takes(&Error::DiskFull);
    }
}
```

`crates/fs-core/src/types.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_datetime_is_fat_epoch() {
        assert_eq!(DateTime::default(), DateTime::new(1980, 1, 1, 0, 0, 0));
    }

    #[test]
    fn datetime_display() {
        assert_eq!(DateTime::new(2026, 9, 21, 7, 5, 9).to_string(), "2026-09-21 07:05:09");
    }
}
```

`crates/fs-core/src/path.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    #[test]
    fn root_parses_to_empty() {
        assert_eq!(parse("/").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn components_are_split_on_either_separator() {
        assert_eq!(parse("/A/B.TXT").unwrap(), vec!["A", "B.TXT"]);
        assert_eq!(parse("\\A\\B.TXT").unwrap(), vec!["A", "B.TXT"]);
        assert_eq!(parse("//A//").unwrap(), vec!["A"]);
    }

    #[test]
    fn rejects_relative_dot_and_empty() {
        assert_eq!(parse("A"), Err(Error::InvalidPath));
        assert_eq!(parse(""), Err(Error::InvalidPath));
        assert_eq!(parse("/./A"), Err(Error::InvalidPath));
        assert_eq!(parse("/../A"), Err(Error::InvalidPath));
    }

    #[test]
    fn split_parent_returns_parent_components_and_name() {
        assert_eq!(split_parent("/A/B").unwrap(), (vec!["A".to_string()], "B".to_string()));
        assert_eq!(split_parent("/B").unwrap(), (vec![], "B".to_string()));
        assert_eq!(split_parent("/"), Err(Error::InvalidPath));
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p fs-core`
Expected: compile errors (`Error`, `DateTime`, `parse` not defined).

- [ ] **Step 4: Implement error, types, path**

`crates/fs-core/src/error.rs` (above the tests module):
```rust
use std::fmt;

/// Every error an emulated filesystem can return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NotFound,
    AlreadyExists,
    InvalidPath,
    InvalidName,
    DiskFull,
    DirectoryFull,
    NotADirectory,
    IsADirectory,
    DirectoryNotEmpty,
    FileTooLarge,
    InvalidGeometry(String),
    CorruptImage(String),
    Unsupported(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound => write!(f, "not found"),
            Error::AlreadyExists => write!(f, "already exists"),
            Error::InvalidPath => write!(f, "invalid path"),
            Error::InvalidName => write!(f, "invalid name"),
            Error::DiskFull => write!(f, "disk full"),
            Error::DirectoryFull => write!(f, "directory full"),
            Error::NotADirectory => write!(f, "not a directory"),
            Error::IsADirectory => write!(f, "is a directory"),
            Error::DirectoryNotEmpty => write!(f, "directory not empty"),
            Error::FileTooLarge => write!(f, "file too large"),
            Error::InvalidGeometry(msg) => write!(f, "invalid geometry: {msg}"),
            Error::CorruptImage(msg) => write!(f, "corrupt image: {msg}"),
            Error::Unsupported(msg) => write!(f, "unsupported: {msg}"),
        }
    }
}

impl std::error::Error for Error {}
```

`crates/fs-core/src/types.rs`:
```rust
use std::fmt;

/// A calendar timestamp with second resolution. Filesystems pack it into
/// their own on-disk format (FAT rounds seconds down to an even number).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl DateTime {
    pub const fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Self {
        Self { year, month, day, hour, minute, second }
    }
}

impl Default for DateTime {
    /// The FAT epoch, 1980-01-01 00:00:00, so tests are deterministic.
    fn default() -> Self {
        Self::new(1980, 1, 1, 0, 0, 0)
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

/// What `list_dir` and `stat` report for one entry, in filesystem-neutral terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo {
    /// Display name: the long name when one exists, otherwise the short name.
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub created: Option<DateTime>,
    pub modified: Option<DateTime>,
    /// FAT stores only a date here; time fields are zero.
    pub accessed: Option<DateTime>,
}
```

`crates/fs-core/src/path.rs`:
```rust
//! Absolute path parsing shared by every filesystem. Case handling is left to
//! the filesystem; components are returned exactly as written.

use crate::{Error, Result};

fn is_separator(c: char) -> bool {
    c == '/' || c == '\\'
}

/// Split an absolute path into components. `/` parses to an empty vector.
/// Repeated separators collapse; `.` and `..` components are rejected.
pub fn parse(path: &str) -> Result<Vec<String>> {
    if !path.starts_with(is_separator) {
        return Err(Error::InvalidPath);
    }
    let mut parts = Vec::new();
    for component in path.split(is_separator) {
        if component.is_empty() {
            continue;
        }
        if component == "." || component == ".." {
            return Err(Error::InvalidPath);
        }
        parts.push(component.to_string());
    }
    Ok(parts)
}

/// Split into (parent components, final name). The root has no name, so
/// `/` is `InvalidPath`.
pub fn split_parent(path: &str) -> Result<(Vec<String>, String)> {
    let mut parts = parse(path)?;
    let name = parts.pop().ok_or(Error::InvalidPath)?;
    Ok((parts, name))
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p fs-core`
Expected: 8 tests pass.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates
git commit -m "feat(fs-core): workspace scaffold with error, types, and path parsing"
```

---

### Task 2: fs-core trace and Disk

**Files:**
- Create: `crates/fs-core/src/trace.rs`, `crates/fs-core/src/disk.rs`
- Modify: `crates/fs-core/src/lib.rs`

**Interfaces:**
- Produces: `Event` trait (`kind()`, `region()`, `clone_box()`), `ByteChange { offset, before, after }`, `OpRecord { op, changes, events }` with `OpRecord::new`, `event_kinds()`, `changed_sectors(sector_size)`; `Disk::new(sector_size, sector_count)`, `Disk::from_bytes(sector_size, bytes)`, `sector_size()`, `sector_count()`, `len()`, `as_bytes()`, `sector(n)`, `read(offset, len)`, `write(offset, data)`, `fill(offset, len, byte)`, `begin_op(name)`, `event(Box<dyn Event>)`, `end_op() -> OpRecord`, `op_open()`.

- [ ] **Step 1: Write failing tests**

`crates/fs-core/src/disk.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{ByteChange, Event};
    use std::fmt;
    use std::ops::Range;

    #[derive(Debug, Clone)]
    struct Dummy;
    impl fmt::Display for Dummy {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "dummy")
        }
    }
    impl Event for Dummy {
        fn kind(&self) -> &'static str {
            "dummy"
        }
        fn region(&self) -> Option<Range<usize>> {
            Some(0..1)
        }
        fn clone_box(&self) -> Box<dyn Event> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn new_disk_is_zeroed_with_expected_size() {
        let disk = Disk::new(512, 4);
        assert_eq!(disk.len(), 2048);
        assert_eq!(disk.sector_count(), 4);
        assert!(disk.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn from_bytes_rejects_partial_sector() {
        assert!(matches!(Disk::from_bytes(512, vec![0; 513]), Err(crate::Error::InvalidGeometry(_))));
        assert_eq!(Disk::from_bytes(512, vec![0; 1024]).unwrap().sector_count(), 2);
    }

    #[test]
    fn write_outside_an_op_changes_bytes_without_recording() {
        let mut disk = Disk::new(512, 1);
        disk.write(3, &[7, 8]);
        assert_eq!(disk.read(3, 2), &[7, 8]);
        assert!(!disk.op_open());
    }

    #[test]
    fn write_inside_an_op_records_before_and_after() {
        let mut disk = Disk::new(512, 2);
        disk.begin_op("test");
        disk.write(3, &[7, 8]);
        disk.fill(600, 3, 0xAA);
        disk.event(Box::new(Dummy));
        let record = disk.end_op();
        assert_eq!(record.op, "test");
        assert_eq!(
            record.changes,
            vec![
                ByteChange { offset: 3, before: vec![0, 0], after: vec![7, 8] },
                ByteChange { offset: 600, before: vec![0, 0, 0], after: vec![0xAA; 3] },
            ]
        );
        assert_eq!(record.event_kinds(), vec!["dummy"]);
        assert_eq!(record.changed_sectors(512), vec![0, 1]);
        assert!(!disk.op_open());
    }

    #[test]
    fn events_outside_an_op_are_dropped() {
        let mut disk = Disk::new(512, 1);
        disk.event(Box::new(Dummy)); // must not panic
        disk.begin_op("x");
        assert!(disk.end_op().events.is_empty());
    }

    #[test]
    fn sector_returns_the_right_slice() {
        let mut disk = Disk::new(4, 3);
        disk.write(4, &[1, 2, 3, 4]);
        assert_eq!(disk.sector(1), &[1, 2, 3, 4]);
        assert_eq!(disk.sector(2), &[0, 0, 0, 0]);
    }

    #[test]
    fn op_record_is_cloneable() {
        let mut disk = Disk::new(4, 1);
        disk.begin_op("clone");
        disk.event(Box::new(Dummy));
        let record = disk.end_op();
        let copy = record.clone();
        assert_eq!(copy.events[0].to_string(), "dummy");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p fs-core disk`
Expected: compile error, `Disk` not found.

- [ ] **Step 3: Implement trace and disk**

`crates/fs-core/src/trace.rs`:
```rust
//! What one filesystem operation did to the disk: every byte written and
//! every semantic event the filesystem chose to report.

use std::fmt;
use std::ops::Range;

/// A semantic event emitted by a filesystem during an operation, such as
/// "allocated cluster 5". Each filesystem defines its own enum implementing
/// this trait; the core never knows about clusters or inodes.
pub trait Event: fmt::Display + fmt::Debug {
    /// A stable machine-readable label, e.g. `"cluster_allocated"`.
    fn kind(&self) -> &'static str;
    /// The absolute byte range on disk this event concerns, if any.
    fn region(&self) -> Option<Range<usize>>;
    fn clone_box(&self) -> Box<dyn Event>;
}

impl Clone for Box<dyn Event> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// One contiguous write: the bytes at `offset` before and after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteChange {
    pub offset: usize,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

/// Everything one operation did, in order.
#[derive(Debug, Clone)]
pub struct OpRecord {
    /// e.g. `"create_file /DOCS/NOTES.TXT"`
    pub op: String,
    pub changes: Vec<ByteChange>,
    pub events: Vec<Box<dyn Event>>,
}

impl OpRecord {
    pub fn new(op: impl Into<String>) -> Self {
        Self { op: op.into(), changes: Vec::new(), events: Vec::new() }
    }

    pub fn event_kinds(&self) -> Vec<&'static str> {
        self.events.iter().map(|e| e.kind()).collect()
    }

    /// Sorted, de-duplicated sector numbers touched by `changes`.
    pub fn changed_sectors(&self, sector_size: usize) -> Vec<u64> {
        let mut sectors: Vec<u64> = self
            .changes
            .iter()
            .flat_map(|c| {
                let first = c.offset / sector_size;
                let last = (c.offset + c.after.len().max(1) - 1) / sector_size;
                first..=last
            })
            .map(|s| s as u64)
            .collect();
        sectors.sort_unstable();
        sectors.dedup();
        sectors
    }
}
```

`crates/fs-core/src/disk.rs`:
```rust
//! The virtual disk: a flat byte buffer addressed by sector. Every write goes
//! through `write`/`fill` so it can be journaled into the open operation.

use crate::trace::{ByteChange, Event, OpRecord};
use crate::{Error, Result};

pub struct Disk {
    bytes: Vec<u8>,
    sector_size: usize,
    open_op: Option<OpRecord>,
}

impl Disk {
    /// A zero-filled disk.
    pub fn new(sector_size: usize, sector_count: u64) -> Disk {
        assert!(sector_size > 0, "sector size must be non-zero");
        Disk { bytes: vec![0; sector_size * sector_count as usize], sector_size, open_op: None }
    }

    /// Wrap an existing image. The length must be a whole number of sectors.
    pub fn from_bytes(sector_size: usize, bytes: Vec<u8>) -> Result<Disk> {
        if sector_size == 0 || bytes.len() % sector_size != 0 {
            return Err(Error::InvalidGeometry(format!(
                "image length {} is not a multiple of sector size {}",
                bytes.len(),
                sector_size
            )));
        }
        Ok(Disk { bytes, sector_size, open_op: None })
    }

    pub fn sector_size(&self) -> usize {
        self.sector_size
    }

    pub fn sector_count(&self) -> u64 {
        (self.bytes.len() / self.sector_size) as u64
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The whole image.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn sector(&self, n: u64) -> &[u8] {
        let start = n as usize * self.sector_size;
        &self.bytes[start..start + self.sector_size]
    }

    pub fn read(&self, offset: usize, len: usize) -> &[u8] {
        &self.bytes[offset..offset + len]
    }

    /// Write bytes at an absolute offset. Recorded if an operation is open.
    /// Writing past the end of the disk is a programmer error and panics.
    pub fn write(&mut self, offset: usize, data: &[u8]) {
        let end = offset + data.len();
        if let Some(op) = self.open_op.as_mut() {
            op.changes.push(ByteChange {
                offset,
                before: self.bytes[offset..end].to_vec(),
                after: data.to_vec(),
            });
        }
        self.bytes[offset..end].copy_from_slice(data);
    }

    pub fn fill(&mut self, offset: usize, len: usize, byte: u8) {
        let data = vec![byte; len];
        self.write(offset, &data);
    }

    /// Start recording. Panics if an operation is already open.
    pub fn begin_op(&mut self, op: impl Into<String>) {
        assert!(self.open_op.is_none(), "an operation is already open");
        self.open_op = Some(OpRecord::new(op));
    }

    /// Attach an event to the open operation. Dropped if none is open.
    pub fn event(&mut self, event: Box<dyn Event>) {
        if let Some(op) = self.open_op.as_mut() {
            op.events.push(event);
        }
    }

    /// Stop recording and return the record. Panics if no operation is open.
    pub fn end_op(&mut self) -> OpRecord {
        self.open_op.take().expect("no operation is open")
    }

    pub fn op_open(&self) -> bool {
        self.open_op.is_some()
    }
}
```

Add to `crates/fs-core/src/lib.rs`:
```rust
pub mod disk;
pub mod trace;

pub use disk::Disk;
pub use trace::{ByteChange, Event, OpRecord};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fs-core`
Expected: all pass (15 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/fs-core
git commit -m "feat(fs-core): virtual disk with per-operation change journal"
```

---

### Task 3: fs-core layout types and FileSystem trait

**Files:**
- Create: `crates/fs-core/src/layout.rs`, `crates/fs-core/src/fs.rs`
- Modify: `crates/fs-core/src/lib.rs`

**Interfaces:**
- Produces: `Region { name, sectors: Range<u64>, kind }`, `RegionKind`, `Annotation { range: Range<usize>, label, value }`, and the `FileSystem` trait exactly as in the spec.

- [ ] **Step 1: Write a failing test proving the trait is object-safe**

`crates/fs-core/src/fs.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DateTime, Disk, EntryInfo, Error, OpRecord, Result};

    struct NullFs {
        disk: Disk,
    }

    impl FileSystem for NullFs {
        fn fs_type(&self) -> &'static str {
            "null"
        }
        fn create_file(&mut self, _: &str, _: &[u8]) -> Result<OpRecord> {
            Err(Error::Unsupported("null".into()))
        }
        fn write_file(&mut self, _: &str, _: &[u8]) -> Result<OpRecord> {
            Err(Error::Unsupported("null".into()))
        }
        fn read_file(&self, _: &str) -> Result<Vec<u8>> {
            Err(Error::NotFound)
        }
        fn delete_file(&mut self, _: &str) -> Result<OpRecord> {
            Err(Error::NotFound)
        }
        fn create_dir(&mut self, _: &str) -> Result<OpRecord> {
            Err(Error::Unsupported("null".into()))
        }
        fn remove_dir(&mut self, _: &str) -> Result<OpRecord> {
            Err(Error::NotFound)
        }
        fn list_dir(&self, _: &str) -> Result<Vec<EntryInfo>> {
            Ok(Vec::new())
        }
        fn stat(&self, _: &str) -> Result<EntryInfo> {
            Err(Error::NotFound)
        }
        fn set_now(&mut self, _: DateTime) {}
        fn disk(&self) -> &Disk {
            &self.disk
        }
        fn layout(&self) -> Vec<Region> {
            vec![Region { name: "all".into(), sectors: 0..1, kind: RegionKind::Other }]
        }
        fn annotate_sector(&self, _: u64) -> Vec<Annotation> {
            Vec::new()
        }
        fn history(&self) -> &[OpRecord] {
            &[]
        }
    }

    #[test]
    fn trait_is_object_safe() {
        let fs: Box<dyn FileSystem> = Box::new(NullFs { disk: Disk::new(512, 1) });
        assert_eq!(fs.fs_type(), "null");
        assert_eq!(fs.layout()[0].kind, RegionKind::Other);
        assert_eq!(fs.list_dir("/").unwrap(), Vec::new());
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fs-core fs::`
Expected: compile error, `FileSystem` not found.

- [ ] **Step 3: Implement layout and the trait**

`crates/fs-core/src/layout.rs`:
```rust
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
```

`crates/fs-core/src/fs.rs`:
```rust
//! The interface a UI programs against. Constructors (`format`, `from_image`)
//! live on the concrete filesystem types because their options differ.

use crate::layout::{Annotation, Region};
use crate::{DateTime, Disk, EntryInfo, OpRecord, Result};

pub trait FileSystem {
    /// `"FAT16"`, later `"FAT32"`, `"ext2"`.
    fn fs_type(&self) -> &'static str;

    /// `AlreadyExists` if the path is present.
    fn create_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord>;
    /// Overwrite an existing file; `NotFound` if absent.
    fn write_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord>;
    fn read_file(&self, path: &str) -> Result<Vec<u8>>;
    fn delete_file(&mut self, path: &str) -> Result<OpRecord>;
    fn create_dir(&mut self, path: &str) -> Result<OpRecord>;
    /// `DirectoryNotEmpty` unless empty.
    fn remove_dir(&mut self, path: &str) -> Result<OpRecord>;
    /// Excludes `.` and `..`.
    fn list_dir(&self, path: &str) -> Result<Vec<EntryInfo>>;
    fn stat(&self, path: &str) -> Result<EntryInfo>;
    /// The timestamp subsequent operations stamp onto entries.
    fn set_now(&mut self, now: DateTime);

    fn disk(&self) -> &Disk;
    fn layout(&self) -> Vec<Region>;
    fn annotate_sector(&self, sector: u64) -> Vec<Annotation>;
    fn history(&self) -> &[OpRecord];
}
```

Add to `crates/fs-core/src/lib.rs`:
```rust
pub mod fs;
pub mod layout;

pub use fs::FileSystem;
pub use layout::{Annotation, Region, RegionKind};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fs-core && cargo clippy -p fs-core --all-targets`
Expected: all pass, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/fs-core
git commit -m "feat(fs-core): layout types and FileSystem trait"
```

---

### Task 4: fat boot sector, FormatOptions, Geometry

**Files:**
- Create: `crates/fat/src/boot_sector.rs`
- Modify: `crates/fat/src/lib.rs`

**Interfaces:**
- Consumes: `fs_core::{Error, Result}`
- Produces: `FormatOptions` (Default), `FatVariant::Fat16`, `BootSector::{from_options, parse, to_bytes, total_sectors, geometry}`, `Geometry` with fields `variant, bytes_per_sector: usize, sectors_per_cluster: usize, reserved_sectors: u64, fat_count: u8, sectors_per_fat: u64, root_entries: usize, root_dir_sectors: u64, first_root_dir_sector: u64, first_data_sector: u64, total_sectors: u64, cluster_count: u32` and methods `cluster_size()`, `fat_offset(fat)`, `fat_len()`, `fat_entry_offset(fat, cluster)`, `root_dir_offset()`, `root_dir_len()`, `cluster_offset(cluster)`, `cluster_range(cluster)`, `max_cluster()`, `is_valid_cluster(c)`, `cluster_of_sector(sector) -> Option<u32>`.

- [ ] **Step 1: Write failing tests**

`crates/fat/src/boot_sector.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fs_core::Error;

    #[test]
    fn defaults_produce_a_fat16_geometry() {
        let bs = BootSector::from_options(&FormatOptions::default()).unwrap();
        assert_eq!(bs.sectors_per_fat, 32);
        assert_eq!(bs.total_sectors_16, 32768);
        assert_eq!(bs.total_sectors_32, 0);
        assert_eq!(&bs.fs_type, b"FAT16   ");
        let g = bs.geometry().unwrap();
        assert_eq!(g.variant, FatVariant::Fat16);
        assert_eq!(g.cluster_count, 8167);
        assert_eq!(g.root_dir_sectors, 32);
        assert_eq!(g.first_root_dir_sector, 65);
        assert_eq!(g.first_data_sector, 97);
        assert_eq!(g.cluster_size(), 2048);
        assert_eq!(g.fat_offset(0), 512);
        assert_eq!(g.fat_offset(1), 512 + 32 * 512);
        assert_eq!(g.fat_entry_offset(1, 5), 512 + 32 * 512 + 10);
        assert_eq!(g.root_dir_offset(), 65 * 512);
        assert_eq!(g.root_dir_len(), 512 * 32);
        assert_eq!(g.cluster_offset(2), 97 * 512);
        assert_eq!(g.cluster_offset(3), 97 * 512 + 2048);
        assert_eq!(g.max_cluster(), 8168);
        assert!(g.is_valid_cluster(2) && g.is_valid_cluster(8168));
        assert!(!g.is_valid_cluster(1) && !g.is_valid_cluster(8169));
        assert_eq!(g.cluster_of_sector(97), Some(2));
        assert_eq!(g.cluster_of_sector(101), Some(3));
        assert_eq!(g.cluster_of_sector(96), None);
    }

    #[test]
    fn to_bytes_and_parse_round_trip() {
        let bs = BootSector::from_options(&FormatOptions::default()).unwrap();
        let bytes = bs.to_bytes();
        assert_eq!(bytes.len(), 512);
        assert_eq!(&bytes[0..3], &[0xEB, 0x3C, 0x90]);
        assert_eq!(&bytes[3..11], b"FAT16EMU");
        assert_eq!(&bytes[510..512], &[0x55, 0xAA]);
        assert_eq!(BootSector::parse(&bytes).unwrap(), bs);
    }

    #[test]
    fn parse_rejects_bad_signature() {
        let mut bytes = BootSector::from_options(&FormatOptions::default()).unwrap().to_bytes();
        bytes[511] = 0;
        assert!(matches!(BootSector::parse(&bytes), Err(Error::CorruptImage(_))));
    }

    #[test]
    fn small_volume_is_rejected_unless_range_check_is_off() {
        let opts = FormatOptions { total_sectors: 2048, ..Default::default() };
        assert!(matches!(BootSector::from_options(&opts), Err(Error::InvalidGeometry(_))));
        let opts = FormatOptions { enforce_fat16_range: false, ..opts };
        let g = BootSector::from_options(&opts).unwrap().geometry().unwrap();
        assert!(g.cluster_count < FAT16_MIN_CLUSTERS);
        assert_eq!(g.variant, FatVariant::Fat16);
    }

    #[test]
    fn geometry_refuses_fat12_and_fat32_images() {
        let mut bs = BootSector::from_options(&FormatOptions {
            total_sectors: 2048,
            enforce_fat16_range: false,
            ..Default::default()
        })
        .unwrap();
        bs.fs_type = *b"FAT12   ";
        assert!(matches!(bs.geometry(), Err(Error::Unsupported(_))));
        let mut bs = BootSector::from_options(&FormatOptions::default()).unwrap();
        bs.sectors_per_fat = 0;
        assert!(matches!(bs.geometry(), Err(Error::Unsupported(_))));
    }

    #[test]
    fn invalid_options_are_rejected() {
        let bad = |o: FormatOptions| matches!(BootSector::from_options(&o), Err(Error::InvalidGeometry(_)));
        assert!(bad(FormatOptions { bytes_per_sector: 500, ..Default::default() }));
        assert!(bad(FormatOptions { sectors_per_cluster: 3, ..Default::default() }));
        assert!(bad(FormatOptions { fat_count: 0, ..Default::default() }));
        assert!(bad(FormatOptions { root_entries: 0, ..Default::default() }));
        assert!(bad(FormatOptions { reserved_sectors: 0, ..Default::default() }));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p fat boot_sector`
Expected: compile error.

- [ ] **Step 3: Implement**

`crates/fat/src/boot_sector.rs`:
```rust
//! The boot sector (BPB + EBPB) and the geometry derived from it.

use fs_core::{Error, Result};
use std::ops::Range;

pub const SIGNATURE: [u8; 2] = [0x55, 0xAA];
pub const OEM_NAME: [u8; 8] = *b"FAT16EMU";
/// Fewer clusters than this is FAT12 to a real driver.
pub const FAT16_MIN_CLUSTERS: u32 = 4085;
/// More clusters than this is FAT32.
pub const FAT16_MAX_CLUSTERS: u32 = 65524;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatOptions {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub total_sectors: u32,
    pub fat_count: u8,
    pub root_entries: u16,
    pub reserved_sectors: u16,
    pub volume_label: [u8; 11],
    pub volume_id: u32,
    /// Reject geometries whose cluster count is not in the FAT16 range.
    pub enforce_fat16_range: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            bytes_per_sector: 512,
            sectors_per_cluster: 4,
            total_sectors: 32768,
            fat_count: 2,
            root_entries: 512,
            reserved_sectors: 1,
            volume_label: *b"NO NAME    ",
            volume_id: 0x1234_5678,
            enforce_fat16_range: true,
        }
    }
}

/// Which FAT flavour a volume uses. Only FAT16 is implemented; FAT32 will be
/// added here and dispatched on wherever the two differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatVariant {
    Fat16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootSector {
    pub oem_name: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub fat_count: u8,
    pub root_entries: u16,
    pub total_sectors_16: u16,
    pub media: u8,
    pub sectors_per_fat: u16,
    pub sectors_per_track: u16,
    pub heads: u16,
    pub hidden_sectors: u32,
    pub total_sectors_32: u32,
    pub drive_number: u8,
    pub boot_signature: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub fs_type: [u8; 8],
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

fn root_dir_sectors(root_entries: u64, bytes_per_sector: u64) -> u64 {
    (root_entries * 32).div_ceil(bytes_per_sector)
}

/// Smallest FAT size (in sectors) that has an entry for every cluster plus
/// the two reserved entries. Grows one sector at a time; each extra FAT
/// sector removes clusters, so this converges quickly.
fn sectors_per_fat(opts: &FormatOptions) -> u16 {
    let bps = opts.bytes_per_sector as u64;
    let spc = opts.sectors_per_cluster as u64;
    let total = opts.total_sectors as u64;
    let fixed = opts.reserved_sectors as u64 + root_dir_sectors(opts.root_entries as u64, bps);
    let mut spf: u64 = 1;
    loop {
        let data_sectors = total.saturating_sub(fixed + opts.fat_count as u64 * spf);
        let entries_needed = data_sectors / spc + 2;
        if spf * bps / 2 >= entries_needed {
            return spf as u16;
        }
        spf += 1;
    }
}

impl BootSector {
    pub fn from_options(opts: &FormatOptions) -> Result<BootSector> {
        let geo_err = Error::InvalidGeometry;
        if ![512, 1024, 2048, 4096].contains(&opts.bytes_per_sector) {
            return Err(geo_err(format!("bytes_per_sector {} must be 512, 1024, 2048 or 4096", opts.bytes_per_sector)));
        }
        if !opts.sectors_per_cluster.is_power_of_two() || opts.sectors_per_cluster > 128 {
            return Err(geo_err(format!("sectors_per_cluster {} must be a power of two up to 128", opts.sectors_per_cluster)));
        }
        if opts.fat_count == 0 {
            return Err(geo_err("fat_count must be at least 1".into()));
        }
        if opts.root_entries == 0 || (opts.root_entries as u32 * 32) % opts.bytes_per_sector as u32 != 0 {
            return Err(geo_err(format!("root_entries {} must be non-zero and fill whole sectors", opts.root_entries)));
        }
        if opts.reserved_sectors == 0 {
            return Err(geo_err("reserved_sectors must be at least 1".into()));
        }
        let spf = sectors_per_fat(opts);
        let (total_16, total_32) = if opts.total_sectors < 0x1_0000 {
            (opts.total_sectors as u16, 0)
        } else {
            (0, opts.total_sectors)
        };
        let bs = BootSector {
            oem_name: OEM_NAME,
            bytes_per_sector: opts.bytes_per_sector,
            sectors_per_cluster: opts.sectors_per_cluster,
            reserved_sectors: opts.reserved_sectors,
            fat_count: opts.fat_count,
            root_entries: opts.root_entries,
            total_sectors_16: total_16,
            media: 0xF8,
            sectors_per_fat: spf,
            sectors_per_track: 63,
            heads: 16,
            hidden_sectors: 0,
            total_sectors_32: total_32,
            drive_number: 0x80,
            boot_signature: 0x29,
            volume_id: opts.volume_id,
            volume_label: opts.volume_label,
            fs_type: *b"FAT16   ",
        };
        let geo = bs.geometry()?;
        if opts.enforce_fat16_range
            && !(FAT16_MIN_CLUSTERS..=FAT16_MAX_CLUSTERS).contains(&geo.cluster_count)
        {
            return Err(geo_err(format!(
                "{} clusters is outside the FAT16 range {}..={}; adjust total_sectors or sectors_per_cluster, or set enforce_fat16_range = false",
                geo.cluster_count, FAT16_MIN_CLUSTERS, FAT16_MAX_CLUSTERS
            )));
        }
        Ok(bs)
    }

    /// Parse the first 512 bytes of a volume.
    pub fn parse(bytes: &[u8]) -> Result<BootSector> {
        if bytes.len() < 512 {
            return Err(Error::CorruptImage("boot sector is shorter than 512 bytes".into()));
        }
        if bytes[510..512] != SIGNATURE {
            return Err(Error::CorruptImage("boot sector signature is not 55 AA".into()));
        }
        let mut oem_name = [0u8; 8];
        oem_name.copy_from_slice(&bytes[3..11]);
        let mut volume_label = [0u8; 11];
        volume_label.copy_from_slice(&bytes[43..54]);
        let mut fs_type = [0u8; 8];
        fs_type.copy_from_slice(&bytes[54..62]);
        let bs = BootSector {
            oem_name,
            bytes_per_sector: u16_at(bytes, 11),
            sectors_per_cluster: bytes[13],
            reserved_sectors: u16_at(bytes, 14),
            fat_count: bytes[16],
            root_entries: u16_at(bytes, 17),
            total_sectors_16: u16_at(bytes, 19),
            media: bytes[21],
            sectors_per_fat: u16_at(bytes, 22),
            sectors_per_track: u16_at(bytes, 24),
            heads: u16_at(bytes, 26),
            hidden_sectors: u32_at(bytes, 28),
            total_sectors_32: u32_at(bytes, 32),
            drive_number: bytes[36],
            boot_signature: bytes[38],
            volume_id: u32_at(bytes, 39),
            volume_label,
            fs_type,
        };
        if ![512, 1024, 2048, 4096].contains(&bs.bytes_per_sector) {
            return Err(Error::CorruptImage(format!("bytes per sector is {}", bs.bytes_per_sector)));
        }
        if bs.sectors_per_cluster == 0 || !bs.sectors_per_cluster.is_power_of_two() {
            return Err(Error::CorruptImage(format!("sectors per cluster is {}", bs.sectors_per_cluster)));
        }
        if bs.fat_count == 0 || bs.reserved_sectors == 0 || bs.total_sectors() == 0 {
            return Err(Error::CorruptImage("zero FAT count, reserved sectors, or total sectors".into()));
        }
        Ok(bs)
    }

    /// Serialize to one full sector (`bytes_per_sector` bytes).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = vec![0u8; self.bytes_per_sector as usize];
        b[0..3].copy_from_slice(&[0xEB, 0x3C, 0x90]);
        b[3..11].copy_from_slice(&self.oem_name);
        b[11..13].copy_from_slice(&self.bytes_per_sector.to_le_bytes());
        b[13] = self.sectors_per_cluster;
        b[14..16].copy_from_slice(&self.reserved_sectors.to_le_bytes());
        b[16] = self.fat_count;
        b[17..19].copy_from_slice(&self.root_entries.to_le_bytes());
        b[19..21].copy_from_slice(&self.total_sectors_16.to_le_bytes());
        b[21] = self.media;
        b[22..24].copy_from_slice(&self.sectors_per_fat.to_le_bytes());
        b[24..26].copy_from_slice(&self.sectors_per_track.to_le_bytes());
        b[26..28].copy_from_slice(&self.heads.to_le_bytes());
        b[28..32].copy_from_slice(&self.hidden_sectors.to_le_bytes());
        b[32..36].copy_from_slice(&self.total_sectors_32.to_le_bytes());
        b[36] = self.drive_number;
        b[37] = 0;
        b[38] = self.boot_signature;
        b[39..43].copy_from_slice(&self.volume_id.to_le_bytes());
        b[43..54].copy_from_slice(&self.volume_label);
        b[54..62].copy_from_slice(&self.fs_type);
        b[510..512].copy_from_slice(&SIGNATURE);
        b
    }

    pub fn total_sectors(&self) -> u32 {
        if self.total_sectors_16 != 0 {
            self.total_sectors_16 as u32
        } else {
            self.total_sectors_32
        }
    }

    pub fn geometry(&self) -> Result<Geometry> {
        if self.sectors_per_fat == 0 {
            return Err(Error::Unsupported("FAT32 volumes (sectors_per_fat = 0) are not supported yet".into()));
        }
        let bps = self.bytes_per_sector as u64;
        let root_dir_sectors = root_dir_sectors(self.root_entries as u64, bps);
        let first_root_dir_sector = self.reserved_sectors as u64 + self.fat_count as u64 * self.sectors_per_fat as u64;
        let first_data_sector = first_root_dir_sector + root_dir_sectors;
        let total_sectors = self.total_sectors() as u64;
        if first_data_sector >= total_sectors {
            return Err(Error::InvalidGeometry(format!(
                "metadata needs {first_data_sector} sectors but the volume has only {total_sectors}"
            )));
        }
        let cluster_count = ((total_sectors - first_data_sector) / self.sectors_per_cluster as u64) as u32;
        if cluster_count == 0 {
            return Err(Error::InvalidGeometry("volume has no data clusters".into()));
        }
        if cluster_count > FAT16_MAX_CLUSTERS {
            return Err(Error::Unsupported(format!("{cluster_count} clusters means FAT32, which is not supported yet")));
        }
        if cluster_count < FAT16_MIN_CLUSTERS && &self.fs_type != b"FAT16   " {
            return Err(Error::Unsupported(format!("{cluster_count} clusters means FAT12, which is not supported")));
        }
        Ok(Geometry {
            variant: FatVariant::Fat16,
            bytes_per_sector: bps as usize,
            sectors_per_cluster: self.sectors_per_cluster as usize,
            reserved_sectors: self.reserved_sectors as u64,
            fat_count: self.fat_count,
            sectors_per_fat: self.sectors_per_fat as u64,
            root_entries: self.root_entries as usize,
            root_dir_sectors,
            first_root_dir_sector,
            first_data_sector,
            total_sectors,
            cluster_count,
        })
    }
}

/// Everything derived from the BPB that the other layers need to find bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Geometry {
    pub variant: FatVariant,
    pub bytes_per_sector: usize,
    pub sectors_per_cluster: usize,
    pub reserved_sectors: u64,
    pub fat_count: u8,
    pub sectors_per_fat: u64,
    pub root_entries: usize,
    pub root_dir_sectors: u64,
    pub first_root_dir_sector: u64,
    pub first_data_sector: u64,
    pub total_sectors: u64,
    /// Number of data clusters. Valid cluster numbers are `2..=cluster_count + 1`.
    pub cluster_count: u32,
}

impl Geometry {
    pub fn cluster_size(&self) -> usize {
        self.bytes_per_sector * self.sectors_per_cluster
    }

    pub fn fat_offset(&self, fat: u8) -> usize {
        (self.reserved_sectors + fat as u64 * self.sectors_per_fat) as usize * self.bytes_per_sector
    }

    pub fn fat_len(&self) -> usize {
        self.sectors_per_fat as usize * self.bytes_per_sector
    }

    pub fn fat_entry_offset(&self, fat: u8, cluster: u32) -> usize {
        match self.variant {
            FatVariant::Fat16 => self.fat_offset(fat) + cluster as usize * 2,
        }
    }

    pub fn root_dir_offset(&self) -> usize {
        self.first_root_dir_sector as usize * self.bytes_per_sector
    }

    pub fn root_dir_len(&self) -> usize {
        self.root_entries * 32
    }

    /// Absolute byte offset of a data cluster. Panics for clusters below 2.
    pub fn cluster_offset(&self, cluster: u32) -> usize {
        assert!(cluster >= 2, "cluster {cluster} has no data");
        self.first_data_sector as usize * self.bytes_per_sector + (cluster as usize - 2) * self.cluster_size()
    }

    pub fn cluster_range(&self, cluster: u32) -> Range<usize> {
        let start = self.cluster_offset(cluster);
        start..start + self.cluster_size()
    }

    pub fn max_cluster(&self) -> u32 {
        self.cluster_count + 1
    }

    pub fn is_valid_cluster(&self, cluster: u32) -> bool {
        (2..=self.max_cluster()).contains(&cluster)
    }

    /// Which cluster a data sector belongs to, if it is inside a cluster.
    pub fn cluster_of_sector(&self, sector: u64) -> Option<u32> {
        if sector < self.first_data_sector {
            return None;
        }
        let cluster = 2 + ((sector - self.first_data_sector) / self.sectors_per_cluster as u64) as u32;
        self.is_valid_cluster(cluster).then_some(cluster)
    }
}
```

`crates/fat/src/lib.rs`:
```rust
//! Byte-accurate FAT16 filesystem on an in-memory disk.

pub mod boot_sector;

pub use boot_sector::{BootSector, FatVariant, FormatOptions, Geometry};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p fat boot_sector`
Expected: 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/fat
git commit -m "feat(fat): boot sector, format options, and geometry"
```

---

### Task 5: fat directory entries, timestamps, LFN checksum

**Files:**
- Create: `crates/fat/src/dir_entry.rs`
- Modify: `crates/fat/src/lib.rs`

**Interfaces:**
- Consumes: `fs_core::DateTime`
- Produces: `attr::{READ_ONLY, HIDDEN, SYSTEM, VOLUME_ID, DIRECTORY, ARCHIVE, LFN}`, `attr::is_lfn(u8)`, `ENTRY_SIZE`, `FREE`, `DELETED`, `pack_date`, `pack_time`, `unpack(date, time) -> Option<DateTime>`, `ShortEntry` (fields as spec) with `new(name, attributes, &DateTime)`, `parse(&[u8])`, `to_bytes() -> [u8; 32]`, `first_cluster()`, `set_first_cluster(u32)`, `is_dir()`, `is_volume_label()`, `is_dot_entry()`, `display_name()`; `LfnEntry` with `new(order, last, checksum, &[u16; 13])`, `parse`, `to_bytes`, `chars() -> [u16; 13]`, `order()`, `is_last()`; `lfn_checksum(&[u8; 11]) -> u8`.

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fs_core::DateTime;

    #[test]
    fn date_and_time_pack_to_known_values() {
        let dt = DateTime::new(2026, 9, 21, 12, 34, 56);
        assert_eq!(pack_date(&dt), 0x5D35);
        assert_eq!(pack_time(&dt), 0x645C);
        assert_eq!(unpack(0x5D35, 0x645C), Some(dt));
    }

    #[test]
    fn odd_seconds_round_down_and_zero_date_is_none() {
        let dt = DateTime::new(2026, 9, 21, 12, 34, 57);
        assert_eq!(unpack(pack_date(&dt), pack_time(&dt)).unwrap().second, 56);
        assert_eq!(unpack(0, 0), None);
    }

    #[test]
    fn checksum_matches_hand_computed_value() {
        assert_eq!(lfn_checksum(b"A          "), 0x80);
        assert_eq!(lfn_checksum(&[0; 11]), 0);
    }

    #[test]
    fn short_entry_round_trips_and_exposes_fields() {
        let now = DateTime::new(2026, 9, 21, 12, 34, 56);
        let mut e = ShortEntry::new(*b"README  TXT", attr::ARCHIVE, &now);
        e.set_first_cluster(0x0001_0005);
        e.size = 1234;
        let bytes = e.to_bytes();
        assert_eq!(&bytes[0..11], b"README  TXT");
        assert_eq!(bytes[11], attr::ARCHIVE);
        assert_eq!(&bytes[20..22], &0x0001u16.to_le_bytes());
        assert_eq!(&bytes[26..28], &0x0005u16.to_le_bytes());
        assert_eq!(&bytes[28..32], &1234u32.to_le_bytes());
        let parsed = ShortEntry::parse(&bytes);
        assert_eq!(parsed, e);
        assert_eq!(parsed.first_cluster(), 0x0001_0005);
        assert_eq!(parsed.display_name(), "README.TXT");
        assert!(!parsed.is_dir());
        assert_eq!(unpack(parsed.create_date, parsed.create_time), Some(now));
        assert_eq!(unpack(parsed.access_date, 0), Some(DateTime::new(2026, 9, 21, 0, 0, 0)));
    }

    #[test]
    fn display_name_without_extension_and_dot_entries() {
        let e = ShortEntry::new(*b"FOO        ", attr::DIRECTORY, &DateTime::default());
        assert_eq!(e.display_name(), "FOO");
        assert!(e.is_dir());
        assert!(ShortEntry::new(*b".          ", attr::DIRECTORY, &DateTime::default()).is_dot_entry());
        assert!(ShortEntry::new(*b"..         ", attr::DIRECTORY, &DateTime::default()).is_dot_entry());
        assert!(!e.is_dot_entry());
        assert!(ShortEntry::new(*b"LABEL      ", attr::VOLUME_ID, &DateTime::default()).is_volume_label());
    }

    #[test]
    fn lfn_entry_round_trips() {
        let chars: [u16; 13] = [b'M' as u16, b'y' as u16, 0, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF];
        let e = LfnEntry::new(1, true, 0x80, &chars);
        assert_eq!(e.sequence, 0x41);
        assert!(e.is_last());
        assert_eq!(e.order(), 1);
        let bytes = e.to_bytes();
        assert_eq!(bytes[0], 0x41);
        assert_eq!(bytes[11], attr::LFN);
        assert_eq!(bytes[13], 0x80);
        assert_eq!(&bytes[1..3], &[b'M', 0]);
        assert_eq!(&bytes[26..28], &[0, 0]);
        assert!(attr::is_lfn(bytes[11]));
        let parsed = LfnEntry::parse(&bytes);
        assert_eq!(parsed, e);
        assert_eq!(parsed.chars(), chars);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p fat dir_entry`
Expected: compile error.

- [ ] **Step 3: Implement**

`crates/fat/src/dir_entry.rs`:
```rust
//! The 32-byte directory entry in its two forms: the classic short (8.3)
//! entry and the VFAT long-file-name entry that precedes it.

use fs_core::DateTime;

pub const ENTRY_SIZE: usize = 32;
/// First byte of a never-used slot; also terminates directory scans.
pub const FREE: u8 = 0x00;
/// First byte of a deleted slot.
pub const DELETED: u8 = 0xE5;

pub mod attr {
    pub const READ_ONLY: u8 = 0x01;
    pub const HIDDEN: u8 = 0x02;
    pub const SYSTEM: u8 = 0x04;
    pub const VOLUME_ID: u8 = 0x08;
    pub const DIRECTORY: u8 = 0x10;
    pub const ARCHIVE: u8 = 0x20;
    /// READ_ONLY | HIDDEN | SYSTEM | VOLUME_ID marks a long-name entry.
    pub const LFN: u8 = 0x0F;

    pub fn is_lfn(attributes: u8) -> bool {
        attributes & 0x3F == LFN
    }
}

/// FAT date: bits 15..9 year since 1980, 8..5 month, 4..0 day.
pub fn pack_date(dt: &DateTime) -> u16 {
    let year = dt.year.saturating_sub(1980).min(127);
    (year << 9) | ((dt.month as u16 & 0x0F) << 5) | (dt.day as u16 & 0x1F)
}

/// FAT time: bits 15..11 hour, 10..5 minute, 4..0 seconds / 2.
pub fn pack_time(dt: &DateTime) -> u16 {
    ((dt.hour as u16) << 11) | ((dt.minute as u16 & 0x3F) << 5) | (dt.second as u16 / 2)
}

/// `None` when the date is zero, which FAT uses for "not set".
pub fn unpack(date: u16, time: u16) -> Option<DateTime> {
    if date == 0 {
        return None;
    }
    Some(DateTime::new(
        1980 + (date >> 9),
        ((date >> 5) & 0x0F) as u8,
        (date & 0x1F) as u8,
        (time >> 11) as u8,
        ((time >> 5) & 0x3F) as u8,
        ((time & 0x1F) * 2) as u8,
    ))
}

/// The checksum of a short name that every LFN entry for it carries.
pub fn lfn_checksum(name: &[u8; 11]) -> u8 {
    name.iter().fold(0u8, |sum, &b| ((sum & 1) << 7).wrapping_add(sum >> 1).wrapping_add(b))
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortEntry {
    pub name: [u8; 11],
    pub attr: u8,
    pub nt_reserved: u8,
    pub create_time_tenths: u8,
    pub create_time: u16,
    pub create_date: u16,
    pub access_date: u16,
    pub first_cluster_hi: u16,
    pub write_time: u16,
    pub write_date: u16,
    pub first_cluster_lo: u16,
    pub size: u32,
}

impl ShortEntry {
    /// A fresh entry with all three timestamps set from `now`, no cluster, size 0.
    pub fn new(name: [u8; 11], attributes: u8, now: &DateTime) -> ShortEntry {
        ShortEntry {
            name,
            attr: attributes,
            nt_reserved: 0,
            create_time_tenths: 0,
            create_time: pack_time(now),
            create_date: pack_date(now),
            access_date: pack_date(now),
            first_cluster_hi: 0,
            write_time: pack_time(now),
            write_date: pack_date(now),
            first_cluster_lo: 0,
            size: 0,
        }
    }

    pub fn parse(b: &[u8]) -> ShortEntry {
        let mut name = [0u8; 11];
        name.copy_from_slice(&b[0..11]);
        ShortEntry {
            name,
            attr: b[11],
            nt_reserved: b[12],
            create_time_tenths: b[13],
            create_time: u16_at(b, 14),
            create_date: u16_at(b, 16),
            access_date: u16_at(b, 18),
            first_cluster_hi: u16_at(b, 20),
            write_time: u16_at(b, 22),
            write_date: u16_at(b, 24),
            first_cluster_lo: u16_at(b, 26),
            size: u32::from_le_bytes([b[28], b[29], b[30], b[31]]),
        }
    }

    pub fn to_bytes(&self) -> [u8; ENTRY_SIZE] {
        let mut b = [0u8; ENTRY_SIZE];
        b[0..11].copy_from_slice(&self.name);
        b[11] = self.attr;
        b[12] = self.nt_reserved;
        b[13] = self.create_time_tenths;
        b[14..16].copy_from_slice(&self.create_time.to_le_bytes());
        b[16..18].copy_from_slice(&self.create_date.to_le_bytes());
        b[18..20].copy_from_slice(&self.access_date.to_le_bytes());
        b[20..22].copy_from_slice(&self.first_cluster_hi.to_le_bytes());
        b[22..24].copy_from_slice(&self.write_time.to_le_bytes());
        b[24..26].copy_from_slice(&self.write_date.to_le_bytes());
        b[26..28].copy_from_slice(&self.first_cluster_lo.to_le_bytes());
        b[28..32].copy_from_slice(&self.size.to_le_bytes());
        b
    }

    /// The high half is always zero on FAT16 but is kept for FAT32.
    pub fn first_cluster(&self) -> u32 {
        ((self.first_cluster_hi as u32) << 16) | self.first_cluster_lo as u32
    }

    pub fn set_first_cluster(&mut self, cluster: u32) {
        self.first_cluster_hi = (cluster >> 16) as u16;
        self.first_cluster_lo = cluster as u16;
    }

    pub fn is_dir(&self) -> bool {
        self.attr & attr::DIRECTORY != 0
    }

    pub fn is_volume_label(&self) -> bool {
        self.attr & attr::VOLUME_ID != 0 && !attr::is_lfn(self.attr)
    }

    pub fn is_dot_entry(&self) -> bool {
        self.name == *b".          " || self.name == *b"..         "
    }

    /// `NAME.EXT`, or `NAME` when the extension is blank.
    pub fn display_name(&self) -> String {
        let base = String::from_utf8_lossy(&self.name[..8]).trim_end().to_string();
        let ext = String::from_utf8_lossy(&self.name[8..]).trim_end().to_string();
        if ext.is_empty() {
            base
        } else {
            format!("{base}.{ext}")
        }
    }
}

/// One VFAT long-name entry holding 13 UCS-2 characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LfnEntry {
    /// Order (1-based, bits 0..5) with bit 6 set on the last (first on disk) entry.
    pub sequence: u8,
    pub name1: [u16; 5],
    pub attr: u8,
    pub entry_type: u8,
    pub checksum: u8,
    pub name2: [u16; 6],
    pub first_cluster_lo: u16,
    pub name3: [u16; 2],
}

pub const LAST_LFN_FLAG: u8 = 0x40;

impl LfnEntry {
    pub fn new(order: u8, last: bool, checksum: u8, chars: &[u16; 13]) -> LfnEntry {
        let mut name1 = [0u16; 5];
        name1.copy_from_slice(&chars[0..5]);
        let mut name2 = [0u16; 6];
        name2.copy_from_slice(&chars[5..11]);
        let mut name3 = [0u16; 2];
        name3.copy_from_slice(&chars[11..13]);
        LfnEntry {
            sequence: order | if last { LAST_LFN_FLAG } else { 0 },
            name1,
            attr: attr::LFN,
            entry_type: 0,
            checksum,
            name2,
            first_cluster_lo: 0,
            name3,
        }
    }

    pub fn parse(b: &[u8]) -> LfnEntry {
        let read = |start: usize, out: &mut [u16]| {
            for (i, slot) in out.iter_mut().enumerate() {
                *slot = u16_at(b, start + i * 2);
            }
        };
        let mut name1 = [0u16; 5];
        read(1, &mut name1);
        let mut name2 = [0u16; 6];
        read(14, &mut name2);
        let mut name3 = [0u16; 2];
        read(28, &mut name3);
        LfnEntry {
            sequence: b[0],
            name1,
            attr: b[11],
            entry_type: b[12],
            checksum: b[13],
            name2,
            first_cluster_lo: u16_at(b, 26),
            name3,
        }
    }

    pub fn to_bytes(&self) -> [u8; ENTRY_SIZE] {
        let mut b = [0u8; ENTRY_SIZE];
        b[0] = self.sequence;
        let write = |start: usize, b: &mut [u8], chars: &[u16]| {
            for (i, c) in chars.iter().enumerate() {
                b[start + i * 2..start + i * 2 + 2].copy_from_slice(&c.to_le_bytes());
            }
        };
        write(1, &mut b, &self.name1);
        b[11] = self.attr;
        b[12] = self.entry_type;
        b[13] = self.checksum;
        write(14, &mut b, &self.name2);
        b[26..28].copy_from_slice(&self.first_cluster_lo.to_le_bytes());
        write(28, &mut b, &self.name3);
        b
    }

    /// All 13 characters in name order.
    pub fn chars(&self) -> [u16; 13] {
        let mut out = [0u16; 13];
        out[0..5].copy_from_slice(&self.name1);
        out[5..11].copy_from_slice(&self.name2);
        out[11..13].copy_from_slice(&self.name3);
        out
    }

    pub fn order(&self) -> u8 {
        self.sequence & 0x1F
    }

    pub fn is_last(&self) -> bool {
        self.sequence & LAST_LFN_FLAG != 0
    }
}
```

Add to `crates/fat/src/lib.rs`:
```rust
pub mod dir_entry;

pub use dir_entry::{LfnEntry, ShortEntry};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p fat dir_entry`
Expected: 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/fat
git commit -m "feat(fat): short and long directory entries with timestamp packing"
```

---

### Task 6: fat name handling (8.3, short-name generation, UCS-2, LFN chunks)

**Files:**
- Create: `crates/fat/src/name.rs`
- Modify: `crates/fat/src/lib.rs`

**Interfaces:**
- Consumes: `fs_core::{Error, Result}`
- Produces: `is_valid_short_name(&str) -> bool`, `to_short_name_bytes(&str) -> Option<[u8; 11]>`, `validate_long_name(&str) -> Result<()>`, `generate_short_name(&str, &dyn Fn(&[u8; 11]) -> bool) -> Result<[u8; 11]>`, `to_ucs2(&str) -> Vec<u16>`, `from_ucs2(&[u16]) -> String`, `lfn_chunks(&str) -> Vec<[u16; 13]>`, `MAX_LONG_NAME`, `LFN_CHARS_PER_ENTRY`.

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fs_core::Error;

    #[test]
    fn valid_short_names() {
        for n in ["README.TXT", "readme.txt", "FOO", "A", "FILE~1.TXT", "12345678.123"] {
            assert!(is_valid_short_name(n), "{n} should be valid");
        }
    }

    #[test]
    fn invalid_short_names_need_lfn() {
        for n in ["My File.txt", "toolongname.txt", "a.b.c", "FOO.", "", ".HIDDEN", "FILE.TOOLONG", "über.txt", "a+b"] {
            assert!(!is_valid_short_name(n), "{n} should be invalid");
        }
    }

    #[test]
    fn short_name_bytes_are_uppercased_and_padded() {
        assert_eq!(to_short_name_bytes("readme.txt"), Some(*b"README  TXT"));
        assert_eq!(to_short_name_bytes("foo"), Some(*b"FOO        "));
    }

    #[test]
    fn generated_short_names() {
        let none = |_: &[u8; 11]| false;
        assert_eq!(generate_short_name("My File.txt", &none).unwrap(), *b"MYFILE~1TXT");
        assert_eq!(generate_short_name("a.b.c.txt", &none).unwrap(), *b"ABC~1   TXT");
        assert_eq!(generate_short_name(".bashrc", &none).unwrap(), *b"BASHRC~1   ");
        assert_eq!(generate_short_name("verylongfilename.document", &none).unwrap(), *b"VERYLO~1DOC");
        assert_eq!(generate_short_name("x", &none).unwrap(), *b"X~1        ");
        assert_eq!(generate_short_name("über.txt", &none).unwrap(), *b"_BER~1  TXT");
    }

    #[test]
    fn tilde_number_skips_taken_names() {
        let taken = |n: &[u8; 11]| n == b"MYFILE~1TXT" || n == b"MYFILE~2TXT";
        assert_eq!(generate_short_name("My File.txt", &taken).unwrap(), *b"MYFILE~3TXT");
        let many = |n: &[u8; 11]| n[6] == b'~'; // every single-digit tail is taken
        assert_eq!(generate_short_name("My File.txt", &many).unwrap(), *b"MYFIL~10TXT");
    }

    #[test]
    fn long_name_validation() {
        assert_eq!(validate_long_name("My File.txt"), Ok(()));
        assert_eq!(validate_long_name(""), Err(Error::InvalidName));
        assert_eq!(validate_long_name(&"a".repeat(256)), Err(Error::InvalidName));
        assert_eq!(validate_long_name("bad*name"), Err(Error::InvalidName));
        assert_eq!(validate_long_name(" lead"), Err(Error::InvalidName));
        assert_eq!(validate_long_name("."), Err(Error::InvalidName));
    }

    #[test]
    fn lfn_chunks_terminate_and_pad() {
        let chunks = lfn_chunks("My File.txt");
        assert_eq!(chunks.len(), 1);
        assert_eq!(&chunks[0][..11], &to_ucs2("My File.txt")[..]);
        assert_eq!(chunks[0][11], 0);
        assert_eq!(chunks[0][12], 0xFFFF);
        assert_eq!(lfn_chunks(&"A".repeat(13)).len(), 1);
        assert_eq!(lfn_chunks(&"A".repeat(13))[0][12], b'A' as u16);
        assert_eq!(lfn_chunks(&"A".repeat(14)).len(), 2);
        assert_eq!(lfn_chunks(&"A".repeat(14))[1][1], 0);
    }

    #[test]
    fn ucs2_round_trip_strips_padding() {
        let mut units = to_ucs2("Héllo");
        units.extend([0, 0xFFFF, 0xFFFF]);
        assert_eq!(from_ucs2(&units), "Héllo");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p fat name::`
Expected: compile error.

- [ ] **Step 3: Implement**

`crates/fat/src/name.rs`:
```rust
//! Name rules: which names fit a short (8.3) entry, how to derive a short
//! alias for a long name, and UCS-2 chunking for long-name entries.

use fs_core::{Error, Result};

pub const MAX_LONG_NAME: usize = 255;
pub const LFN_CHARS_PER_ENTRY: usize = 13;

const SHORT_INVALID: &[u8] = b"\"*+,/:;<=>?\\[]|";
const LONG_INVALID: &[char] = &['"', '*', '/', ':', '<', '>', '?', '\\', '|'];

fn is_short_char(c: u8) -> bool {
    c > 0x20 && c < 0x7F && c != b'.' && !SHORT_INVALID.contains(&c)
}

/// True when the name, uppercased, fits a short entry without an LFN.
pub fn is_valid_short_name(name: &str) -> bool {
    to_short_name_bytes(name).is_some()
}

/// The padded, uppercased 11-byte form, or `None` if the name needs an LFN.
pub fn to_short_name_bytes(name: &str) -> Option<[u8; 11]> {
    if name.is_empty() || !name.is_ascii() || name.matches('.').count() > 1 {
        return None;
    }
    let (base, ext) = match name.rfind('.') {
        Some(i) => (&name[..i], &name[i + 1..]),
        None => (name, ""),
    };
    if base.is_empty() || base.len() > 8 || ext.len() > 3 || (name.contains('.') && ext.is_empty()) {
        return None;
    }
    if !base.bytes().chain(ext.bytes()).all(is_short_char) {
        return None;
    }
    let mut out = [b' '; 11];
    for (i, b) in base.bytes().enumerate() {
        out[i] = b.to_ascii_uppercase();
    }
    for (i, b) in ext.bytes().enumerate() {
        out[8 + i] = b.to_ascii_uppercase();
    }
    Some(out)
}

/// Rules for any name the caller passes in: non-empty, at most 255 UCS-2
/// units, no reserved characters, no leading/trailing spaces, not all dots.
pub fn validate_long_name(name: &str) -> Result<()> {
    if name.is_empty() || name.encode_utf16().count() > MAX_LONG_NAME {
        return Err(Error::InvalidName);
    }
    if name.chars().any(|c| LONG_INVALID.contains(&c) || (c as u32) < 0x20) {
        return Err(Error::InvalidName);
    }
    if name.trim() != name || name.chars().all(|c| c == '.') {
        return Err(Error::InvalidName);
    }
    Ok(())
}

/// Windows-style alias: cleaned, uppercased base truncated to make room for
/// `~N`, where N is the smallest number whose result `taken` does not report.
pub fn generate_short_name(long: &str, taken: &dyn Fn(&[u8; 11]) -> bool) -> Result<[u8; 11]> {
    let (base_src, ext_src) = match long.rfind('.') {
        Some(i) if i > 0 => (&long[..i], &long[i + 1..]),
        _ => (long, ""),
    };
    let clean = |s: &str| -> Vec<u8> {
        s.chars()
            .filter(|&c| c != ' ' && c != '.')
            .map(|c| {
                if c.is_ascii() && is_short_char(c as u8) {
                    (c as u8).to_ascii_uppercase()
                } else {
                    b'_'
                }
            })
            .collect()
    };
    let base = clean(base_src);
    let ext: Vec<u8> = clean(ext_src).into_iter().take(3).collect();
    for n in 1..=999_999u32 {
        let tail = format!("~{n}");
        let keep = 8 - tail.len();
        let mut out = [b' '; 11];
        let mut i = 0;
        for &b in base.iter().take(keep) {
            out[i] = b;
            i += 1;
        }
        for &b in tail.as_bytes() {
            out[i] = b;
            i += 1;
        }
        for (j, &b) in ext.iter().enumerate() {
            out[8 + j] = b;
        }
        if !taken(&out) {
            return Ok(out);
        }
    }
    Err(Error::DirectoryFull)
}

pub fn to_ucs2(name: &str) -> Vec<u16> {
    name.encode_utf16().collect()
}

/// Decode up to the first `0x0000`, ignoring `0xFFFF` padding.
pub fn from_ucs2(units: &[u16]) -> String {
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    let kept: Vec<u16> = units[..end].iter().copied().filter(|&u| u != 0xFFFF).collect();
    String::from_utf16_lossy(&kept)
}

/// Split a long name into 13-unit chunks: a `0x0000` terminator follows the
/// name unless it exactly fills the last chunk, and `0xFFFF` pads the rest.
pub fn lfn_chunks(name: &str) -> Vec<[u16; 13]> {
    let mut units = to_ucs2(name);
    if units.len() % LFN_CHARS_PER_ENTRY != 0 {
        units.push(0);
    }
    while units.len() % LFN_CHARS_PER_ENTRY != 0 {
        units.push(0xFFFF);
    }
    units
        .chunks(LFN_CHARS_PER_ENTRY)
        .map(|c| {
            let mut a = [0u16; 13];
            a.copy_from_slice(c);
            a
        })
        .collect()
}
```

Add `pub mod name;` to `crates/fat/src/lib.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p fat name::`
Expected: 8 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/fat
git commit -m "feat(fat): 8.3 validation, short-name generation, and LFN chunking"
```

---

### Task 7: FatEntry type and FatEvent

**Files:**
- Create: `crates/fat/src/table.rs` (entry type, codec, Display only; operations come in Task 8), `crates/fat/src/events.rs`
- Modify: `crates/fat/src/lib.rs`

**Interfaces:**
- Consumes: `fs_core::Event`
- Produces: `FatEntry::{Free, Next(u32), EndOfChain, Bad, Reserved}` (Copy, Display), `table::decode16(u16) -> FatEntry`, `table::encode16(FatEntry) -> u16`; `EntryKind::{Short, Lfn}`; `FatEvent` variants `ClusterAllocated { cluster, range }`, `ClusterFreed { cluster, range }`, `FatEntrySet { fat, cluster, value, range }`, `DirEntryWritten { dir, slot, kind, range }`, `DirEntryDeleted { dir, slot, range }`, `DataWritten { cluster, bytes, range }`, `DirectoryGrown { dir, cluster, range }`, all with `range: Range<usize>`.

- [ ] **Step 1: Write failing tests**

`crates/fat/src/table.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

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
        for e in [FatEntry::Free, FatEntry::Next(77), FatEntry::EndOfChain, FatEntry::Bad, FatEntry::Reserved] {
            assert_eq!(decode16(encode16(e)), e);
        }
        assert_eq!(encode16(FatEntry::EndOfChain), 0xFFFF);
    }

    #[test]
    fn display() {
        assert_eq!(FatEntry::Next(9).to_string(), "next -> 9");
        assert_eq!(FatEntry::EndOfChain.to_string(), "end of chain");
    }
}
```

`crates/fat/src/events.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::FatEntry;
    use fs_core::Event;

    #[test]
    fn events_have_kinds_regions_and_text() {
        let e = FatEvent::FatEntrySet { fat: 1, cluster: 5, value: FatEntry::EndOfChain, range: 100..102 };
        assert_eq!(e.kind(), "fat_entry_set");
        assert_eq!(e.region(), Some(100..102));
        assert_eq!(e.to_string(), "FAT 1: entry for cluster 5 set to end of chain");
        let e = FatEvent::DirEntryWritten { dir: "/DOCS".into(), slot: 3, kind: EntryKind::Lfn, range: 0..32 };
        assert_eq!(e.to_string(), "wrote long-name entry in /DOCS slot 3");
        let boxed: Box<dyn Event> = Box::new(FatEvent::ClusterAllocated { cluster: 2, range: 0..4 });
        assert_eq!(boxed.clone().kind(), "cluster_allocated");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p fat table events`
Expected: compile error.

- [ ] **Step 3: Implement**

`crates/fat/src/table.rs` (initial content; Task 8 appends operations):
```rust
//! The file allocation table: one entry per cluster saying whether it is free,
//! which cluster follows it, or that it ends a chain.

use std::fmt;

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
```

`crates/fat/src/events.rs`:
```rust
//! Semantic events a FAT volume reports while it works, for the change log.

use crate::table::FatEntry;
use fs_core::Event;
use std::fmt;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Short,
    Lfn,
}

/// Every variant carries the absolute byte range it concerns so the UI can
/// highlight it without knowing the geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FatEvent {
    ClusterAllocated { cluster: u32, range: Range<usize> },
    ClusterFreed { cluster: u32, range: Range<usize> },
    FatEntrySet { fat: u8, cluster: u32, value: FatEntry, range: Range<usize> },
    DirEntryWritten { dir: String, slot: usize, kind: EntryKind, range: Range<usize> },
    DirEntryDeleted { dir: String, slot: usize, range: Range<usize> },
    DataWritten { cluster: u32, bytes: usize, range: Range<usize> },
    DirectoryGrown { dir: String, cluster: u32, range: Range<usize> },
}

impl fmt::Display for FatEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FatEvent::ClusterAllocated { cluster, .. } => write!(f, "allocated cluster {cluster}"),
            FatEvent::ClusterFreed { cluster, .. } => write!(f, "freed cluster {cluster}"),
            FatEvent::FatEntrySet { fat, cluster, value, .. } => {
                write!(f, "FAT {fat}: entry for cluster {cluster} set to {value}")
            }
            FatEvent::DirEntryWritten { dir, slot, kind, .. } => {
                let kind = match kind {
                    EntryKind::Short => "short",
                    EntryKind::Lfn => "long-name",
                };
                write!(f, "wrote {kind} entry in {dir} slot {slot}")
            }
            FatEvent::DirEntryDeleted { dir, slot, .. } => write!(f, "marked {dir} slot {slot} deleted"),
            FatEvent::DataWritten { cluster, bytes, .. } => write!(f, "wrote {bytes} bytes of data to cluster {cluster}"),
            FatEvent::DirectoryGrown { dir, cluster, .. } => write!(f, "grew directory {dir} with cluster {cluster}"),
        }
    }
}

impl Event for FatEvent {
    fn kind(&self) -> &'static str {
        match self {
            FatEvent::ClusterAllocated { .. } => "cluster_allocated",
            FatEvent::ClusterFreed { .. } => "cluster_freed",
            FatEvent::FatEntrySet { .. } => "fat_entry_set",
            FatEvent::DirEntryWritten { .. } => "dir_entry_written",
            FatEvent::DirEntryDeleted { .. } => "dir_entry_deleted",
            FatEvent::DataWritten { .. } => "data_written",
            FatEvent::DirectoryGrown { .. } => "directory_grown",
        }
    }

    fn region(&self) -> Option<Range<usize>> {
        let r = match self {
            FatEvent::ClusterAllocated { range, .. }
            | FatEvent::ClusterFreed { range, .. }
            | FatEvent::FatEntrySet { range, .. }
            | FatEvent::DirEntryWritten { range, .. }
            | FatEvent::DirEntryDeleted { range, .. }
            | FatEvent::DataWritten { range, .. }
            | FatEvent::DirectoryGrown { range, .. } => range,
        };
        Some(r.clone())
    }

    fn clone_box(&self) -> Box<dyn Event> {
        Box::new(self.clone())
    }
}
```

Add to `crates/fat/src/lib.rs`:
```rust
pub mod events;
pub mod table;

pub use events::{EntryKind, FatEvent};
pub use table::FatEntry;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p fat`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/fat
git commit -m "feat(fat): FAT entry codec and FatEvent change-log events"
```

---

### Task 8: FAT table operations

**Files:**
- Modify: `crates/fat/src/table.rs`

**Interfaces:**
- Consumes: `Disk`, `Geometry`, `FatEvent`, `FatEntry`
- Produces: `read_entry_from(&Disk, &Geometry, fat: u8, cluster: u32) -> FatEntry`, `read_entry(&Disk, &Geometry, cluster) -> FatEntry` (FAT 0), `write_entry(&mut Disk, &Geometry, cluster, FatEntry)` (mirrors to every FAT, emits `FatEntrySet` per FAT), `count_free(&Disk, &Geometry) -> u32`, `allocate_chain(&mut Disk, &Geometry, count: u32) -> Result<Vec<u32>>` (atomic: `DiskFull` before any write; emits `ClusterAllocated`), `chain(&Disk, &Geometry, start) -> Result<Vec<u32>>` (`CorruptImage` on invalid cluster, loop, or Free/Bad/Reserved mid-chain), `free_chain(&mut Disk, &Geometry, start) -> Result<()>` (emits `ClusterFreed`).

- [ ] **Step 1: Write failing tests (append to the tests module in table.rs)**

```rust
    use crate::boot_sector::{BootSector, FormatOptions};
    use fs_core::{Disk, Error};

    fn tiny() -> (Disk, Geometry) {
        let opts = FormatOptions { total_sectors: 64, sectors_per_cluster: 1, root_entries: 16, enforce_fat16_range: false, ..Default::default() };
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
        assert_eq!(allocate_chain(&mut disk, &geo, 0).unwrap(), Vec::<u32>::new());
        let kinds = disk.end_op().event_kinds();
        assert_eq!(kinds.iter().filter(|k| **k == "cluster_allocated").count(), 4);
    }

    #[test]
    fn allocate_is_atomic_when_disk_is_full() {
        let (mut disk, geo) = tiny();
        let free = count_free(&disk, &geo);
        assert_eq!(free, geo.cluster_count);
        assert_eq!(allocate_chain(&mut disk, &geo, free + 1), Err(Error::DiskFull));
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
        assert_eq!(disk.end_op().event_kinds().iter().filter(|k| **k == "cluster_freed").count(), 3);
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
        assert!(matches!(chain(&disk, &geo, geo.max_cluster() + 1), Err(Error::CorruptImage(_))));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p fat table::`
Expected: compile error (`write_entry` not found).

- [ ] **Step 3: Implement (append to table.rs above the tests module)**

```rust
use crate::boot_sector::Geometry;
use crate::events::FatEvent;
use fs_core::{Disk, Error, Result};

pub fn read_entry_from(disk: &Disk, geo: &Geometry, fat: u8, cluster: u32) -> FatEntry {
    let off = geo.fat_entry_offset(fat, cluster);
    let b = disk.read(off, 2);
    decode16(u16::from_le_bytes([b[0], b[1]]))
}

/// Read from the first FAT, which is the one drivers trust.
pub fn read_entry(disk: &Disk, geo: &Geometry, cluster: u32) -> FatEntry {
    read_entry_from(disk, geo, 0, cluster)
}

/// Write the entry into every FAT copy.
pub fn write_entry(disk: &mut Disk, geo: &Geometry, cluster: u32, value: FatEntry) {
    let raw = encode16(value).to_le_bytes();
    for fat in 0..geo.fat_count {
        let off = geo.fat_entry_offset(fat, cluster);
        disk.write(off, &raw);
        disk.event(Box::new(FatEvent::FatEntrySet { fat, cluster, value, range: off..off + 2 }));
    }
}

pub fn count_free(disk: &Disk, geo: &Geometry) -> u32 {
    (2..=geo.max_cluster()).filter(|&c| read_entry(disk, geo, c) == FatEntry::Free).count() as u32
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
        disk.event(Box::new(FatEvent::ClusterAllocated { cluster, range: geo.cluster_range(cluster) }));
    }
    Ok(free)
}

/// Every cluster in the chain starting at `start`, in order.
pub fn chain(disk: &Disk, geo: &Geometry, start: u32) -> Result<Vec<u32>> {
    let mut out = Vec::new();
    let mut current = start;
    loop {
        if !geo.is_valid_cluster(current) {
            return Err(Error::CorruptImage(format!("cluster chain points to invalid cluster {current}")));
        }
        if out.len() as u32 > geo.cluster_count {
            return Err(Error::CorruptImage(format!("cluster chain starting at {start} loops")));
        }
        out.push(current);
        match read_entry(disk, geo, current) {
            FatEntry::Next(next) => current = next,
            FatEntry::EndOfChain => return Ok(out),
            other => {
                return Err(Error::CorruptImage(format!("cluster {current} inside a chain is marked {other}")));
            }
        }
    }
}

pub fn free_chain(disk: &mut Disk, geo: &Geometry, start: u32) -> Result<()> {
    for cluster in chain(disk, geo, start)? {
        write_entry(disk, geo, cluster, FatEntry::Free);
        disk.event(Box::new(FatEvent::ClusterFreed { cluster, range: geo.cluster_range(cluster) }));
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p fat table::`
Expected: 8 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/fat
git commit -m "feat(fat): FAT allocation, chain following, and freeing"
```

---

### Task 9: Directory slots

**Files:**
- Create: `crates/fat/src/dir.rs`
- Modify: `crates/fat/src/lib.rs`

**Interfaces:**
- Consumes: `table::{chain, allocate_chain, write_entry}`, `FatEntry`, `FatEvent`, `dir_entry::{ENTRY_SIZE, FREE, DELETED}`
- Produces: `DirLocation::{Root, Cluster(u32)}` with `DirLocation::from_cluster(u32)` (0 maps to Root), `Slot { index: usize, offset: usize }`, `slots(&Disk, &Geometry, DirLocation) -> Result<Vec<Slot>>`, `find_free_run(&Disk, &Geometry, DirLocation, count: usize) -> Result<Option<usize>>` (index of first slot), `grow(&mut Disk, &Geometry, DirLocation, dir_name: &str) -> Result<()>` (`DirectoryFull` for Root).

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot_sector::{BootSector, FormatOptions};
    use crate::table;
    use fs_core::{Disk, Error};

    fn tiny() -> (Disk, Geometry) {
        let opts = FormatOptions { total_sectors: 64, sectors_per_cluster: 1, root_entries: 16, enforce_fat16_range: false, ..Default::default() };
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
        assert_eq!(s[0], Slot { index: 0, offset: geo.root_dir_offset() });
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
        assert_eq!(find_free_run(&disk, &geo, DirLocation::Root, 1).unwrap(), Some(1));
        assert_eq!(find_free_run(&disk, &geo, DirLocation::Root, 2).unwrap(), Some(3));
        assert_eq!(find_free_run(&disk, &geo, DirLocation::Root, 13).unwrap(), Some(3));
        assert_eq!(find_free_run(&disk, &geo, DirLocation::Root, 14).unwrap(), None);
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
        assert!(disk.read(geo.cluster_offset(chain[1]), 512).iter().all(|&b| b == 0));
        assert!(rec.event_kinds().contains(&"directory_grown"));
        assert_eq!(grow(&mut disk, &geo, DirLocation::Root, "/"), Err(Error::DirectoryFull));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p fat dir::`
Expected: compile error.

- [ ] **Step 3: Implement**

`crates/fat/src/dir.rs`:
```rust
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
            .map(|i| Slot { index: i, offset: geo.root_dir_offset() + i * ENTRY_SIZE })
            .collect()),
        DirLocation::Cluster(start) => {
            let per_cluster = geo.cluster_size() / ENTRY_SIZE;
            let mut out = Vec::new();
            for cluster in table::chain(disk, geo, start)? {
                let base = geo.cluster_offset(cluster);
                for i in 0..per_cluster {
                    out.push(Slot { index: out.len(), offset: base + i * ENTRY_SIZE });
                }
            }
            Ok(out)
        }
    }
}

/// Index of the first slot of a run of `count` consecutive free or deleted
/// slots, if one exists.
pub fn find_free_run(disk: &Disk, geo: &Geometry, loc: DirLocation, count: usize) -> Result<Option<usize>> {
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
    disk.event(Box::new(FatEvent::DirectoryGrown { dir: dir_name.to_string(), cluster: new, range: geo.cluster_range(new) }));
    Ok(())
}
```

Add `pub mod dir;` to `crates/fat/src/lib.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p fat dir::`
Expected: 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/fat
git commit -m "feat(fat): directory slot iteration, free-run search, and growth"
```

---

### Task 10: FatFs format, from_image, layout, and basics

**Files:**
- Create: `crates/fat/src/fs.rs`
- Modify: `crates/fat/src/lib.rs`

**Interfaces:**
- Consumes: `BootSector`, `Geometry`, `FormatOptions`, `Disk`, `Region`, `RegionKind`, `OpRecord`, `DateTime`
- Produces: `FatFs` with `format(FormatOptions) -> Result<FatFs>`, `from_image(Vec<u8>) -> Result<FatFs>`, `boot_sector() -> &BootSector`, `geometry() -> &Geometry`, `fs_type() -> &'static str`, `disk() -> &Disk`, `set_now(DateTime)`, `now() -> DateTime`, `history() -> &[OpRecord]`, `layout() -> Vec<Region>`. These are inherent methods; the trait impl in Task 14 delegates to them. Private fields: `disk, boot, geo, now, history`.

- [ ] **Step 1: Write failing tests**

`crates/fat/src/fs.rs` tests module:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::table;
    use fs_core::{Error, RegionKind};

    #[test]
    fn format_writes_boot_sector_and_fat_headers() {
        let fs = FatFs::format(FormatOptions::default()).unwrap();
        assert_eq!(fs.fs_type(), "FAT16");
        assert_eq!(fs.disk().len(), 32768 * 512);
        assert_eq!(BootSector::parse(fs.disk().sector(0)).unwrap(), *fs.boot_sector());
        for fat in 0..2 {
            let off = fs.geometry().fat_offset(fat);
            assert_eq!(fs.disk().read(off, 4), &[0xF8, 0xFF, 0xFF, 0xFF]);
        }
        assert_eq!(table::count_free(fs.disk(), fs.geometry()), fs.geometry().cluster_count);
        assert!(fs.history().is_empty());
        assert_eq!(fs.now(), DateTime::default());
    }

    #[test]
    fn layout_regions_are_contiguous_and_cover_the_disk() {
        let fs = FatFs::format(FormatOptions::default()).unwrap();
        let regions = fs.layout();
        let names: Vec<&str> = regions.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["reserved (boot sector)", "FAT 0", "FAT 1", "root directory", "data"]);
        assert_eq!(regions[0].kind, RegionKind::Boot);
        assert_eq!(regions[1].kind, RegionKind::AllocationTable);
        assert_eq!(regions[3].kind, RegionKind::Directory);
        assert_eq!(regions[4].kind, RegionKind::Data);
        let mut next = 0;
        for r in &regions {
            assert_eq!(r.sectors.start, next);
            next = r.sectors.end;
        }
        assert_eq!(next, 32768);
    }

    #[test]
    fn from_image_round_trips_and_validates() {
        let fs = FatFs::format(FormatOptions::default()).unwrap();
        let image = fs.disk().as_bytes().to_vec();
        let again = FatFs::from_image(image.clone()).unwrap();
        assert_eq!(again.geometry(), fs.geometry());
        assert_eq!(again.disk().as_bytes(), fs.disk().as_bytes());

        let mut bad = image.clone();
        bad[510] = 0;
        assert!(matches!(FatFs::from_image(bad), Err(Error::CorruptImage(_))));
        assert!(matches!(FatFs::from_image(vec![0; 100]), Err(Error::CorruptImage(_))));
        let short = image[..image.len() - 512].to_vec();
        assert!(matches!(FatFs::from_image(short), Err(Error::CorruptImage(_))));
    }

    #[test]
    fn set_now_is_stored() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let t = DateTime::new(2026, 9, 21, 1, 2, 3);
        fs.set_now(t);
        assert_eq!(fs.now(), t);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p fat fs::`
Expected: compile error.

- [ ] **Step 3: Implement**

`crates/fat/src/fs.rs`:
```rust
//! `FatFs`: a FAT16 volume on an in-memory disk. The disk bytes are the only
//! state; every method re-reads what it needs from them.

use crate::boot_sector::{BootSector, FormatOptions, Geometry};
use fs_core::{DateTime, Disk, Error, OpRecord, Region, RegionKind, Result};

pub struct FatFs {
    disk: Disk,
    boot: BootSector,
    geo: Geometry,
    now: DateTime,
    history: Vec<OpRecord>,
}

impl FatFs {
    /// A freshly formatted, empty volume.
    pub fn format(opts: FormatOptions) -> Result<FatFs> {
        let boot = BootSector::from_options(&opts)?;
        let geo = boot.geometry()?;
        let mut disk = Disk::new(geo.bytes_per_sector, geo.total_sectors);
        disk.write(0, &boot.to_bytes());
        for fat in 0..geo.fat_count {
            // Entry 0 holds the media descriptor (0xFFF8); entry 1 is reserved (0xFFFF).
            disk.write(geo.fat_offset(fat), &[0xF8, 0xFF, 0xFF, 0xFF]);
        }
        Ok(FatFs { disk, boot, geo, now: DateTime::default(), history: Vec::new() })
    }

    /// Wrap an existing FAT16 image. Extra trailing bytes are dropped; a
    /// shorter image than the boot sector describes is rejected.
    pub fn from_image(mut bytes: Vec<u8>) -> Result<FatFs> {
        if bytes.len() < 512 {
            return Err(Error::CorruptImage("image is shorter than one sector".into()));
        }
        let boot = BootSector::parse(&bytes[..512])?;
        let geo = boot.geometry()?;
        let expected = geo.total_sectors as usize * geo.bytes_per_sector;
        if bytes.len() < expected {
            return Err(Error::CorruptImage(format!(
                "image is {} bytes but the boot sector describes {expected}",
                bytes.len()
            )));
        }
        bytes.truncate(expected);
        let disk = Disk::from_bytes(geo.bytes_per_sector, bytes)?;
        Ok(FatFs { disk, boot, geo, now: DateTime::default(), history: Vec::new() })
    }

    pub fn boot_sector(&self) -> &BootSector {
        &self.boot
    }

    pub fn geometry(&self) -> &Geometry {
        &self.geo
    }

    pub fn fs_type(&self) -> &'static str {
        "FAT16"
    }

    pub fn disk(&self) -> &Disk {
        &self.disk
    }

    pub fn set_now(&mut self, now: DateTime) {
        self.now = now;
    }

    pub fn now(&self) -> DateTime {
        self.now
    }

    pub fn history(&self) -> &[OpRecord] {
        &self.history
    }

    pub fn layout(&self) -> Vec<Region> {
        let g = &self.geo;
        let mut regions = vec![Region {
            name: "reserved (boot sector)".into(),
            sectors: 0..g.reserved_sectors,
            kind: RegionKind::Boot,
        }];
        for fat in 0..g.fat_count {
            let start = g.reserved_sectors + fat as u64 * g.sectors_per_fat;
            regions.push(Region {
                name: format!("FAT {fat}"),
                sectors: start..start + g.sectors_per_fat,
                kind: RegionKind::AllocationTable,
            });
        }
        regions.push(Region {
            name: "root directory".into(),
            sectors: g.first_root_dir_sector..g.first_data_sector,
            kind: RegionKind::Directory,
        });
        regions.push(Region {
            name: "data".into(),
            sectors: g.first_data_sector..g.total_sectors,
            kind: RegionKind::Data,
        });
        regions
    }

    /// Run `body` inside a recorded operation. On success the record is
    /// appended to history and returned; on failure it is discarded.
    fn run_op(&mut self, op: String, body: impl FnOnce(&mut Self) -> Result<()>) -> Result<OpRecord> {
        self.disk.begin_op(op);
        let result = body(self);
        let record = self.disk.end_op();
        result?;
        self.history.push(record.clone());
        Ok(record)
    }

    /// `/` for the root, otherwise `/A/B` from components.
    fn dir_display_name(parts: &[String]) -> String {
        if parts.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", parts.join("/"))
        }
    }
}
```

`run_op` and `dir_display_name` are unused until Task 12; add `#[allow(dead_code)]` on each for now and remove it in Task 12.

Add to `crates/fat/src/lib.rs`:
```rust
pub mod fs;

pub use fs::FatFs;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p fat fs::`
Expected: 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/fat
git commit -m "feat(fat): FatFs format, image import, and layout"
```

---

### Task 11: Directory reading: list_dir, stat, read_file

**Files:**
- Modify: `crates/fat/src/fs.rs`

**Interfaces:**
- Consumes: `dir::{slots, DirLocation}`, `dir_entry::{ShortEntry, LfnEntry, attr, lfn_checksum, unpack, ENTRY_SIZE, FREE, DELETED}`, `name::from_ucs2`, `path::parse`, `table::chain`
- Produces: `pub struct Located { pub dir: DirLocation, pub slot: usize, pub lfn_slots: Vec<usize>, pub entry: ShortEntry, pub long_name: Option<String> }` with `name()` and `info() -> EntryInfo`; private `scan_dir(DirLocation) -> Result<Vec<Located>>`, `find_in_dir(DirLocation, &str) -> Result<Option<Located>>`, `resolve_dir(&[String]) -> Result<DirLocation>`, `resolve(&str) -> Result<Option<Located>>` (None for root, `NotFound` if missing), `read_chain_data(first, size) -> Result<Vec<u8>>`; public `list_dir(&str) -> Result<Vec<EntryInfo>>`, `stat(&str) -> Result<EntryInfo>`, `read_file(&str) -> Result<Vec<u8>>`.

- [ ] **Step 1: Write failing tests (append to the tests module in fs.rs)**

These tests hand-write entries into the disk so the reader is tested independently of the writer.

```rust
    use crate::dir_entry::{attr, lfn_checksum, LfnEntry, ShortEntry, ENTRY_SIZE, DELETED};
    use crate::name;

    fn write_root_slot(fs: &mut FatFs, slot: usize, bytes: &[u8; 32]) {
        let off = fs.geo.root_dir_offset() + slot * ENTRY_SIZE;
        fs.disk.write(off, bytes);
    }

    fn short(name: &[u8; 11], attributes: u8) -> ShortEntry {
        ShortEntry::new(*name, attributes, &DateTime::new(2026, 9, 21, 10, 0, 0))
    }

    #[test]
    fn empty_root_lists_nothing_and_stat_root_works() {
        let fs = FatFs::format(FormatOptions::default()).unwrap();
        assert_eq!(fs.list_dir("/").unwrap(), Vec::new());
        let root = fs.stat("/").unwrap();
        assert_eq!(root.name, "/");
        assert!(root.is_dir);
        assert_eq!(fs.stat("/NOPE"), Err(Error::NotFound));
        assert_eq!(fs.list_dir("/NOPE"), Err(Error::NotFound));
        assert_eq!(fs.list_dir("relative"), Err(Error::InvalidPath));
    }

    #[test]
    fn short_entries_are_listed_with_display_names_and_timestamps() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let mut e = short(b"README  TXT", attr::ARCHIVE);
        e.size = 5;
        write_root_slot(&mut fs, 0, &e.to_bytes());
        write_root_slot(&mut fs, 1, &short(b"LABEL      ", attr::VOLUME_ID).to_bytes());
        write_root_slot(&mut fs, 2, &short(b"DOCS       ", attr::DIRECTORY).to_bytes());
        let list = fs.list_dir("/").unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "README.TXT");
        assert_eq!(list[0].size, 5);
        assert!(!list[0].is_dir);
        assert_eq!(list[0].modified, Some(DateTime::new(2026, 9, 21, 10, 0, 0)));
        assert_eq!(list[0].accessed, Some(DateTime::new(2026, 9, 21, 0, 0, 0)));
        assert_eq!(list[1].name, "DOCS");
        assert!(list[1].is_dir);
        assert_eq!(fs.stat("/readme.txt").unwrap().name, "README.TXT");
    }

    #[test]
    fn long_names_are_reassembled_when_valid() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let short_name = *b"MYFILE~1TXT";
        let sum = lfn_checksum(&short_name);
        let chunks = name::lfn_chunks("My File.txt");
        write_root_slot(&mut fs, 0, &LfnEntry::new(1, true, sum, &chunks[0]).to_bytes());
        write_root_slot(&mut fs, 1, &short(&short_name, attr::ARCHIVE).to_bytes());
        // A two-entry name: 14 A's, written highest order first.
        let long = "A".repeat(14);
        let short2 = *b"AAAAAA~1   ";
        let sum2 = lfn_checksum(&short2);
        let chunks2 = name::lfn_chunks(&long);
        write_root_slot(&mut fs, 2, &LfnEntry::new(2, true, sum2, &chunks2[1]).to_bytes());
        write_root_slot(&mut fs, 3, &LfnEntry::new(1, false, sum2, &chunks2[0]).to_bytes());
        write_root_slot(&mut fs, 4, &short(&short2, attr::ARCHIVE).to_bytes());
        let list = fs.list_dir("/").unwrap();
        assert_eq!(list[0].name, "My File.txt");
        assert_eq!(list[1].name, long);
        assert_eq!(fs.stat("/my file.TXT").unwrap().name, "My File.txt");
        assert_eq!(fs.stat("/MYFILE~1.TXT").unwrap().name, "My File.txt");
    }

    #[test]
    fn orphaned_or_mismatched_lfn_entries_are_ignored() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let chunks = name::lfn_chunks("Wrong.txt");
        // Checksum does not match the short name that follows.
        write_root_slot(&mut fs, 0, &LfnEntry::new(1, true, 0x11, &chunks[0]).to_bytes());
        write_root_slot(&mut fs, 1, &short(b"REAL    TXT", attr::ARCHIVE).to_bytes());
        // A deleted LFN entry followed by a short entry.
        let mut deleted = LfnEntry::new(1, true, lfn_checksum(b"OTHER   TXT"), &chunks[0]).to_bytes();
        deleted[0] = DELETED;
        write_root_slot(&mut fs, 2, &deleted);
        write_root_slot(&mut fs, 3, &short(b"OTHER   TXT", attr::ARCHIVE).to_bytes());
        let names: Vec<String> = fs.list_dir("/").unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["REAL.TXT", "OTHER.TXT"]);
    }

    #[test]
    fn read_file_follows_the_chain_and_respects_size() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let clusters = table::allocate_chain(&mut fs.disk, &fs.geo, 2).unwrap();
        let cs = fs.geo.cluster_size();
        fs.disk.fill(fs.geo.cluster_offset(clusters[0]), cs, b'x');
        fs.disk.fill(fs.geo.cluster_offset(clusters[1]), cs, b'y');
        let mut e = short(b"DATA    BIN", attr::ARCHIVE);
        e.set_first_cluster(clusters[0]);
        e.size = (cs + 3) as u32;
        write_root_slot(&mut fs, 0, &e.to_bytes());
        let mut d = short(b"SUB        ", attr::DIRECTORY);
        d.set_first_cluster(clusters[1]);
        write_root_slot(&mut fs, 1, &d.to_bytes());
        write_root_slot(&mut fs, 2, &short(b"EMPTY   TXT", attr::ARCHIVE).to_bytes());

        let data = fs.read_file("/DATA.BIN").unwrap();
        assert_eq!(data.len(), cs + 3);
        assert!(data[..cs].iter().all(|&b| b == b'x'));
        assert_eq!(&data[cs..], b"yyy");
        assert_eq!(fs.read_file("/EMPTY.TXT").unwrap(), Vec::<u8>::new());
        assert_eq!(fs.read_file("/SUB"), Err(Error::IsADirectory));
        assert_eq!(fs.read_file("/"), Err(Error::IsADirectory));
        assert_eq!(fs.read_file("/MISSING"), Err(Error::NotFound));
        assert_eq!(fs.list_dir("/DATA.BIN"), Err(Error::NotADirectory));
    }

    #[test]
    fn nested_directory_is_resolved_through_its_cluster() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let c = table::allocate_chain(&mut fs.disk, &fs.geo, 1).unwrap()[0];
        let mut d = short(b"SUB        ", attr::DIRECTORY);
        d.set_first_cluster(c);
        write_root_slot(&mut fs, 0, &d.to_bytes());
        let base = fs.geo.cluster_offset(c);
        let mut dot = short(b".          ", attr::DIRECTORY);
        dot.set_first_cluster(c);
        fs.disk.write(base, &dot.to_bytes());
        fs.disk.write(base + ENTRY_SIZE, &short(b"..         ", attr::DIRECTORY).to_bytes());
        fs.disk.write(base + 2 * ENTRY_SIZE, &short(b"INNER   TXT", attr::ARCHIVE).to_bytes());
        let names: Vec<String> = fs.list_dir("/SUB").unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["INNER.TXT"]);
        assert_eq!(fs.stat("/sub/inner.txt").unwrap().name, "INNER.TXT");
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p fat fs::`
Expected: compile error (`list_dir` not found).

- [ ] **Step 3: Implement (add to fs.rs)**

Add imports at the top of `fs.rs`:
```rust
use crate::dir::{self, DirLocation};
use crate::dir_entry::{self, attr, LfnEntry, ShortEntry, DELETED, ENTRY_SIZE, FREE};
use crate::name;
use crate::table;
use fs_core::path;
use fs_core::EntryInfo;
```

Add after the `FatFs` struct:
```rust
/// A directory entry found on disk, with where it lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub dir: DirLocation,
    /// Slot index of the short entry.
    pub slot: usize,
    /// Slot indices of the long-name entries that precede it, in disk order.
    pub lfn_slots: Vec<usize>,
    pub entry: ShortEntry,
    pub long_name: Option<String>,
}

impl Located {
    pub fn name(&self) -> String {
        self.long_name.clone().unwrap_or_else(|| self.entry.display_name())
    }

    pub fn info(&self) -> EntryInfo {
        EntryInfo {
            name: self.name(),
            is_dir: self.entry.is_dir(),
            size: self.entry.size as u64,
            created: dir_entry::unpack(self.entry.create_date, self.entry.create_time),
            modified: dir_entry::unpack(self.entry.write_date, self.entry.write_time),
            accessed: dir_entry::unpack(self.entry.access_date, 0),
        }
    }
}

/// Reassemble a long name from the LFN entries collected before a short
/// entry, or `None` if they are incomplete or do not match its checksum.
fn assemble_lfn(pending: &[(usize, LfnEntry)], short_name: &[u8; 11]) -> Option<String> {
    let (_, last) = pending.last()?;
    let (_, first) = &pending[0];
    if last.order() != 1 || !first.is_last() || first.order() as usize != pending.len() {
        return None;
    }
    let sum = dir_entry::lfn_checksum(short_name);
    if pending.iter().any(|(_, e)| e.checksum != sum) {
        return None;
    }
    let mut units = Vec::with_capacity(pending.len() * 13);
    for (_, e) in pending.iter().rev() {
        units.extend(e.chars());
    }
    Some(name::from_ucs2(&units))
}

fn names_match(a: &str, b: &str) -> bool {
    a.to_uppercase() == b.to_uppercase()
}
```

Add to `impl FatFs`:
```rust
    /// All in-use entries of a directory in slot order, with long names
    /// attached. Includes `.` and `..`; excludes the volume label.
    fn scan_dir(&self, loc: DirLocation) -> Result<Vec<Located>> {
        let mut out = Vec::new();
        let mut pending: Vec<(usize, LfnEntry)> = Vec::new();
        for slot in dir::slots(&self.disk, &self.geo, loc)? {
            let bytes = self.disk.read(slot.offset, ENTRY_SIZE);
            match bytes[0] {
                FREE => break,
                DELETED => {
                    pending.clear();
                    continue;
                }
                _ => {}
            }
            if attr::is_lfn(bytes[11]) {
                let lfn = LfnEntry::parse(bytes);
                if lfn.is_last() {
                    pending.clear();
                    pending.push((slot.index, lfn));
                } else if let Some((_, prev)) = pending.last() {
                    if prev.order() == lfn.order() + 1 && prev.checksum == lfn.checksum {
                        pending.push((slot.index, lfn));
                    } else {
                        pending.clear();
                    }
                }
                continue;
            }
            let entry = ShortEntry::parse(bytes);
            if entry.is_volume_label() {
                pending.clear();
                continue;
            }
            let long_name = assemble_lfn(&pending, &entry.name);
            let lfn_slots = if long_name.is_some() {
                pending.iter().map(|(i, _)| *i).collect()
            } else {
                Vec::new()
            };
            pending.clear();
            out.push(Located { dir: loc, slot: slot.index, lfn_slots, entry, long_name });
        }
        Ok(out)
    }

    /// Case-insensitive lookup against both the long and the short name.
    fn find_in_dir(&self, loc: DirLocation, name: &str) -> Result<Option<Located>> {
        Ok(self.scan_dir(loc)?.into_iter().find(|l| {
            !l.entry.is_dot_entry()
                && (names_match(&l.entry.display_name(), name)
                    || l.long_name.as_deref().is_some_and(|ln| names_match(ln, name)))
        }))
    }

    /// Walk directory components from the root.
    fn resolve_dir(&self, parts: &[String]) -> Result<DirLocation> {
        let mut loc = DirLocation::Root;
        for part in parts {
            let found = self.find_in_dir(loc, part)?.ok_or(Error::NotFound)?;
            if !found.entry.is_dir() {
                return Err(Error::NotADirectory);
            }
            loc = DirLocation::from_cluster(found.entry.first_cluster());
        }
        Ok(loc)
    }

    /// `Ok(None)` for the root, `Err(NotFound)` if the path is absent.
    fn resolve(&self, path: &str) -> Result<Option<Located>> {
        let parts = path::parse(path)?;
        let Some((name, parent)) = parts.split_last() else {
            return Ok(None);
        };
        let loc = self.resolve_dir(parent)?;
        self.find_in_dir(loc, name)?.ok_or(Error::NotFound).map(Some)
    }

    fn read_chain_data(&self, first: u32, size: usize) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(size);
        if first == 0 || size == 0 {
            return Ok(out);
        }
        for cluster in table::chain(&self.disk, &self.geo, first)? {
            if out.len() >= size {
                break;
            }
            let take = (size - out.len()).min(self.geo.cluster_size());
            out.extend_from_slice(self.disk.read(self.geo.cluster_offset(cluster), take));
        }
        if out.len() < size {
            return Err(Error::CorruptImage(format!(
                "file claims {size} bytes but its cluster chain holds only {}",
                out.len()
            )));
        }
        Ok(out)
    }

    pub fn list_dir(&self, path: &str) -> Result<Vec<EntryInfo>> {
        let parts = path::parse(path)?;
        let loc = self.resolve_dir(&parts)?;
        Ok(self
            .scan_dir(loc)?
            .into_iter()
            .filter(|l| !l.entry.is_dot_entry())
            .map(|l| l.info())
            .collect())
    }

    pub fn stat(&self, path: &str) -> Result<EntryInfo> {
        match self.resolve(path)? {
            Some(located) => Ok(located.info()),
            None => Ok(EntryInfo {
                name: "/".into(),
                is_dir: true,
                size: 0,
                created: None,
                modified: None,
                accessed: None,
            }),
        }
    }

    pub fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        let located = self.resolve(path)?.ok_or(Error::IsADirectory)?;
        if located.entry.is_dir() {
            return Err(Error::IsADirectory);
        }
        self.read_chain_data(located.entry.first_cluster(), located.entry.size as usize)
    }
```

Add `pub use fs::{FatFs, Located};` in `lib.rs` (replacing `pub use fs::FatFs;`).

- [ ] **Step 4: Run tests**

Run: `cargo test -p fat fs::`
Expected: 10 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/fat
git commit -m "feat(fat): directory scanning with LFN reassembly, list_dir, stat, read_file"
```

---

### Task 12: create_file and create_dir

**Files:**
- Modify: `crates/fat/src/fs.rs`

**Interfaces:**
- Consumes: `name::{validate_long_name, is_valid_short_name, to_short_name_bytes, generate_short_name, lfn_chunks}`, `dir::{find_free_run, grow, slots}`, `table::{allocate_chain, free_chain}`, `FatEvent`, `EntryKind`, `path::split_parent`
- Produces: `create_file(&mut self, path, data) -> Result<OpRecord>`, `create_dir(&mut self, path) -> Result<OpRecord>`; private `write_data(&[u8]) -> Result<Vec<u32>>`, `write_new_entry(parent, parent_name, name, attributes, first_cluster, size) -> Result<()>`.

- [ ] **Step 1: Write failing tests (append to fs.rs tests)**

```rust
    fn tiny_fs() -> FatFs {
        FatFs::format(FormatOptions {
            total_sectors: 128,
            sectors_per_cluster: 1,
            root_entries: 16,
            enforce_fat16_range: false,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn create_file_round_trips_and_records_the_operation() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        fs.set_now(DateTime::new(2026, 9, 21, 8, 30, 0));
        let cs = fs.geo.cluster_size();
        let data: Vec<u8> = (0..cs * 2 + 10).map(|i| i as u8).collect();
        let rec = fs.create_file("/HELLO.TXT", &data).unwrap();
        assert_eq!(rec.op, "create_file /HELLO.TXT");
        assert!(!rec.changes.is_empty());
        let kinds = rec.event_kinds();
        assert_eq!(kinds.iter().filter(|k| **k == "cluster_allocated").count(), 3);
        assert_eq!(kinds.iter().filter(|k| **k == "data_written").count(), 3);
        assert_eq!(kinds.iter().filter(|k| **k == "dir_entry_written").count(), 1);
        assert_eq!(fs.read_file("/hello.txt").unwrap(), data);
        let info = fs.stat("/HELLO.TXT").unwrap();
        assert_eq!(info.size, data.len() as u64);
        assert_eq!(info.created, Some(DateTime::new(2026, 9, 21, 8, 30, 0)));
        assert_eq!(fs.history().len(), 1);
        assert_eq!(fs.create_file("/hello.txt", b"x").unwrap_err(), Error::AlreadyExists);
        assert_eq!(fs.history().len(), 1);
    }

    #[test]
    fn empty_file_has_no_clusters() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let rec = fs.create_file("/EMPTY", b"").unwrap();
        assert!(!rec.event_kinds().contains(&"cluster_allocated"));
        assert_eq!(fs.read_file("/EMPTY").unwrap(), Vec::<u8>::new());
        assert_eq!(table::count_free(&fs.disk, &fs.geo), fs.geo.cluster_count);
    }

    #[test]
    fn exact_cluster_multiple_uses_no_extra_cluster() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let cs = fs.geo.cluster_size();
        fs.create_file("/TWO.BIN", &vec![7u8; cs * 2]).unwrap();
        assert_eq!(table::count_free(&fs.disk, &fs.geo), fs.geo.cluster_count - 2);
        assert_eq!(fs.read_file("/TWO.BIN").unwrap().len(), cs * 2);
    }

    #[test]
    fn long_names_get_lfn_entries_and_tilde_aliases() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let rec = fs.create_file("/My First File.txt", b"1").unwrap();
        assert_eq!(rec.event_kinds().iter().filter(|k| **k == "dir_entry_written").count(), 3);
        fs.create_file("/My Second File.txt", b"2").unwrap();
        let list = fs.list_dir("/").unwrap();
        assert_eq!(list[0].name, "My First File.txt");
        assert_eq!(list[1].name, "My Second File.txt");
        assert_eq!(fs.stat("/MYFIRS~1.TXT").unwrap().name, "My First File.txt");
        assert_eq!(fs.stat("/MYSECO~1.TXT").unwrap().name, "My Second File.txt");
        fs.create_file("/my first file.doc", b"3").unwrap();
        assert_eq!(fs.stat("/MYFIRS~1.DOC").unwrap().name, "my first file.doc");
        fs.create_file("/My First File (copy).txt", b"4").unwrap();
        assert_eq!(fs.stat("/MYFIRS~2.TXT").unwrap().name, "My First File (copy).txt");
        assert_eq!(fs.read_file("/MY FIRST FILE (COPY).TXT").unwrap(), b"4");
        assert_eq!(fs.create_file("/bad*name", b"").unwrap_err(), Error::InvalidName);
        assert_eq!(fs.create_file("/", b"").unwrap_err(), Error::InvalidPath);
    }

    #[test]
    fn create_dir_writes_dot_entries_and_nests() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let rec = fs.create_dir("/DOCS").unwrap();
        assert_eq!(rec.event_kinds().iter().filter(|k| **k == "dir_entry_written").count(), 3);
        assert!(fs.stat("/DOCS").unwrap().is_dir);
        assert_eq!(fs.list_dir("/DOCS").unwrap(), Vec::new());
        fs.create_dir("/DOCS/NOTES").unwrap();
        fs.create_file("/DOCS/NOTES/a.txt", b"hi").unwrap();
        assert_eq!(fs.read_file("/docs/notes/A.TXT").unwrap(), b"hi");
        assert_eq!(fs.list_dir("/DOCS").unwrap()[0].name, "NOTES");
        let docs = fs.resolve("/DOCS").unwrap().unwrap();
        let notes = fs.resolve("/DOCS/NOTES").unwrap().unwrap();
        let base = fs.geo.cluster_offset(notes.entry.first_cluster());
        let dot = ShortEntry::parse(fs.disk.read(base, ENTRY_SIZE));
        let dotdot = ShortEntry::parse(fs.disk.read(base + ENTRY_SIZE, ENTRY_SIZE));
        assert_eq!(dot.name, *b".          ");
        assert_eq!(dot.first_cluster(), notes.entry.first_cluster());
        assert_eq!(dotdot.name, *b"..         ");
        assert_eq!(dotdot.first_cluster(), docs.entry.first_cluster());
        let docs_base = fs.geo.cluster_offset(docs.entry.first_cluster());
        assert_eq!(ShortEntry::parse(fs.disk.read(docs_base + ENTRY_SIZE, ENTRY_SIZE)).first_cluster(), 0);
        assert_eq!(fs.create_dir("/DOCS").unwrap_err(), Error::AlreadyExists);
        assert_eq!(fs.create_file("/DOCS/NOTES/a.txt/x", b"").unwrap_err(), Error::NotADirectory);
        assert_eq!(fs.create_file("/NOPE/x", b"").unwrap_err(), Error::NotFound);
    }

    #[test]
    fn subdirectory_grows_when_its_cluster_fills() {
        let mut fs = tiny_fs();
        fs.create_dir("/SUB").unwrap();
        let free_before = table::count_free(&fs.disk, &fs.geo);
        // 16 slots per 512-byte cluster; two are taken by . and ..
        let mut grew = false;
        for i in 0..20 {
            let rec = fs.create_file(&format!("/SUB/F{i}"), b"").unwrap();
            grew |= rec.event_kinds().contains(&"directory_grown");
        }
        assert!(grew);
        assert_eq!(fs.list_dir("/SUB").unwrap().len(), 20);
        assert_eq!(table::count_free(&fs.disk, &fs.geo), free_before - 1);
    }

    #[test]
    fn root_directory_full_and_disk_full_leave_nothing_behind() {
        let mut fs = tiny_fs();
        for i in 0..16 {
            fs.create_file(&format!("/F{i}"), b"").unwrap();
        }
        assert_eq!(fs.create_file("/F16", b"").unwrap_err(), Error::DirectoryFull);
        assert_eq!(fs.create_file("/Long name needs two slots", b"").unwrap_err(), Error::DirectoryFull);
        assert_eq!(fs.list_dir("/").unwrap().len(), 16);
        assert_eq!(fs.history().len(), 16);

        let mut fs = tiny_fs();
        let free = table::count_free(&fs.disk, &fs.geo) as usize;
        let cs = fs.geo.cluster_size();
        assert_eq!(fs.create_file("/BIG", &vec![1u8; (free + 1) * cs]).unwrap_err(), Error::DiskFull);
        assert_eq!(fs.list_dir("/").unwrap(), Vec::new());
        assert_eq!(table::count_free(&fs.disk, &fs.geo) as usize, free);
        fs.create_file("/FITS", &vec![1u8; free * cs]).unwrap();
        assert_eq!(table::count_free(&fs.disk, &fs.geo), 0);
        assert_eq!(fs.create_dir("/D").unwrap_err(), Error::DiskFull);
        assert_eq!(fs.history().len(), 1);
    }

    #[test]
    fn create_in_full_root_with_data_frees_the_clusters_again() {
        let mut fs = tiny_fs();
        for i in 0..16 {
            fs.create_file(&format!("/F{i}"), b"").unwrap();
        }
        let free = table::count_free(&fs.disk, &fs.geo);
        assert_eq!(fs.create_file("/X", b"data").unwrap_err(), Error::DirectoryFull);
        assert_eq!(table::count_free(&fs.disk, &fs.geo), free);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p fat fs::`
Expected: compile error (`create_file` not found).

- [ ] **Step 3: Implement (add to `impl FatFs`; remove the `#[allow(dead_code)]` from `run_op` and `dir_display_name`)**

Add `use crate::events::{EntryKind, FatEvent};` and `use std::collections::HashSet;` to the imports.

```rust
    pub fn create_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        let (parent_parts, name) = path::split_parent(path)?;
        name::validate_long_name(&name)?;
        let parent = self.resolve_dir(&parent_parts)?;
        if self.find_in_dir(parent, &name)?.is_some() {
            return Err(Error::AlreadyExists);
        }
        let size = u32::try_from(data.len()).map_err(|_| Error::FileTooLarge)?;
        let parent_name = Self::dir_display_name(&parent_parts);
        self.run_op(format!("create_file {path}"), |fs| {
            let clusters = fs.write_data(data)?;
            let first = clusters.first().copied().unwrap_or(0);
            if let Err(e) = fs.write_new_entry(parent, &parent_name, &name, attr::ARCHIVE, first, size) {
                if first != 0 {
                    table::free_chain(&mut fs.disk, &fs.geo, first)?;
                }
                return Err(e);
            }
            Ok(())
        })
    }

    pub fn create_dir(&mut self, path: &str) -> Result<OpRecord> {
        let (parent_parts, name) = path::split_parent(path)?;
        name::validate_long_name(&name)?;
        let parent = self.resolve_dir(&parent_parts)?;
        if self.find_in_dir(parent, &name)?.is_some() {
            return Err(Error::AlreadyExists);
        }
        let parent_name = Self::dir_display_name(&parent_parts);
        let own_name = Self::dir_display_name(&[parent_parts.clone(), vec![name.clone()]].concat());
        self.run_op(format!("create_dir {path}"), |fs| {
            let cluster = table::allocate_chain(&mut fs.disk, &fs.geo, 1)?[0];
            let base = fs.geo.cluster_offset(cluster);
            let cs = fs.geo.cluster_size();
            fs.disk.fill(base, cs, 0);
            let parent_cluster = match parent {
                DirLocation::Root => 0,
                DirLocation::Cluster(c) => c,
            };
            let mut dot = ShortEntry::new(*b".          ", attr::DIRECTORY, &fs.now);
            dot.set_first_cluster(cluster);
            let mut dotdot = ShortEntry::new(*b"..         ", attr::DIRECTORY, &fs.now);
            dotdot.set_first_cluster(parent_cluster);
            for (slot, entry) in [(0, dot), (1, dotdot)] {
                let off = base + slot * ENTRY_SIZE;
                fs.disk.write(off, &entry.to_bytes());
                fs.disk.event(Box::new(FatEvent::DirEntryWritten {
                    dir: own_name.clone(),
                    slot,
                    kind: EntryKind::Short,
                    range: off..off + ENTRY_SIZE,
                }));
            }
            if let Err(e) = fs.write_new_entry(parent, &parent_name, &name, attr::DIRECTORY, cluster, 0) {
                table::free_chain(&mut fs.disk, &fs.geo, cluster)?;
                return Err(e);
            }
            Ok(())
        })
    }

    /// Allocate a chain for `data` and write it cluster by cluster. The tail
    /// of the last cluster is zeroed so cluster contents are deterministic.
    fn write_data(&mut self, data: &[u8]) -> Result<Vec<u32>> {
        let cs = self.geo.cluster_size();
        let needed = data.len().div_ceil(cs) as u32;
        let clusters = table::allocate_chain(&mut self.disk, &self.geo, needed)?;
        for (i, &cluster) in clusters.iter().enumerate() {
            let chunk = &data[i * cs..((i + 1) * cs).min(data.len())];
            let off = self.geo.cluster_offset(cluster);
            self.disk.write(off, chunk);
            if chunk.len() < cs {
                self.disk.fill(off + chunk.len(), cs - chunk.len(), 0);
            }
            self.disk.event(Box::new(FatEvent::DataWritten { cluster, bytes: chunk.len(), range: off..off + cs }));
        }
        Ok(clusters)
    }

    /// Write the LFN entries (if the name needs them) and the short entry
    /// into the first free run of slots, growing the directory as needed.
    fn write_new_entry(
        &mut self,
        parent: DirLocation,
        parent_name: &str,
        name: &str,
        attributes: u8,
        first_cluster: u32,
        size: u32,
    ) -> Result<()> {
        let (short, chunks) = match name::to_short_name_bytes(name) {
            Some(short) => (short, Vec::new()),
            None => {
                let taken: HashSet<[u8; 11]> = self.scan_dir(parent)?.iter().map(|l| l.entry.name).collect();
                let short = name::generate_short_name(name, &|c| taken.contains(c))?;
                (short, name::lfn_chunks(name))
            }
        };
        let needed = chunks.len() + 1;
        let start = loop {
            if let Some(i) = dir::find_free_run(&self.disk, &self.geo, parent, needed)? {
                break i;
            }
            dir::grow(&mut self.disk, &self.geo, parent, parent_name)?;
        };
        let slots = dir::slots(&self.disk, &self.geo, parent)?;
        let checksum = dir_entry::lfn_checksum(&short);
        let n = chunks.len();
        for (i, chunk) in chunks.iter().enumerate().rev() {
            let entry = LfnEntry::new((i + 1) as u8, i == n - 1, checksum, chunk);
            let slot = slots[start + (n - 1 - i)];
            self.disk.write(slot.offset, &entry.to_bytes());
            self.disk.event(Box::new(FatEvent::DirEntryWritten {
                dir: parent_name.to_string(),
                slot: slot.index,
                kind: EntryKind::Lfn,
                range: slot.offset..slot.offset + ENTRY_SIZE,
            }));
        }
        let mut entry = ShortEntry::new(short, attributes, &self.now);
        entry.set_first_cluster(first_cluster);
        entry.size = size;
        let slot = slots[start + n];
        self.disk.write(slot.offset, &entry.to_bytes());
        self.disk.event(Box::new(FatEvent::DirEntryWritten {
            dir: parent_name.to_string(),
            slot: slot.index,
            kind: EntryKind::Short,
            range: slot.offset..slot.offset + ENTRY_SIZE,
        }));
        Ok(())
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p fat`
Expected: all pass (18 in fs::).

- [ ] **Step 5: Commit**

```bash
git add crates/fat
git commit -m "feat(fat): create_file and create_dir with LFN entries and rollback"
```

---

### Task 13: delete_file, remove_dir, write_file

**Files:**
- Modify: `crates/fat/src/fs.rs`

**Interfaces:**
- Consumes: `Located`, `dir::slots`, `table::{chain, count_free, free_chain}`, `dir_entry::{pack_date, pack_time, DELETED}`
- Produces: `delete_file(&mut self, path) -> Result<OpRecord>`, `remove_dir(&mut self, path) -> Result<OpRecord>`, `write_file(&mut self, path, data) -> Result<OpRecord>`; private `remove_entry(&Located, dir_name: &str) -> Result<()>`, `parent_display_name(path) -> Result<String>`.

- [ ] **Step 1: Write failing tests (append to fs.rs tests)**

```rust
    #[test]
    fn delete_marks_entries_frees_chain_and_leaves_data() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        fs.create_file("/My File.txt", b"keep me around").unwrap();
        let located = fs.resolve("/My File.txt").unwrap().unwrap();
        let first = located.entry.first_cluster();
        let data_off = fs.geo.cluster_offset(first);
        let rec = fs.delete_file("/my file.txt").unwrap();
        assert_eq!(rec.op, "delete_file /my file.txt");
        let kinds = rec.event_kinds();
        assert_eq!(kinds.iter().filter(|k| **k == "dir_entry_deleted").count(), 2);
        assert_eq!(kinds.iter().filter(|k| **k == "cluster_freed").count(), 1);
        assert_eq!(fs.stat("/My File.txt"), Err(Error::NotFound));
        assert_eq!(fs.list_dir("/").unwrap(), Vec::new());
        let slots = dir::slots(&fs.disk, &fs.geo, DirLocation::Root).unwrap();
        assert_eq!(fs.disk.read(slots[0].offset, 1)[0], DELETED);
        assert_eq!(fs.disk.read(slots[1].offset, 1)[0], DELETED);
        assert_eq!(&fs.disk.read(slots[1].offset + 1, 10), b"YFILE~1TXT");
        assert_eq!(fs.disk.read(data_off, 14), b"keep me around");
        assert_eq!(table::read_entry(&fs.disk, &fs.geo, first), FatEntry::Free);
        assert_eq!(fs.delete_file("/My File.txt").unwrap_err(), Error::NotFound);
        assert_eq!(fs.delete_file("/").unwrap_err(), Error::InvalidPath);
        fs.create_dir("/D").unwrap();
        assert_eq!(fs.delete_file("/D").unwrap_err(), Error::IsADirectory);
    }

    #[test]
    fn deleted_slots_are_reused() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        fs.create_file("/A", b"").unwrap();
        fs.create_file("/B", b"").unwrap();
        fs.delete_file("/A").unwrap();
        fs.create_file("/C", b"").unwrap();
        let names: Vec<String> = fs.list_dir("/").unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["C", "B"]);
    }

    #[test]
    fn remove_dir_requires_empty() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        fs.create_dir("/D").unwrap();
        fs.create_file("/D/X", b"1").unwrap();
        assert_eq!(fs.remove_dir("/D").unwrap_err(), Error::DirectoryNotEmpty);
        fs.delete_file("/D/X").unwrap();
        let free = table::count_free(&fs.disk, &fs.geo);
        fs.remove_dir("/D").unwrap();
        assert_eq!(table::count_free(&fs.disk, &fs.geo), free + 1);
        assert_eq!(fs.stat("/D"), Err(Error::NotFound));
        fs.create_file("/F", b"").unwrap();
        assert_eq!(fs.remove_dir("/F").unwrap_err(), Error::NotADirectory);
        assert_eq!(fs.remove_dir("/").unwrap_err(), Error::InvalidPath);
    }

    #[test]
    fn write_file_replaces_data_and_reuses_the_entry() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let cs = fs.geo.cluster_size();
        fs.create_file("/F.TXT", &vec![1u8; cs * 3]).unwrap();
        let free = table::count_free(&fs.disk, &fs.geo);
        let slot_before = fs.resolve("/F.TXT").unwrap().unwrap().slot;
        fs.set_now(DateTime::new(2027, 1, 2, 3, 4, 6));
        let rec = fs.write_file("/F.TXT", b"small").unwrap();
        assert_eq!(rec.event_kinds().iter().filter(|k| **k == "cluster_freed").count(), 3);
        assert_eq!(rec.event_kinds().iter().filter(|k| **k == "cluster_allocated").count(), 1);
        assert_eq!(fs.read_file("/F.TXT").unwrap(), b"small");
        assert_eq!(table::count_free(&fs.disk, &fs.geo), free + 2);
        let after = fs.resolve("/F.TXT").unwrap().unwrap();
        assert_eq!(after.slot, slot_before);
        let info = after.info();
        assert_eq!(info.modified, Some(DateTime::new(2027, 1, 2, 3, 4, 6)));
        assert_eq!(info.created, Some(DateTime::default()));
        fs.write_file("/F.TXT", &vec![2u8; cs * 5]).unwrap();
        assert_eq!(fs.read_file("/F.TXT").unwrap().len(), cs * 5);
        assert_eq!(table::count_free(&fs.disk, &fs.geo), free - 2);
        fs.write_file("/F.TXT", b"").unwrap();
        assert_eq!(fs.resolve("/F.TXT").unwrap().unwrap().entry.first_cluster(), 0);
        assert_eq!(fs.write_file("/NOPE", b"").unwrap_err(), Error::NotFound);
        fs.create_dir("/D").unwrap();
        assert_eq!(fs.write_file("/D", b"").unwrap_err(), Error::IsADirectory);
    }

    #[test]
    fn write_file_can_grow_into_its_own_old_clusters() {
        let mut fs = tiny_fs();
        let free = table::count_free(&fs.disk, &fs.geo) as usize;
        let cs = fs.geo.cluster_size();
        fs.create_file("/F", &vec![1u8; (free - 1) * cs]).unwrap();
        fs.write_file("/F", &vec![2u8; free * cs]).unwrap();
        assert_eq!(table::count_free(&fs.disk, &fs.geo), 0);
        assert_eq!(fs.write_file("/F", &vec![3u8; (free + 1) * cs]).unwrap_err(), Error::DiskFull);
        assert_eq!(fs.read_file("/F").unwrap()[0], 2);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p fat fs::`
Expected: compile error (`delete_file` not found).

- [ ] **Step 3: Implement (add to `impl FatFs`)**

Add `use crate::table::FatEntry;` to the test module imports if not already present.

```rust
    pub fn delete_file(&mut self, path: &str) -> Result<OpRecord> {
        let located = self.resolve(path)?.ok_or(Error::InvalidPath)?;
        if located.entry.is_dir() {
            return Err(Error::IsADirectory);
        }
        let dir_name = Self::parent_display_name(path)?;
        self.run_op(format!("delete_file {path}"), |fs| fs.remove_entry(&located, &dir_name))
    }

    pub fn remove_dir(&mut self, path: &str) -> Result<OpRecord> {
        let located = self.resolve(path)?.ok_or(Error::InvalidPath)?;
        if !located.entry.is_dir() {
            return Err(Error::NotADirectory);
        }
        let contents = self.scan_dir(DirLocation::Cluster(located.entry.first_cluster()))?;
        if contents.iter().any(|l| !l.entry.is_dot_entry()) {
            return Err(Error::DirectoryNotEmpty);
        }
        let dir_name = Self::parent_display_name(path)?;
        self.run_op(format!("remove_dir {path}"), |fs| fs.remove_entry(&located, &dir_name))
    }

    pub fn write_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        let located = self.resolve(path)?.ok_or(Error::InvalidPath)?;
        if located.entry.is_dir() {
            return Err(Error::IsADirectory);
        }
        let size = u32::try_from(data.len()).map_err(|_| Error::FileTooLarge)?;
        let needed = data.len().div_ceil(self.geo.cluster_size()) as u32;
        let old_first = located.entry.first_cluster();
        let old_len = if old_first == 0 {
            0
        } else {
            table::chain(&self.disk, &self.geo, old_first)?.len() as u32
        };
        if table::count_free(&self.disk, &self.geo) + old_len < needed {
            return Err(Error::DiskFull);
        }
        let dir_name = Self::parent_display_name(path)?;
        self.run_op(format!("write_file {path}"), |fs| {
            if old_first != 0 {
                table::free_chain(&mut fs.disk, &fs.geo, old_first)?;
            }
            let clusters = fs.write_data(data)?;
            let mut entry = located.entry.clone();
            entry.set_first_cluster(clusters.first().copied().unwrap_or(0));
            entry.size = size;
            entry.write_time = dir_entry::pack_time(&fs.now);
            entry.write_date = dir_entry::pack_date(&fs.now);
            entry.access_date = dir_entry::pack_date(&fs.now);
            let slots = dir::slots(&fs.disk, &fs.geo, located.dir)?;
            let slot = slots[located.slot];
            fs.disk.write(slot.offset, &entry.to_bytes());
            fs.disk.event(Box::new(FatEvent::DirEntryWritten {
                dir: dir_name.clone(),
                slot: slot.index,
                kind: EntryKind::Short,
                range: slot.offset..slot.offset + ENTRY_SIZE,
            }));
            Ok(())
        })
    }

    /// Mark the entry and its LFN entries deleted and free its chain. Data
    /// bytes are left in place, exactly as a real driver leaves them.
    fn remove_entry(&mut self, located: &Located, dir_name: &str) -> Result<()> {
        let slots = dir::slots(&self.disk, &self.geo, located.dir)?;
        for &i in located.lfn_slots.iter().chain(std::iter::once(&located.slot)) {
            let off = slots[i].offset;
            self.disk.write(off, &[DELETED]);
            self.disk.event(Box::new(FatEvent::DirEntryDeleted {
                dir: dir_name.to_string(),
                slot: i,
                range: off..off + ENTRY_SIZE,
            }));
        }
        let first = located.entry.first_cluster();
        if first != 0 {
            table::free_chain(&mut self.disk, &self.geo, first)?;
        }
        Ok(())
    }

    fn parent_display_name(path: &str) -> Result<String> {
        let (parts, _) = path::split_parent(path)?;
        Ok(Self::dir_display_name(&parts))
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p fat && cargo clippy -p fat --all-targets`
Expected: all pass, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/fat
git commit -m "feat(fat): delete_file, remove_dir, and write_file"
```

---

### Task 14: Inspection API, sector annotations, and the FileSystem trait impl

**Files:**
- Modify: `crates/fat/src/fs.rs`, `crates/fat/src/lib.rs`

**Interfaces:**
- Consumes: everything above, `fs_core::{Annotation, FileSystem}`
- Produces: `RawEntry::{Free, Deleted { bytes: [u8; 32] }, Short(ShortEntry), Lfn(LfnEntry)}`, `fat_entries(&self, fat: u8) -> Vec<FatEntry>` (index = cluster number; entries 0 and 1 report `Reserved`), `cluster_chain(&self, start) -> Result<Vec<u32>>`, `raw_dir_entries(&self, path) -> Result<Vec<RawEntry>>`, `annotate_sector(&self, sector) -> Vec<Annotation>`, `impl FileSystem for FatFs`; private `cluster_owners() -> BTreeMap<u32, (String, bool)>`.

- [ ] **Step 1: Write failing tests (append to fs.rs tests)**

```rust
    use fs_core::FileSystem;

    #[test]
    fn fat_entries_and_cluster_chain() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let cs = fs.geo.cluster_size();
        fs.create_file("/F", &vec![0u8; cs * 3]).unwrap();
        let entries = fs.fat_entries(0);
        assert_eq!(entries.len() as u32, fs.geo.cluster_count + 2);
        assert_eq!(entries[0], FatEntry::Reserved);
        assert_eq!(entries[1], FatEntry::Reserved);
        assert_eq!(entries[2], FatEntry::Next(3));
        assert_eq!(entries[3], FatEntry::Next(4));
        assert_eq!(entries[4], FatEntry::EndOfChain);
        assert_eq!(entries[5], FatEntry::Free);
        assert_eq!(fs.fat_entries(1), entries);
        assert_eq!(fs.cluster_chain(2).unwrap(), vec![2, 3, 4]);
        assert!(matches!(fs.cluster_chain(0), Err(Error::CorruptImage(_))));
    }

    #[test]
    fn raw_dir_entries_show_every_slot() {
        let mut fs = tiny_fs();
        fs.create_file("/My File.txt", b"").unwrap();
        fs.create_file("/B", b"").unwrap();
        fs.delete_file("/B").unwrap();
        let raw = fs.raw_dir_entries("/").unwrap();
        assert_eq!(raw.len(), 16);
        assert!(matches!(&raw[0], RawEntry::Lfn(e) if e.order() == 1));
        assert!(matches!(&raw[1], RawEntry::Short(e) if e.name == *b"MYFILE~1TXT"));
        assert!(matches!(&raw[2], RawEntry::Deleted { bytes } if &bytes[1..11] == b"          "));
        assert_eq!(raw[3], RawEntry::Free);
        assert_eq!(fs.raw_dir_entries("/My File.txt"), Err(Error::NotADirectory));
    }

    #[test]
    fn annotations_cover_boot_fat_directory_and_data_sectors() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        fs.create_dir("/DOCS").unwrap();
        fs.create_file("/DOCS/N.TXT", b"note").unwrap();
        let g = fs.geo.clone();

        let boot = fs.annotate_sector(0);
        assert!(boot.iter().any(|a| a.label == "bytes per sector" && a.value == "512" && a.range == (11..13)));
        assert!(boot.iter().any(|a| a.label == "boot signature" && a.range == (510..512)));

        let fat = fs.annotate_sector(g.reserved_sectors);
        assert_eq!(fat.len(), 256);
        assert_eq!(fat[0].range, 0..2);
        assert_eq!(fat[0].label, "cluster 0 (media descriptor)");
        assert_eq!(fat[2].label, "cluster 2");
        assert_eq!(fat[2].value, "end of chain");
        assert_eq!(fat[3].value, "end of chain");
        assert_eq!(fat[4].value, "free");

        let root = fs.annotate_sector(g.first_root_dir_sector);
        assert_eq!(root.len(), 16);
        assert_eq!(root[0].range, 0..32);
        assert!(root[0].value.contains("DOCS") && root[0].value.contains("directory"));
        assert_eq!(root[1].value, "free");

        let docs_cluster = fs.resolve("/DOCS").unwrap().unwrap().entry.first_cluster();
        let docs_sector = g.first_data_sector + (docs_cluster as u64 - 2) * g.sectors_per_cluster as u64;
        let docs = fs.annotate_sector(docs_sector);
        assert!(docs[0].value.contains(".") && docs[2].value.contains("N.TXT"));

        let n_cluster = fs.resolve("/DOCS/N.TXT").unwrap().unwrap().entry.first_cluster();
        let n_sector = g.first_data_sector + (n_cluster as u64 - 2) * g.sectors_per_cluster as u64;
        let data = fs.annotate_sector(n_sector);
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].range, 0..512);
        assert_eq!(data[0].label, format!("cluster {n_cluster}"));
        assert_eq!(data[0].value, "data of /DOCS/N.TXT");

        let unused = fs.annotate_sector(n_sector + 8);
        assert_eq!(unused[0].value, "free cluster");
        assert!(fs.annotate_sector(g.total_sectors).is_empty());
    }

    #[test]
    fn works_through_the_trait_object() {
        let mut fs: Box<dyn FileSystem> = Box::new(FatFs::format(FormatOptions::default()).unwrap());
        assert_eq!(fs.fs_type(), "FAT16");
        fs.create_dir("/A").unwrap();
        fs.create_file("/A/b.txt", b"via trait").unwrap();
        fs.write_file("/A/b.txt", b"again").unwrap();
        assert_eq!(fs.read_file("/A/B.TXT").unwrap(), b"again");
        assert_eq!(fs.list_dir("/A").unwrap()[0].name, "B.TXT");
        fs.delete_file("/A/b.txt").unwrap();
        fs.remove_dir("/A").unwrap();
        assert_eq!(fs.history().len(), 5);
        assert_eq!(fs.layout().len(), 5);
        assert_eq!(fs.disk().sector_count(), 32768);
        fs.set_now(DateTime::new(2000, 1, 1, 0, 0, 0));
        assert!(!fs.annotate_sector(0).is_empty());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p fat fs::`
Expected: compile error (`fat_entries` not found).

- [ ] **Step 3: Implement**

Add imports: `use std::collections::BTreeMap;`, `use std::ops::Range;`, `use fs_core::{Annotation, FileSystem};`, `use crate::table::FatEntry;`.

Add after `Located`:
```rust
/// One directory slot exactly as it is on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawEntry {
    Free,
    Deleted { bytes: [u8; 32] },
    Short(ShortEntry),
    Lfn(LfnEntry),
}

impl RawEntry {
    pub fn parse(bytes: &[u8]) -> RawEntry {
        match bytes[0] {
            FREE => RawEntry::Free,
            DELETED => {
                let mut copy = [0u8; ENTRY_SIZE];
                copy.copy_from_slice(&bytes[..ENTRY_SIZE]);
                RawEntry::Deleted { bytes: copy }
            }
            _ if attr::is_lfn(bytes[11]) => RawEntry::Lfn(LfnEntry::parse(bytes)),
            _ => RawEntry::Short(ShortEntry::parse(bytes)),
        }
    }

    /// One line describing the slot, for annotations.
    pub fn describe(&self) -> String {
        match self {
            RawEntry::Free => "free".into(),
            RawEntry::Deleted { bytes } => {
                let mut name = *b"?          ";
                name[1..].copy_from_slice(&bytes[1..11]);
                let mut e = ShortEntry::parse(bytes);
                e.name = name;
                format!("deleted (was {})", e.display_name())
            }
            RawEntry::Short(e) => {
                let kind = if e.is_dir() { "directory" } else if e.is_volume_label() { "volume label" } else { "file" };
                format!(
                    "{} {} attr=0x{:02X} first_cluster={} size={}",
                    kind,
                    e.display_name(),
                    e.attr,
                    e.first_cluster(),
                    e.size
                )
            }
            RawEntry::Lfn(e) => format!(
                "LFN part {}{} checksum=0x{:02X} \"{}\"",
                e.order(),
                if e.is_last() { " (last)" } else { "" },
                e.checksum,
                name::from_ucs2(&e.chars())
            ),
        }
    }
}

fn annotation(range: Range<usize>, label: impl Into<String>, value: impl Into<String>) -> Annotation {
    Annotation { range, label: label.into(), value: value.into() }
}
```

Add to `impl FatFs`:
```rust
    /// Every entry of one FAT copy, indexed by cluster number.
    pub fn fat_entries(&self, fat: u8) -> Vec<FatEntry> {
        (0..self.geo.cluster_count + 2)
            .map(|c| if c < 2 { FatEntry::Reserved } else { table::read_entry_from(&self.disk, &self.geo, fat, c) })
            .collect()
    }

    pub fn cluster_chain(&self, start: u32) -> Result<Vec<u32>> {
        table::chain(&self.disk, &self.geo, start)
    }

    /// Every slot of a directory, including free, deleted and LFN slots.
    pub fn raw_dir_entries(&self, path: &str) -> Result<Vec<RawEntry>> {
        let parts = path::parse(path)?;
        let loc = self.resolve_dir(&parts)?;
        Ok(dir::slots(&self.disk, &self.geo, loc)?
            .into_iter()
            .map(|s| RawEntry::parse(self.disk.read(s.offset, ENTRY_SIZE)))
            .collect())
    }

    /// Which file or directory each allocated cluster belongs to.
    fn cluster_owners(&self) -> BTreeMap<u32, (String, bool)> {
        let mut owners = BTreeMap::new();
        let mut stack = vec![(DirLocation::Root, String::new())];
        while let Some((loc, prefix)) = stack.pop() {
            let Ok(entries) = self.scan_dir(loc) else { continue };
            for l in entries.into_iter().filter(|l| !l.entry.is_dot_entry()) {
                let path = format!("{prefix}/{}", l.name());
                let first = l.entry.first_cluster();
                if first != 0 {
                    if let Ok(chain) = table::chain(&self.disk, &self.geo, first) {
                        for c in chain {
                            owners.insert(c, (path.clone(), l.entry.is_dir()));
                        }
                    }
                    if l.entry.is_dir() {
                        stack.push((DirLocation::Cluster(first), path));
                    }
                }
            }
        }
        owners
    }

    pub fn annotate_sector(&self, sector: u64) -> Vec<Annotation> {
        let g = &self.geo;
        if sector >= g.total_sectors {
            return Vec::new();
        }
        if sector == 0 {
            return self.annotate_boot_sector();
        }
        if sector < g.reserved_sectors {
            return vec![annotation(0..g.bytes_per_sector, "reserved", "unused reserved sector")];
        }
        if sector < g.first_root_dir_sector {
            return self.annotate_fat_sector(sector);
        }
        if sector < g.first_data_sector {
            return self.annotate_dir_sector(sector);
        }
        let Some(cluster) = g.cluster_of_sector(sector) else {
            return vec![annotation(0..g.bytes_per_sector, "unused", "sector beyond the last cluster")];
        };
        match self.cluster_owners().get(&cluster) {
            Some((_, true)) => self.annotate_dir_sector(sector),
            Some((path, false)) => vec![annotation(0..g.bytes_per_sector, format!("cluster {cluster}"), format!("data of {path}"))],
            None => {
                let state = match table::read_entry(&self.disk, g, cluster) {
                    FatEntry::Free => "free cluster".to_string(),
                    other => format!("cluster marked {other} but owned by no file"),
                };
                vec![annotation(0..g.bytes_per_sector, format!("cluster {cluster}"), state)]
            }
        }
    }

    fn annotate_boot_sector(&self) -> Vec<Annotation> {
        let b = &self.boot;
        let jump = self.disk.read(0, 3);
        let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).to_string();
        vec![
            annotation(0..3, "jump instruction", format!("{:02X} {:02X} {:02X}", jump[0], jump[1], jump[2])),
            annotation(3..11, "OEM name", text(&b.oem_name)),
            annotation(11..13, "bytes per sector", b.bytes_per_sector.to_string()),
            annotation(13..14, "sectors per cluster", b.sectors_per_cluster.to_string()),
            annotation(14..16, "reserved sectors", b.reserved_sectors.to_string()),
            annotation(16..17, "FAT count", b.fat_count.to_string()),
            annotation(17..19, "root directory entries", b.root_entries.to_string()),
            annotation(19..21, "total sectors (16-bit)", b.total_sectors_16.to_string()),
            annotation(21..22, "media descriptor", format!("0x{:02X}", b.media)),
            annotation(22..24, "sectors per FAT", b.sectors_per_fat.to_string()),
            annotation(24..26, "sectors per track", b.sectors_per_track.to_string()),
            annotation(26..28, "heads", b.heads.to_string()),
            annotation(28..32, "hidden sectors", b.hidden_sectors.to_string()),
            annotation(32..36, "total sectors (32-bit)", b.total_sectors_32.to_string()),
            annotation(36..37, "drive number", format!("0x{:02X}", b.drive_number)),
            annotation(37..38, "reserved", "0"),
            annotation(38..39, "extended boot signature", format!("0x{:02X}", b.boot_signature)),
            annotation(39..43, "volume ID", format!("0x{:08X}", b.volume_id)),
            annotation(43..54, "volume label", text(&b.volume_label)),
            annotation(54..62, "filesystem type", text(&b.fs_type)),
            annotation(62..510, "boot code", "unused"),
            annotation(510..512, "boot signature", "55 AA"),
        ]
    }

    fn annotate_fat_sector(&self, sector: u64) -> Vec<Annotation> {
        let g = &self.geo;
        let fat = ((sector - g.reserved_sectors) / g.sectors_per_fat) as u8;
        let sector_in_fat = (sector - g.reserved_sectors) % g.sectors_per_fat;
        let entries_per_sector = g.bytes_per_sector / 2;
        let first_cluster = sector_in_fat as usize * entries_per_sector;
        (0..entries_per_sector)
            .map(|i| {
                let cluster = (first_cluster + i) as u32;
                let range = i * 2..i * 2 + 2;
                if cluster >= g.cluster_count + 2 {
                    return annotation(range, format!("entry {cluster}"), "unused (beyond last cluster)");
                }
                let label = match cluster {
                    0 => "cluster 0 (media descriptor)".to_string(),
                    1 => "cluster 1 (reserved)".to_string(),
                    _ => format!("cluster {cluster}"),
                };
                let value = if cluster < 2 {
                    let raw = self.disk.read(g.fat_entry_offset(fat, cluster), 2);
                    format!("0x{:04X}", u16::from_le_bytes([raw[0], raw[1]]))
                } else {
                    table::read_entry_from(&self.disk, g, fat, cluster).to_string()
                };
                annotation(range, label, value)
            })
            .collect()
    }

    fn annotate_dir_sector(&self, sector: u64) -> Vec<Annotation> {
        let g = &self.geo;
        let base = sector as usize * g.bytes_per_sector;
        (0..g.bytes_per_sector / ENTRY_SIZE)
            .map(|i| {
                let off = base + i * ENTRY_SIZE;
                let raw = RawEntry::parse(self.disk.read(off, ENTRY_SIZE));
                annotation(i * ENTRY_SIZE..(i + 1) * ENTRY_SIZE, format!("slot {i}"), raw.describe())
            })
            .collect()
    }
```

Add the trait impl at the bottom of `fs.rs`:
```rust
impl FileSystem for FatFs {
    fn fs_type(&self) -> &'static str {
        FatFs::fs_type(self)
    }
    fn create_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        FatFs::create_file(self, path, data)
    }
    fn write_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        FatFs::write_file(self, path, data)
    }
    fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        FatFs::read_file(self, path)
    }
    fn delete_file(&mut self, path: &str) -> Result<OpRecord> {
        FatFs::delete_file(self, path)
    }
    fn create_dir(&mut self, path: &str) -> Result<OpRecord> {
        FatFs::create_dir(self, path)
    }
    fn remove_dir(&mut self, path: &str) -> Result<OpRecord> {
        FatFs::remove_dir(self, path)
    }
    fn list_dir(&self, path: &str) -> Result<Vec<EntryInfo>> {
        FatFs::list_dir(self, path)
    }
    fn stat(&self, path: &str) -> Result<EntryInfo> {
        FatFs::stat(self, path)
    }
    fn set_now(&mut self, now: DateTime) {
        FatFs::set_now(self, now)
    }
    fn disk(&self) -> &Disk {
        FatFs::disk(self)
    }
    fn layout(&self) -> Vec<Region> {
        FatFs::layout(self)
    }
    fn annotate_sector(&self, sector: u64) -> Vec<Annotation> {
        FatFs::annotate_sector(self, sector)
    }
    fn history(&self) -> &[OpRecord] {
        FatFs::history(self)
    }
}
```

Update `lib.rs` exports to: `pub use fs::{FatFs, Located, RawEntry};`

- [ ] **Step 4: Run tests**

Run: `cargo test -p fat && cargo clippy --workspace --all-targets`
Expected: all pass, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/fat
git commit -m "feat(fat): inspection API, sector annotations, and FileSystem trait impl"
```

---

### Task 15: Integration tests, macOS mount check, README, WASM build check

**Files:**
- Create: `crates/fat/tests/fat16.rs`, `crates/fat/tests/mount_macos.rs`, `README.md`

**Interfaces:**
- Consumes: the public API of `fat` and `fs_core` only (no private access).

- [ ] **Step 1: Write the integration tests**

`crates/fat/tests/fat16.rs`:
```rust
use fat::{FatEntry, FatFs, FormatOptions};
use fs_core::{DateTime, Error, FileSystem};

fn default_fs() -> FatFs {
    FatFs::format(FormatOptions::default()).unwrap()
}

fn tiny_fs() -> FatFs {
    FatFs::format(FormatOptions {
        total_sectors: 128,
        sectors_per_cluster: 1,
        root_entries: 16,
        enforce_fat16_range: false,
        ..Default::default()
    })
    .unwrap()
}

fn free_clusters(fs: &FatFs) -> usize {
    fs.fat_entries(0).iter().filter(|e| **e == FatEntry::Free).count()
}

#[test]
fn formatted_volume_is_a_valid_fat16() {
    let fs = default_fs();
    let boot = fs.boot_sector();
    assert_eq!(boot.bytes_per_sector, 512);
    assert_eq!(&boot.fs_type, b"FAT16   ");
    assert_eq!(&fs.disk().sector(0)[510..], &[0x55, 0xAA]);
    let g = fs.geometry();
    assert!((4085..65525).contains(&g.cluster_count));
    let entries = fs.fat_entries(0);
    assert_eq!(entries[0], FatEntry::Reserved);
    assert!(entries[2..].iter().all(|e| *e == FatEntry::Free));
    let layout = fs.layout();
    assert_eq!(layout.first().unwrap().sectors.start, 0);
    assert_eq!(layout.last().unwrap().sectors.end, g.total_sectors);
    for pair in layout.windows(2) {
        assert_eq!(pair[0].sectors.end, pair[1].sectors.start);
    }
}

#[test]
fn files_of_every_size_shape_round_trip() {
    let mut fs = default_fs();
    let cs = fs.geometry().cluster_size();
    let cases = [0usize, 1, 100, cs - 1, cs, cs + 1, cs * 2, cs * 3 + 17];
    for (i, len) in cases.iter().enumerate() {
        let data: Vec<u8> = (0..*len).map(|b| (b * 7 + i) as u8).collect();
        fs.create_file(&format!("/F{i}.BIN"), &data).unwrap();
        assert_eq!(fs.read_file(&format!("/F{i}.BIN")).unwrap(), data);
        assert_eq!(fs.stat(&format!("/F{i}.BIN")).unwrap().size, *len as u64);
    }
    assert_eq!(fs.list_dir("/").unwrap().len(), cases.len());
}

#[test]
fn overwrite_frees_and_reallocates() {
    let mut fs = default_fs();
    let cs = fs.geometry().cluster_size();
    let all = free_clusters(&fs);
    fs.create_file("/F", &vec![1; cs * 4]).unwrap();
    assert_eq!(free_clusters(&fs), all - 4);
    fs.write_file("/F", &vec![2; cs]).unwrap();
    assert_eq!(free_clusters(&fs), all - 1);
    fs.write_file("/F", &vec![3; cs * 6]).unwrap();
    assert_eq!(free_clusters(&fs), all - 6);
    assert_eq!(fs.read_file("/F").unwrap(), vec![3; cs * 6]);
}

#[test]
fn delete_leaves_data_but_frees_everything_else() {
    let mut fs = default_fs();
    let all = free_clusters(&fs);
    fs.create_file("/SECRET.TXT", b"still here").unwrap();
    let first = fs.cluster_chain(2).unwrap()[0];
    fs.delete_file("/SECRET.TXT").unwrap();
    assert_eq!(free_clusters(&fs), all);
    let off = fs.geometry().cluster_offset(first);
    assert_eq!(fs.disk().read(off, 10), b"still here");
    assert_eq!(fs.read_file("/SECRET.TXT"), Err(Error::NotFound));
}

#[test]
fn nested_directories_and_removal_rules() {
    let mut fs = default_fs();
    fs.create_dir("/A").unwrap();
    fs.create_dir("/A/B").unwrap();
    fs.create_dir("/A/B/C").unwrap();
    fs.create_file("/A/B/C/deep.txt", b"deep").unwrap();
    assert_eq!(fs.read_file("/a/b/c/DEEP.TXT").unwrap(), b"deep");
    assert_eq!(fs.remove_dir("/A").unwrap_err(), Error::DirectoryNotEmpty);
    fs.delete_file("/A/B/C/deep.txt").unwrap();
    fs.remove_dir("/A/B/C").unwrap();
    fs.remove_dir("/A/B").unwrap();
    fs.remove_dir("/A").unwrap();
    assert_eq!(fs.list_dir("/").unwrap(), Vec::new());
    assert_eq!(free_clusters(&fs), fs.geometry().cluster_count as usize);
}

#[test]
fn directory_growth_and_root_limits() {
    let mut fs = tiny_fs();
    fs.create_dir("/D").unwrap();
    for i in 0..40 {
        fs.create_file(&format!("/D/F{i}"), b"").unwrap();
    }
    assert_eq!(fs.list_dir("/D").unwrap().len(), 40);
    assert!(fs.cluster_chain(2).unwrap().len() >= 3);
    for i in 0..15 {
        fs.create_file(&format!("/R{i}"), b"").unwrap();
    }
    assert_eq!(fs.create_file("/R15", b"").unwrap_err(), Error::DirectoryFull);
}

#[test]
fn long_names_and_case_insensitivity() {
    let mut fs = default_fs();
    let long = "A very long file name that needs several LFN entries.txt";
    fs.create_file(&format!("/{long}"), b"long").unwrap();
    assert_eq!(fs.list_dir("/").unwrap()[0].name, long);
    assert_eq!(fs.read_file(&format!("/{}", long.to_uppercase())).unwrap(), b"long");
    assert_eq!(fs.read_file("/AVERYL~1.TXT").unwrap(), b"long");
    for i in 1..=3 {
        fs.create_file(&format!("/A very long name number {i}.txt"), b"").unwrap();
        assert!(fs.stat(&format!("/AVERYL~{}.TXT", i + 1)).is_ok());
    }
    fs.create_file("/héllo wörld.txt", b"u").unwrap();
    assert_eq!(fs.read_file("/HÉLLO WÖRLD.TXT").unwrap(), b"u");
}

#[test]
fn disk_full_leaks_nothing() {
    let mut fs = tiny_fs();
    let cs = fs.geometry().cluster_size();
    let free = free_clusters(&fs);
    fs.create_file("/A", &vec![1; (free - 2) * cs]).unwrap();
    assert_eq!(fs.create_file("/B", &vec![2; 3 * cs]).unwrap_err(), Error::DiskFull);
    assert_eq!(free_clusters(&fs), 2);
    assert_eq!(fs.list_dir("/").unwrap().len(), 1);
    fs.create_file("/B", &vec![2; 2 * cs]).unwrap();
    assert_eq!(free_clusters(&fs), 0);
}

#[test]
fn export_and_reimport_preserve_everything() {
    let mut fs = default_fs();
    fs.set_now(DateTime::new(2026, 9, 21, 15, 0, 0));
    fs.create_dir("/DOCS").unwrap();
    fs.create_file("/DOCS/Notes for later.txt", b"remember").unwrap();
    let image = fs.disk().as_bytes().to_vec();
    let again = FatFs::from_image(image).unwrap();
    assert_eq!(again.read_file("/DOCS/Notes for later.txt").unwrap(), b"remember");
    let info = again.stat("/DOCS/Notes for later.txt").unwrap();
    assert_eq!(info.modified, Some(DateTime::new(2026, 9, 21, 15, 0, 0)));
    assert!(again.history().is_empty());
}

#[test]
fn from_image_rejects_wrong_signature_and_fat12() {
    let fs = default_fs();
    let mut bad = fs.disk().as_bytes().to_vec();
    bad[510] = 0;
    assert!(matches!(FatFs::from_image(bad), Err(Error::CorruptImage(_))));
    let mut fat12 = tiny_fs().disk().as_bytes().to_vec();
    fat12[54..62].copy_from_slice(b"FAT12   ");
    assert!(matches!(FatFs::from_image(fat12), Err(Error::Unsupported(_))));
}

#[test]
fn every_mutating_op_records_changes_events_and_history() {
    let mut fs = default_fs();
    let records = vec![
        fs.create_dir("/D").unwrap(),
        fs.create_file("/D/F", b"1").unwrap(),
        fs.write_file("/D/F", b"22").unwrap(),
        fs.delete_file("/D/F").unwrap(),
        fs.remove_dir("/D").unwrap(),
    ];
    for r in &records {
        assert!(!r.changes.is_empty(), "{} recorded no byte changes", r.op);
        assert!(!r.events.is_empty(), "{} recorded no events", r.op);
        for e in &r.events {
            assert!(e.region().is_some());
            assert!(!e.to_string().is_empty());
        }
    }
    assert!(records[0].event_kinds().contains(&"cluster_allocated"));
    assert!(records[2].event_kinds().contains(&"cluster_freed"));
    assert!(records[3].event_kinds().contains(&"dir_entry_deleted"));
    assert_eq!(fs.history().len(), 5);
    assert_eq!(fs.history()[4].op, "remove_dir /D");
    assert!(fs.list_dir("/").is_ok());
    assert_eq!(fs.history().len(), 5);
    let sectors = records[1].changed_sectors(512);
    assert!(sectors.len() >= 3, "create_file should touch FAT, directory and data sectors");
}

#[test]
fn usable_as_a_trait_object() {
    let mut fs: Box<dyn FileSystem> = Box::new(default_fs());
    fs.create_file("/T", b"trait").unwrap();
    assert_eq!(fs.read_file("/T").unwrap(), b"trait");
    assert_eq!(fs.fs_type(), "FAT16");
}
```

`crates/fat/tests/mount_macos.rs`:
```rust
//! Manual validation: mount the exported image with macOS and read a file
//! back. Run with `cargo test -p fat --test mount_macos -- --ignored`.

use fat::{FatFs, FormatOptions};
use std::process::Command;

#[test]
#[ignore]
fn macos_mounts_the_image_and_reads_files() {
    let mut fs = FatFs::format(FormatOptions::default()).unwrap();
    fs.create_dir("/DOCS").unwrap();
    fs.create_file("/DOCS/Hello world.txt", b"hello from the emulator\n").unwrap();
    fs.create_file("/README.TXT", b"short name\n").unwrap();

    let dir = std::env::temp_dir().join(format!("fat16-emulator-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let image = dir.join("test.img");
    let mount = dir.join("mnt");
    std::fs::create_dir_all(&mount).unwrap();
    std::fs::write(&image, fs.disk().as_bytes()).unwrap();

    let attach = Command::new("hdiutil")
        .args(["attach", "-imagekey", "diskimage-class=CRawDiskImage", "-nobrowse", "-mountpoint"])
        .arg(&mount)
        .arg(&image)
        .output()
        .unwrap();
    assert!(attach.status.success(), "hdiutil attach failed: {}", String::from_utf8_lossy(&attach.stderr));

    let result = (|| {
        let hello = std::fs::read(mount.join("DOCS").join("Hello world.txt"))?;
        let readme = std::fs::read(mount.join("README.TXT"))?;
        Ok::<_, std::io::Error>((hello, readme))
    })();

    let detach = Command::new("hdiutil").arg("detach").arg(&mount).output().unwrap();
    assert!(detach.status.success(), "hdiutil detach failed: {}", String::from_utf8_lossy(&detach.stderr));
    let _ = std::fs::remove_dir_all(&dir);

    let (hello, readme) = result.unwrap();
    assert_eq!(hello, b"hello from the emulator\n");
    assert_eq!(readme, b"short name\n");
}
```

- [ ] **Step 2: Run the integration tests**

Run: `cargo test -p fat --test fat16`
Expected: 12 tests pass. If any fails, the failure is in library code from an earlier task; fix it there with a unit test reproducing it.

- [ ] **Step 3: Run the macOS mount check**

Run: `cargo test -p fat --test mount_macos -- --ignored`
Expected: passes, proving macOS mounts the image as FAT16 and reads both the short-named and the long-named file. If `hdiutil attach` refuses the image, compare the boot sector against the spec field list; the usual culprits are a cluster count outside the FAT16 range, or `total_sectors_16` and `total_sectors_32` both set.

- [ ] **Step 4: Confirm the WASM target builds**

Run: `cargo build --workspace --target wasm32-unknown-unknown`
Expected: builds with no errors. Any failure means a `std` feature that does not exist on that target crept in; remove it.

- [ ] **Step 5: Write the README**

`README.md`:
````markdown
# fs-emulator (written as fat16-emulator; renamed later)

A byte-accurate FAT16 filesystem on an in-memory virtual disk, written in Rust
as a learning tool. Every operation records exactly which bytes changed and
why, so a UI can show how the disk evolves. Exported images mount on macOS and
Linux.

## Crates

- `fs-core`: filesystem-agnostic core. The `Disk`, the change journal
  (`OpRecord`, `ByteChange`, `Event`), shared types, and the `FileSystem`
  trait. FAT32 and ext2/ext3 will plug in here later.
- `fat`: the FAT16 implementation (`FatFs`) plus FAT-specific inspection:
  `fat_entries`, `cluster_chain`, `raw_dir_entries`, `annotate_sector`.

## Example

```rust
use fat::{FatFs, FormatOptions};
use fs_core::FileSystem;

let mut fs = FatFs::format(FormatOptions::default()).unwrap();
fs.create_dir("/DOCS").unwrap();
let record = fs.create_file("/DOCS/Hello world.txt", b"hi").unwrap();
for event in &record.events {
    println!("{event}");
}
let image: &[u8] = fs.disk().as_bytes();
```

## Development

```
cargo test --workspace
cargo test -p fat --test mount_macos -- --ignored   # mounts the image with hdiutil
cargo build --workspace --target wasm32-unknown-unknown
```

Design: `docs/superpowers/specs/2026-09-21-fat16-emulator-design.md`.
````

- [ ] **Step 6: Final check and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets && cargo test --workspace`
Expected: clean, all pass.

```bash
git add README.md crates/fat/tests
git commit -m "test(fat): integration tests, macOS mount check, and README"
```
