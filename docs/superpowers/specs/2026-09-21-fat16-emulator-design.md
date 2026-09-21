# FAT16 Emulator Library — Design

*Project: fs-emulator (renamed from fat16-emulator after this spec was written).*

**Date:** 2026-09-21
**Status:** Approved

## Purpose

A Rust library that emulates a FAT16 filesystem on an in-memory virtual disk,
built as a learning tool. Every byte of the disk is a genuine FAT16 layout: the
exported image can be mounted by macOS or Linux. Each operation records exactly
which bytes changed and why, so a future browser UI (loading the library as
WASM) can show how the disk evolves over time.

The architecture is deliberately filesystem-agnostic at its core so that FAT32,
ext2 and ext3 can be added later without changing what the UI depends on.

## Non-goals for v1

- FAT12, FAT32, ext2, ext3 (but their seams are designed in; see below)
- Rename, append, truncate-to-size
- Undo (the change log stores before-bytes so undo can be added later)
- WASM bindings (a separate crate will wrap `fs-core` + `fat` when the UI is
  built)
- Any I/O, clock, or threading inside the library

## Workspace layout

```
fs-emulator/
  Cargo.toml              # workspace root
  crates/fs-core/         # filesystem-agnostic: disk, trace, trait, shared types
  crates/fat/             # FAT16 now; FAT32 later in the same crate
  crates/ext/             # future: ext2, then ext3 = ext2 + journal
  docs/superpowers/specs/ # design documents
```

Both crates have zero external dependencies and use `std` only for `Vec`,
`String`, `fmt`, and friends. Nothing in them requires I/O, `SystemTime`, or
threads, so they compile unchanged for `wasm32-unknown-unknown`.

## Architectural approach: layered, disk-is-truth

The `Disk` is a flat byte buffer addressed by sector. Every higher layer (FAT
table, directories, files) reads and writes through it. Nothing is cached in
parsed structs; the bytes are the only state. This keeps every change the UI
shows genuine, and change tracking is implemented once at the disk write path.

Rejected alternatives:

- **Parsed model with serialization.** Rust structs as the source of truth,
  rendered to bytes on demand. Simpler code, but the disk becomes a view rather
  than the thing being manipulated, which defeats the learning goal.
- **Pluggable `BlockDevice` trait with multiple backends.** Unneeded for an
  in-browser tool.

## Crate: `fs-core`

Owns everything that is the same for every filesystem.

### `disk`

```rust
pub struct Disk { bytes: Vec<u8>, sector_size: usize, open_op: Option<OpRecord> }

impl Disk {
    pub fn new(sector_size: usize, sector_count: u64) -> Disk;   // zero-filled
    pub fn from_bytes(sector_size: usize, bytes: Vec<u8>) -> Result<Disk>;
    pub fn sector_size(&self) -> usize;
    pub fn sector_count(&self) -> u64;
    pub fn len(&self) -> usize;
    pub fn as_bytes(&self) -> &[u8];              // the whole image
    pub fn sector(&self, n: u64) -> &[u8];
    pub fn read(&self, offset: usize, len: usize) -> &[u8];
    pub fn write(&mut self, offset: usize, data: &[u8]);   // records a ByteChange if an op is open
    pub fn fill(&mut self, offset: usize, len: usize, byte: u8);

    pub fn begin_op(&mut self, op: impl Into<String>);
    pub fn event(&mut self, e: Box<dyn Event>);   // attach to the open op
    pub fn end_op(&mut self) -> OpRecord;         // panics if no op is open (programmer error)
}
```

Writes outside the disk are a programmer error (panic); filesystem layers must
bounds-check against their own layout before writing. Sector size is
configurable so ext's 1 KiB to 4 KiB blocks map to N sectors.

### `trace`

```rust
pub struct OpRecord {
    pub op: String,                     // e.g. "create_file /DOCS/NOTES.TXT"
    pub changes: Vec<ByteChange>,       // in write order
    pub events: Vec<Box<dyn Event>>,    // in emission order
}

pub struct ByteChange { pub offset: usize, pub before: Vec<u8>, pub after: Vec<u8> }

pub trait Event: fmt::Display + fmt::Debug {
    fn kind(&self) -> &'static str;                 // stable machine label, e.g. "cluster_allocated"
    fn region(&self) -> Option<Range<usize>>;       // byte range on disk this event concerns, if any
}
```

Each filesystem defines its own typed event enum and implements `Event`. The
core never knows about clusters or inodes.

`ByteChange` stores the bytes before and after every write. This costs memory
proportional to total bytes written over the session, which is acceptable for a
learning tool with 16 MiB disks, and it keeps undo possible later.

### `layout` and `annotate`

```rust
pub struct Region { pub name: String, pub sectors: Range<u64>, pub kind: RegionKind }
pub enum RegionKind { Boot, Metadata, AllocationTable, Directory, Data, Reserved, Other }

pub struct Annotation { pub range: Range<usize>, pub label: String, pub value: String }
// range is relative to the start of the annotated sector
```

FAT reports regions reserved, FAT 0, FAT 1, root directory, data. ext2 will
report superblock, group descriptors, bitmaps, inode tables, data per block
group. `Annotation` lets any filesystem label any sector's bytes so a hex viewer
in the UI works identically for all of them.

### Shared types

```rust
pub struct DateTime { pub year: u16, pub month: u8, pub day: u8, pub hour: u8, pub minute: u8, pub second: u8 }
impl Default for DateTime  // 1980-01-01 00:00:00

pub struct EntryInfo {
    pub name: String,           // display name (LFN if present, else short name)
    pub is_dir: bool,
    pub size: u64,
    pub created: Option<DateTime>,
    pub modified: Option<DateTime>,
    pub accessed: Option<DateTime>,   // date only on FAT; time fields are zero
}

pub enum Error {
    NotFound, AlreadyExists, InvalidPath, InvalidName, DiskFull, DirectoryFull,
    NotADirectory, IsADirectory, DirectoryNotEmpty, FileTooLarge,
    InvalidGeometry(String), CorruptImage(String), Unsupported(String),
}
pub type Result<T> = core::result::Result<T, Error>;
```

`Error` implements `Display` and `std::error::Error` by hand (no `thiserror`).

### `path`

`parse(path: &str) -> Result<Vec<String>>`. Accepts `/` or `\` as separator,
requires a leading separator, collapses repeated separators, rejects empty
components, `.` and `..`. Returns components as given (case handling is the
filesystem's job). `/` parses to an empty vector meaning the root.

### `FileSystem` trait

```rust
pub trait FileSystem {
    fn fs_type(&self) -> &'static str;        // "FAT16", later "FAT32", "ext2"

    fn create_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord>;  // AlreadyExists if present
    fn write_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord>;   // overwrite; NotFound if absent
    fn read_file(&self, path: &str) -> Result<Vec<u8>>;
    fn delete_file(&mut self, path: &str) -> Result<OpRecord>;
    fn create_dir(&mut self, path: &str) -> Result<OpRecord>;
    fn remove_dir(&mut self, path: &str) -> Result<OpRecord>;                // DirectoryNotEmpty otherwise
    fn list_dir(&self, path: &str) -> Result<Vec<EntryInfo>>;               // excludes "." and ".."
    fn stat(&self, path: &str) -> Result<EntryInfo>;
    fn set_now(&mut self, now: DateTime);

    fn disk(&self) -> &Disk;
    fn layout(&self) -> Vec<Region>;
    fn annotate_sector(&self, sector: u64) -> Vec<Annotation>;
    fn history(&self) -> &[OpRecord];
}
```

The trait is what the UI programs against. Constructors (`format`,
`from_image`) are per-filesystem and live on the concrete types, since their
options differ.

## Crate: `fat`

### Modules

| Module | Responsibility |
|---|---|
| `boot_sector` | BPB parse/serialize, `FormatOptions`, derived `Geometry` (region offsets, cluster count, `FatVariant`). |
| `table` | FAT entry read/write dispatched on `FatVariant`, allocate/free clusters, follow chains, mirror to all FAT copies. |
| `dir_entry` | 32-byte short entry, attribute flags, FAT date/time packing, LFN entry layout and checksum. |
| `name` | 8.3 validation and case rules, long-name to short-name generation with `~N` tails, UCS-2 conversion. |
| `dir` | A directory as a slot sequence (fixed root region or cluster chain): iterate, find N contiguous free slots, grow by allocating a cluster. |
| `events` | `FatEvent` enum implementing `fs_core::Event`. |
| `fs` | `FatFs` implementing `FileSystem`, plus FAT-specific inspection methods. |

### FAT32 seams (designed in, not implemented)

What v1 actually dispatches on `FatVariant`:

- Cluster numbers are `u32` everywhere, including the high half of the short
  entry's first-cluster field.
- `enum FatVariant { Fat16 }` with FAT32 to be added. The FAT entry byte
  offset (`Geometry::fat_entry_offset`) and the entry codec in `table`
  (`decode`/`encode`) match on it, so a FAT32 arm cannot silently reuse the
  16-bit width.
- `format` and `from_image` return `Error::Unsupported` for anything but FAT16.

Still to be done when FAT32 lands (not dispatched in v1):

- `BootSector::parse` reads the FAT16 EBPB at offset 36 unconditionally; FAT32
  needs the common 36 bytes, then 28 extra fields, then the EBPB at 64.
- `dir::slots(DirLocation::Root)` assumes the fixed root region; FAT32 keeps
  the root in a cluster chain.
- End-of-chain and bad-cluster markers are FAT16 constants in `decode16`.

### `FormatOptions` and defaults

```rust
pub struct FormatOptions {
    pub bytes_per_sector: u16,       // 512
    pub sectors_per_cluster: u8,     // 4
    pub total_sectors: u32,          // 32768  (16 MiB)
    pub fat_count: u8,               // 2
    pub root_entries: u16,           // 512
    pub reserved_sectors: u16,       // 1
    pub volume_label: [u8; 11],      // b"NO NAME    "
    pub volume_id: u32,              // 0x1234_5678
    pub enforce_fat16_range: bool,   // true
}
```

FAT type is decided by cluster count: fewer than 4085 clusters is FAT12, 65525
or more is FAT32. With the defaults the data area has about 8100 clusters. When
`enforce_fat16_range` is true, `format` returns `InvalidGeometry` for a cluster
count outside `4085..65525`; setting it false permits tiny teaching disks whose
bytes are still laid out as FAT16.

`format` writes: boot sector (jump, OEM name `FAT16EMU`, BPB, EBPB with
signature `0x29`, label, `FAT16   ` type string, `0x55AA`), FAT copies with the
media descriptor entry `0xFFF8` and `0xFFFF` in entries 0 and 1, zeroed root
directory and data region.

### Names

- A name is written as a short entry only when it is already a valid 8.3 name
  after uppercasing: at most 8 base and 3 extension characters, ASCII, no
  characters from `" * + , / : ; < = > ? \ [ ] |`, no leading space or dot.
- Anything else gets a generated short name: strip invalid characters and
  spaces, uppercase, take the first 6 base characters, append `~N` with the
  smallest N (from 1) not already used in that directory, keep the first 3
  extension characters. If the base collapses to fewer than 6 characters,
  `~N` follows it directly.
- LFN entries are written immediately before the short entry in reverse
  order, 13 UCS-2 characters each, padded with `0xFFFF` after a terminating
  `0x0000`, with the standard checksum of the short name. The first-written
  (highest-numbered) entry has bit 6 of its sequence number set.
- Lookup is case-insensitive against both the long and the short name.
- On listing, LFN entries preceding a short entry are reassembled only if
  their checksums match and the sequence numbers are contiguous; otherwise
  they are ignored (orphaned LFN entries are shown in `raw_dir_entries`).

### Timestamps

`set_now` stores a `DateTime`. Creating an entry stamps create, modify and
access from it (FAT create time has 10 ms resolution in a separate byte, which
is set to 0; modify time has 2 s resolution; access is date only). Writing a
file updates modify and access. The default `DateTime` is 1980-01-01 so tests
are deterministic.

### `FatFs` public API beyond the trait

```rust
impl FatFs {
    pub fn format(opts: FormatOptions) -> Result<FatFs>;
    pub fn from_image(bytes: Vec<u8>) -> Result<FatFs>;   // validates signature, BPB, FAT type
    pub fn boot_sector(&self) -> BootSector;             // parsed BPB fields
    pub fn geometry(&self) -> &Geometry;
    pub fn fat_entries(&self, fat_index: u8) -> Vec<FatEntry>;   // Free | Next(u32) | EndOfChain | Bad | Reserved
    pub fn cluster_chain(&self, start: u32) -> Result<Vec<u32>>;   // CorruptImage on loop or out-of-range
    pub fn raw_dir_entries(&self, path: &str) -> Result<Vec<RawEntry>>;
}

pub enum RawEntry {
    Free,                                          // 0x00 first byte
    Deleted { bytes: [u8; 32] },                   // 0xE5 first byte
    Short(ShortEntry),                             // parsed
    Lfn(LfnEntry),                                 // parsed
}
```

### Events

```rust
pub enum FatEvent {
    ClusterAllocated { cluster: u32 },
    ClusterFreed { cluster: u32 },
    FatEntrySet { fat: u8, cluster: u32, value: FatEntry },
    DirEntryWritten { dir: String, slot: usize, kind: EntryKind },   // Short | Lfn
    DirEntryDeleted { dir: String, slot: usize },
    DataWritten { cluster: u32, bytes: usize },
    DirectoryGrown { dir: String, cluster: u32 },
}
```

`region()` returns the on-disk byte range: the FAT entry bytes, the 32-byte
slot, or the cluster's data.

### Operation semantics

- **create_file**: resolve parent (must be a directory), reject if any entry
  matches the name, allocate clusters for the data (none for an empty file),
  write data cluster by cluster, write LFN + short entries into the first run
  of free slots (growing a cluster-chain directory if needed; `DirectoryFull`
  if the fixed root is full). On `DiskFull` mid-way, free any clusters already
  allocated and leave no entry.
- **write_file**: free the old chain, then proceed as create but reuse the
  existing short entry slot, updating size, first cluster and modify/access
  stamps.
- **delete_file**: mark the short entry and its LFN entries `0xE5`, free the
  chain. Data bytes are left in place (which is what makes undelete tools
  work, and is worth showing in the UI).
- **create_dir**: allocate one cluster, zero it, write `.` and `..` entries
  (`..` first cluster is 0 when the parent is the root), then the entry in the
  parent with the directory attribute.
- **remove_dir**: `DirectoryNotEmpty` if any slot other than `.` and `..` is
  in use; otherwise as delete_file.
- Root directory: `stat("/")` reports a directory with size 0; `delete_file`,
  `remove_dir`, `create_dir("/")` are `InvalidPath`.
- `FileTooLarge` if data exceeds `u32::MAX` bytes or the free cluster count.

### Annotations

`annotate_sector` covers: the boot sector (every BPB/EBPB field), any FAT
sector (each entry with its cluster number and decoded value), any directory
sector (each 32-byte slot with its state and decoded fields), and data
sectors (which cluster they belong to and, when findable by walking the
directory tree, which file).

## Testing

**Unit tests** in each module: FAT date/time packing round trip, LFN checksum
against a known value, short-name generation cases (`~N` collisions, short
bases, stripped characters), 8.3 validation, path parsing, FAT entry
read/write and chain following, `Disk` change recording.

**Integration tests** in `crates/fat/tests`:

- format with defaults; boot sector fields, signature, FAT entries 0 and 1,
  cluster count in FAT16 range, `layout()` regions contiguous and covering
  the disk
- create, read, stat round trip for empty, one-cluster, multi-cluster, and
  exact-cluster-multiple files
- overwrite with a smaller and a larger file; old clusters are freed
- delete; entry is `0xE5`, chain freed, data bytes untouched
- nested directories: create, list, stat, remove; `DirectoryNotEmpty`
- directory growth beyond one cluster of entries
- root directory full returns `DirectoryFull`
- long name round trip, `~N` collision sequence, case-insensitive lookup
- `DiskFull` on a small volume, with no clusters leaked afterwards
- export `disk().as_bytes()`, re-import with `from_image`, read the same files
- `from_image` rejects a bad signature and a FAT12-sized volume
- every op returns an `OpRecord` whose `changes` are non-empty, and whose
  `events` include the expected kinds
- `history()` grows by one per mutating op

**Manual validation** (`#[ignore]` test): write the image to a temp file and
run `hdiutil attach -imagekey diskimage-class=CRawDiskImage` to confirm macOS
mounts it as MS-DOS (FAT16) and lists the files, then detach.

## Future work (out of scope)

- `crates/fat`: add `FatVariant::Fat32` (FSInfo sector, 28-bit entries,
  cluster-chain root)
- `crates/ext`: ext2 (superblock, block groups, inodes, bitmaps), then ext3
  journal
- rename, append, truncate
- undo derived from `ByteChange.before`
- `crates/wasm`: `wasm-bindgen` wrapper over the `FileSystem` trait
