# Terminal Access Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Embed the user's `@benjamin-small/browser-terminal` in the FAT explorer as a bottom drawer whose commands (`ls`, `cd`, `cat`, `write`, `dd`, `xxd`, ...) drive the emulated volume at `/mnt` and the raw disk at `/dev/hda`, with every write journaled so the hex dump, ribbon, timeline, and tree update like a form action.

**Architecture:** Raw disk writes become a first-class journaled operation (`fs_core::raw_write` → `FatFs::write_raw` with boot-sector re-parse and a `CorruptImage` gate → `Volume.writeRaw`/`readRaw`/`corruption()`). A pure TypeScript shell layer under `web/ui/src/shell/` (VFS, byte convention, dd, xxd, commands) talks to the stores through a narrow `ShellHost` seam, so it is unit-tested under node against the real wasm package. A Svelte drawer mounts the terminal with `create({ mount })` and registers the commands once.

**Tech Stack:** Rust 1.87 workspace (fs-core, fat, wasm via wasm-bindgen + serde-wasm-bindgen), Svelte 5 runes + TypeScript strict, Vite 6, Vitest 3, pnpm 10, `@benjamin-small/browser-terminal@0.2.0`, `@xterm/xterm@^6.0.0` (stylesheet only).

**Spec:** `docs/superpowers/specs/2026-09-22-terminal-access-design.md` (binding). Design record: `~/.claude/plans/ok-let-s-start-working-quiet-wren.md`.

## Global Constraints

- `crates/fs-core` and `crates/fat` keep zero external dependencies and no I/O, clock, or threads; everything builds for `wasm32-unknown-unknown`.
- `rust-version = "1.87"`; `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass at every commit.
- The wasm crate maps every core error to a unique `code` string; `OutOfBounds` is appended last.
- `web/ui`: Svelte 5 runes, TypeScript strict with `verbatimModuleSyntax`, Vite 6, Vitest 3 in the node environment loading the real wasm package; no UI or utility libraries beyond `@benjamin-small/browser-terminal@0.2.0` (exact) and `@xterm/xterm@^6.0.0`.
- Nothing under `web/ui/src/shell/` imports browser-terminal at runtime (type-only imports); `storeHost.svelte.ts` is the only shell file that touches the stores.
- The wasm package is consumed via `file:../../crates/wasm/pkg`, produced by `wasm-pack build crates/wasm --target bundler`; rebuild it after Task 2 before any web task.
- Every mutation from the shell goes through `ShellHost.run` (the store's `run`), never around it.
- Shell error messages never carry the command name (the engine prefixes it); `dd` and `cat` are capped at `DD_MAX_BYTES = 1 MiB` per invocation.
- Reads show the latest state and warn when the timeline is rewound; they never move the timeline.
- Commit messages are conventional (`feat`, `fix`, `test`, `docs`, `chore`) with no attribution lines. Specs are binding; where a task's text and the spec differ, the spec wins.

---

### Task 1: Rust core: journaled raw writes

Spec: `docs/superpowers/specs/2026-09-22-terminal-access-design.md`, section "1. Rust core: journaled raw writes" and the fs-core/fat bullets under "Testing". Everything below was compiled and run against the current tree (all gates green), so copy the code verbatim.

**Files:**
- Modify: `crates/fs-core/src/error.rs` (variant after line 18, Display arm after line 38, test at lines 50-56)
- Modify: `crates/fs-core/src/trace.rs` (new struct after line 22, new test module at end of file)
- Modify: `crates/fs-core/src/disk.rs` (imports lines 4-5, free functions after line 116, tests before line 225)
- Modify: `crates/fs-core/src/fs.rs` (trait method after line 24, `NullFs` at lines 37-97)
- Modify: `crates/fs-core/src/lib.rs` (lines 12 and 16)
- Modify: `crates/wasm/src/error.rs` (one match arm after line 21; without it the workspace stops compiling because `code_of` matches `fs_core::Error` exhaustively)
- Modify: `crates/fat/src/boot_sector.rs` (constant after line 6, `parse` at lines 174-230 split into `decode` + `parse`, test after line 447)
- Modify: `crates/fat/src/fs.rs` (import line 4, struct lines 18-24, constructors lines 37-43 and 65-71, accessor after line 80, three private/public methods after `run_op` line 151, gates in nine methods, `annotate_boot_sector` lines 677-724, trait forwarding after line 949, tests at end of the `mod tests` block line 1878)
- Test: `crates/fat/tests/fat16.rs` (extend lines 196-217, append four tests after line 231)

**Interfaces:**
- Consumes (existing): `fs_core::Disk { new, len, sector_size, sector_count, read, write, begin_op, end_op, event, op_open, as_bytes }`, `fs_core::trace::{Event, OpRecord, ByteChange}`, `fs_core::Error`, `fat::FatFs::run_op(&mut self, op: String, body: impl FnOnce(&mut Self) -> Result<()>) -> Result<OpRecord>` (private, `crates/fat/src/fs.rs:134`), `fat::BootSector::{parse, geometry, to_bytes, from_options}`, `fat::Geometry { bytes_per_sector: usize, total_sectors: u64, .. }`.
- Produces:
  - `fs_core::Error::OutOfBounds { offset: u64, len: u64, disk_len: u64 }` (last variant; Display `out of bounds: {len} bytes at offset {offset} run past the end of the {disk_len}-byte disk`)
  - `fs_core::trace::RawWrite { pub offset: usize, pub len: usize }` implementing `Event` (kind `"raw_write"`, region `offset..offset+len`, Display `wrote {len} raw bytes at 0x{offset:x}`); re-exported as `fs_core::RawWrite`
  - `pub fn fs_core::disk::raw_write_op(offset: u64, len: usize) -> String` (`write_raw 0x{offset:x} +{len}`); re-exported as `fs_core::raw_write_op`
  - `pub fn fs_core::disk::raw_write(disk: &mut Disk, offset: u64, bytes: &[u8]) -> Result<Range<usize>>`; re-exported as `fs_core::raw_write`
  - `fs_core::FileSystem::write_raw(&mut self, offset: u64, bytes: &[u8]) -> Result<OpRecord>` (required trait method)
  - `pub const fat::boot_sector::BOOT_SECTOR_LEN: usize = 512`
  - `fat::BootSector::decode(bytes: &[u8]) -> BootSector`
  - `fat::FatFs::corruption(&self) -> Option<&fs_core::Error>`
  - `fat::FatFs::write_raw(&mut self, offset: u64, bytes: &[u8]) -> Result<OpRecord>` (inherent, plus the trait forwarding)
  - `code_of(&Error::OutOfBounds { .. }) == "OutOfBounds"` in `crates/wasm/src/error.rs` (task 2 adds its test and the `writeRaw`/`readRaw`/`corruption` exports)

Gate commands used throughout (run from the repository root):

```
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --target wasm32-unknown-unknown
```

---

- [ ] **Step 1: Write the failing `OutOfBounds` Display test**

In `crates/fs-core/src/error.rs`, inside `display_is_human_readable` (lines 50-56), replace:

```rust
        assert_eq!(
            Error::InvalidGeometry("too small".into()).to_string(),
            "invalid geometry: too small"
        );
    }
```

with:

```rust
        assert_eq!(
            Error::InvalidGeometry("too small".into()).to_string(),
            "invalid geometry: too small"
        );
        assert_eq!(
            Error::OutOfBounds {
                offset: 3,
                len: 2,
                disk_len: 4
            }
            .to_string(),
            "out of bounds: 2 bytes at offset 3 run past the end of the 4-byte disk"
        );
    }
```

- [ ] **Step 2: Run it and confirm the failure**

Run: `cargo test -p fs-core display_is_human_readable`

Expected: compile error `error[E0599]: no variant named `OutOfBounds` found for enum `error::Error``.

- [ ] **Step 3: Add the variant, its Display arm, and the wasm code string**

`crates/fs-core/src/error.rs`, the enum (lines 16-19). Replace:

```rust
    InvalidGeometry(String),
    CorruptImage(String),
    Unsupported(String),
}
```

with:

```rust
    InvalidGeometry(String),
    CorruptImage(String),
    Unsupported(String),
    /// A raw byte range that does not lie inside the disk.
    OutOfBounds {
        offset: u64,
        len: u64,
        disk_len: u64,
    },
}
```

Same file, the Display `match` (line 38). Replace:

```rust
            Error::Unsupported(msg) => write!(f, "unsupported: {msg}"),
        }
```

with:

```rust
            Error::Unsupported(msg) => write!(f, "unsupported: {msg}"),
            Error::OutOfBounds {
                offset,
                len,
                disk_len,
            } => write!(
                f,
                "out of bounds: {len} bytes at offset {offset} run past the end of the {disk_len}-byte disk"
            ),
        }
```

`crates/wasm/src/error.rs`, in `code_of` (line 21). Replace:

```rust
        Unsupported(_) => "Unsupported",
    }
```

with:

```rust
        Unsupported(_) => "Unsupported",
        OutOfBounds { .. } => "OutOfBounds",
    }
```

(The existing wasm test `every_variant_has_its_own_code` keeps passing; task 2 extends it with the new variant.)

- [ ] **Step 4: Run the tests and the workspace build**

Run: `cargo test -p fs-core && cargo check --workspace --all-targets`

Expected: `display_is_human_readable ... ok`, 17 fs-core unit tests pass, and the workspace check finishes with no errors.

- [ ] **Step 5: Commit**

```
git add crates/fs-core/src/error.rs crates/wasm/src/error.rs
git commit -m "feat(fs-core): add Error::OutOfBounds for raw byte ranges past the disk"
```

---

- [ ] **Step 6: Write the failing `RawWrite` event tests**

Append to the end of `crates/fs-core/src/trace.rs` (after the closing `}` of `impl OpRecord`, line 70):

```rust

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_write_event_has_kind_region_text_and_clones() {
        let event = RawWrite {
            offset: 512,
            len: 3,
        };
        assert_eq!(event.kind(), "raw_write");
        assert_eq!(event.region(), Some(512..515));
        assert_eq!(event.to_string(), "wrote 3 raw bytes at 0x200");
        let boxed: Box<dyn Event> = Box::new(event.clone());
        let copy = boxed.clone();
        assert_eq!(copy.kind(), "raw_write");
        assert_eq!(copy.region(), Some(512..515));
        let mut record = OpRecord::new("write_raw 0x200 +3");
        record.events.push(boxed);
        assert_eq!(record.event_kinds(), vec!["raw_write"]);
    }

    #[test]
    fn empty_raw_write_has_an_empty_region() {
        let event = RawWrite { offset: 4, len: 0 };
        assert_eq!(event.region(), Some(4..4));
        assert_eq!(event.to_string(), "wrote 0 raw bytes at 0x4");
    }
}
```

- [ ] **Step 7: Run them and confirm the failure**

Run: `cargo test -p fs-core trace::`

Expected: compile error `error[E0422]: cannot find struct, variant or union type `RawWrite` in this scope`.

- [ ] **Step 8: Implement `RawWrite`**

In `crates/fs-core/src/trace.rs`, after the `impl Clone for Box<dyn Event>` block (line 22) and before the `ByteChange` doc comment, insert:

```rust

/// The one event a filesystem-agnostic raw write reports: `len` bytes
/// written at absolute `offset`. Kind `raw_write`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawWrite {
    pub offset: usize,
    pub len: usize,
}

impl fmt::Display for RawWrite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "wrote {} raw bytes at 0x{:x}", self.len, self.offset)
    }
}

impl Event for RawWrite {
    fn kind(&self) -> &'static str {
        "raw_write"
    }
    fn region(&self) -> Option<Range<usize>> {
        Some(self.offset..self.offset + self.len)
    }
    fn clone_box(&self) -> Box<dyn Event> {
        Box::new(self.clone())
    }
}
```

In `crates/fs-core/src/lib.rs` (line 16) replace:

```rust
pub use trace::{ByteChange, Event, OpRecord};
```

with:

```rust
pub use trace::{ByteChange, Event, OpRecord, RawWrite};
```

- [ ] **Step 9: Run the tests**

Run: `cargo test -p fs-core`

Expected: `trace::tests::raw_write_event_has_kind_region_text_and_clones ... ok`, `trace::tests::empty_raw_write_has_an_empty_region ... ok`, 19 tests pass.

- [ ] **Step 10: Commit**

```
git add crates/fs-core/src/trace.rs crates/fs-core/src/lib.rs
git commit -m "feat(fs-core): add the RawWrite event"
```

---

- [ ] **Step 11: Write the failing `raw_write` / `raw_write_op` tests**

In `crates/fs-core/src/disk.rs`, inside `mod tests`, insert the following between the end of `sector_returns_the_right_slice` (line 223) and `#[test] fn op_record_is_cloneable()` (line 225):

```rust

    #[test]
    fn raw_write_op_formats_lowercase_hex_and_length() {
        assert_eq!(raw_write_op(512, 3), "write_raw 0x200 +3");
        assert_eq!(raw_write_op(0, 0), "write_raw 0x0 +0");
        assert_eq!(raw_write_op(0xABCD, 16), "write_raw 0xabcd +16");
    }

    #[test]
    fn raw_write_inside_an_op_records_change_and_event() {
        let mut disk = Disk::new(512, 2);
        disk.begin_op("t");
        assert_eq!(raw_write(&mut disk, 512, &[1, 2, 3]), Ok(512..515));
        let record = disk.end_op();
        assert_eq!(
            record.changes,
            vec![ByteChange {
                offset: 512,
                before: vec![0, 0, 0],
                after: vec![1, 2, 3]
            }]
        );
        assert_eq!(record.event_kinds(), vec!["raw_write"]);
        assert_eq!(record.events[0].region(), Some(512..515));
        assert_eq!(record.events[0].to_string(), "wrote 3 raw bytes at 0x200");
        assert_eq!(record.changed_sectors(512), vec![1]);
        assert_eq!(disk.read(512, 3), &[1, 2, 3]);
    }

    #[test]
    fn raw_write_outside_an_op_changes_bytes_without_recording() {
        let mut disk = Disk::new(512, 1);
        assert_eq!(raw_write(&mut disk, 5, &[9, 9]), Ok(5..7));
        assert_eq!(disk.read(5, 2), &[9, 9]);
        assert!(!disk.op_open());
    }

    #[test]
    fn raw_write_rejects_ranges_past_the_end_without_touching_the_disk() {
        let mut disk = Disk::new(4, 1);
        assert_eq!(
            raw_write(&mut disk, 3, &[1, 2]),
            Err(Error::OutOfBounds {
                offset: 3,
                len: 2,
                disk_len: 4
            })
        );
        assert_eq!(
            raw_write(&mut disk, 4, &[1]),
            Err(Error::OutOfBounds {
                offset: 4,
                len: 1,
                disk_len: 4
            })
        );
        assert_eq!(
            raw_write(&mut disk, u64::MAX, &[1]),
            Err(Error::OutOfBounds {
                offset: u64::MAX,
                len: 1,
                disk_len: 4
            })
        );
        assert_eq!(
            raw_write(&mut disk, u64::MAX, &[]),
            Err(Error::OutOfBounds {
                offset: u64::MAX,
                len: 0,
                disk_len: 4
            })
        );
        assert!(disk.as_bytes().iter().all(|&b| b == 0));
        disk.begin_op("t");
        assert!(raw_write(&mut disk, 3, &[1, 2]).is_err());
        let record = disk.end_op();
        assert!(record.changes.is_empty());
        assert!(record.events.is_empty());
    }

    #[test]
    fn raw_write_of_zero_bytes_is_bounds_checked_but_records_nothing() {
        let mut disk = Disk::new(4, 1);
        assert_eq!(raw_write(&mut disk, 4, &[]), Ok(4..4));
        assert_eq!(raw_write(&mut disk, 1, &[]), Ok(1..1));
        assert_eq!(
            raw_write(&mut disk, 5, &[]),
            Err(Error::OutOfBounds {
                offset: 5,
                len: 0,
                disk_len: 4
            })
        );
        disk.begin_op("t");
        assert_eq!(raw_write(&mut disk, 2, &[]), Ok(2..2));
        let record = disk.end_op();
        assert!(record.changes.is_empty());
        assert!(record.events.is_empty());
    }
```

(`Error`, `ByteChange`, `Event` and `Range` are already imported by the test module via `use super::*;`, `use crate::trace::{ByteChange, Event};` and `use std::ops::Range;`.)

- [ ] **Step 12: Run them and confirm the failure**

Run: `cargo test -p fs-core disk::`

Expected: compile errors `error[E0425]: cannot find function `raw_write_op` in this scope` and `error[E0425]: cannot find function `raw_write` in this scope`.

- [ ] **Step 13: Implement the free helpers**

`crates/fs-core/src/disk.rs`, imports (lines 4-5). Replace:

```rust
use crate::trace::{ByteChange, Event, OpRecord};
use crate::{Error, Result};
```

with:

```rust
use crate::trace::{ByteChange, Event, OpRecord, RawWrite};
use crate::{Error, Result};
use std::ops::Range;
```

Same file, after the closing `}` of `impl Disk` (line 116) and before `#[cfg(test)]`, insert:

```rust

/// The op name every filesystem uses for a raw write, e.g. `write_raw 0x200 +512`.
pub fn raw_write_op(offset: u64, len: usize) -> String {
    format!("write_raw 0x{offset:x} +{len}")
}

/// Bounds-check, then write `bytes` at `offset` and report one `RawWrite`
/// event. Journaled only if the caller has an operation open; the caller
/// (a filesystem's `run_op`) owns begin/end and history. Nothing is written
/// on `OutOfBounds`, so a rollback has nothing to undo. Empty `bytes` is
/// bounds-checked but records no `ByteChange` and no event. Returns the
/// absolute byte range written.
pub fn raw_write(disk: &mut Disk, offset: u64, bytes: &[u8]) -> Result<Range<usize>> {
    let disk_len = disk.len() as u64;
    let len = bytes.len() as u64;
    let Some(end) = offset.checked_add(len).filter(|&e| e <= disk_len) else {
        return Err(Error::OutOfBounds {
            offset,
            len,
            disk_len,
        });
    };
    // Both are <= disk.len(), which fits in usize by construction.
    let (start, end) = (offset as usize, end as usize);
    if bytes.is_empty() {
        return Ok(start..start);
    }
    disk.write(start, bytes);
    disk.event(Box::new(RawWrite {
        offset: start,
        len: bytes.len(),
    }));
    Ok(start..end)
}
```

`crates/fs-core/src/lib.rs` (line 12). Replace:

```rust
pub use disk::Disk;
```

with:

```rust
pub use disk::{raw_write, raw_write_op, Disk};
```

- [ ] **Step 14: Run the tests and clippy**

Run: `cargo test -p fs-core && cargo clippy -p fs-core --all-targets -- -D warnings`

Expected: 24 fs-core unit tests pass, including the five new `disk::tests::raw_write_*` tests; clippy is clean (the `let ... else` and `checked_add` forms are stable on 1.87 and `std::ops::Range` is used by `raw_write` outside tests, so no unused-import warning).

- [ ] **Step 15: Commit**

```
git add crates/fs-core/src/disk.rs crates/fs-core/src/lib.rs
git commit -m "feat(fs-core): add bounds-checked, journaled raw_write helper"
```

---

- [ ] **Step 16: Write the failing `BootSector::decode` test**

In `crates/fat/src/boot_sector.rs`, inside `mod tests`, after `to_bytes_and_parse_round_trip` (ends line 447) and before `#[test] fn parse_rejects_bad_signature()` (line 449), insert:

```rust

    #[test]
    fn decode_reads_fields_without_validating() {
        let zeros = BootSector::decode(&[0u8; BOOT_SECTOR_LEN]);
        assert_eq!(zeros.bytes_per_sector, 0);
        assert_eq!(zeros.sectors_per_cluster, 0);
        assert_eq!(zeros.volume_label, [0u8; 11]);
        let bs = BootSector::from_options(&FormatOptions::default()).unwrap();
        let bytes = bs.to_bytes();
        assert_eq!(BootSector::decode(&bytes), bs);
        assert_eq!(
            BootSector::decode(&bytes),
            BootSector::parse(&bytes).unwrap()
        );
        let mut unsigned = bytes.clone();
        unsigned[510] = 0;
        assert_eq!(BootSector::decode(&unsigned), bs);
        assert!(BootSector::parse(&unsigned).is_err());
    }
```

- [ ] **Step 17: Run it and confirm the failure**

Run: `cargo test -p fat decode_reads_fields`

Expected: compile errors `error[E0425]: cannot find value `BOOT_SECTOR_LEN` in this scope` and `error[E0599]: no function or associated item named `decode` found for struct `BootSector``.

- [ ] **Step 18: Implement `decode` and `BOOT_SECTOR_LEN`**

`crates/fat/src/boot_sector.rs`, line 6. Replace:

```rust
pub const SIGNATURE: [u8; 2] = [0x55, 0xAA];
```

with:

```rust
pub const SIGNATURE: [u8; 2] = [0x55, 0xAA];
/// How many bytes `BootSector::parse` and `decode` read: the BPB, EBPB and
/// signature all lie inside the first 512 bytes whatever the sector size.
pub const BOOT_SECTOR_LEN: usize = 512;
```

Same file, lines 174-211 (the head of `parse` up to and including the `let bs = BootSector { ... };` literal). Replace:

```rust
    /// Parse the first 512 bytes of a volume.
    pub fn parse(bytes: &[u8]) -> Result<BootSector> {
        if bytes.len() < 512 {
            return Err(Error::CorruptImage(
                "boot sector is shorter than 512 bytes".into(),
            ));
        }
        if bytes[510..512] != SIGNATURE {
            return Err(Error::CorruptImage(
                "boot sector signature is not 55 AA".into(),
            ));
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
```

with:

```rust
    /// Read the BPB/EBPB fields of the first `BOOT_SECTOR_LEN` bytes without
    /// validating any of them, so callers can describe whatever is on disk.
    /// Panics if `bytes` is shorter than `BOOT_SECTOR_LEN`.
    pub fn decode(bytes: &[u8]) -> BootSector {
        let mut oem_name = [0u8; 8];
        oem_name.copy_from_slice(&bytes[3..11]);
        let mut volume_label = [0u8; 11];
        volume_label.copy_from_slice(&bytes[43..54]);
        let mut fs_type = [0u8; 8];
        fs_type.copy_from_slice(&bytes[54..62]);
        BootSector {
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
        }
    }

    /// Parse and validate the first `BOOT_SECTOR_LEN` bytes of a volume.
    pub fn parse(bytes: &[u8]) -> Result<BootSector> {
        if bytes.len() < BOOT_SECTOR_LEN {
            return Err(Error::CorruptImage(
                "boot sector is shorter than 512 bytes".into(),
            ));
        }
        if bytes[510..512] != SIGNATURE {
            return Err(Error::CorruptImage(
                "boot sector signature is not 55 AA".into(),
            ));
        }
        let bs = Self::decode(bytes);
        if ![512, 1024, 2048, 4096].contains(&bs.bytes_per_sector) {
```

The three validation blocks after that line and the final `Ok(bs)` stay exactly as they are.

- [ ] **Step 19: Run the tests**

Run: `cargo test -p fat`

Expected: `boot_sector::tests::decode_reads_fields_without_validating ... ok`; 75 fat unit tests and 12 `fat16` integration tests pass (the ignored `mount_macos` test stays ignored).

- [ ] **Step 20: Commit**

```
git add crates/fat/src/boot_sector.rs
git commit -m "feat(fat): split BootSector::decode from parse and name BOOT_SECTOR_LEN"
```

---

- [ ] **Step 21: Write the failing trait, `FatFs`, and integration tests**

This step and Step 23 land together: adding the required trait method makes `crates/fat` (and therefore the wasm crate) fail to compile until `FatFs` implements it, so the workspace is only green again after Step 23.

**(a) `crates/fs-core/src/fs.rs`**, the test module. Replace the `NullFs` struct (lines 37-39):

```rust
    struct NullFs {
        disk: Disk,
    }
```

with:

```rust
    struct NullFs {
        disk: Disk,
        history: Vec<OpRecord>,
    }
```

Replace (lines 69-72):

```rust
        fn set_now(&mut self, _: DateTime) {}
        fn disk(&self) -> &Disk {
            &self.disk
        }
```

with:

```rust
        fn set_now(&mut self, _: DateTime) {}
        fn write_raw(&mut self, offset: u64, bytes: &[u8]) -> Result<OpRecord> {
            self.disk.begin_op(crate::raw_write_op(offset, bytes.len()));
            let written = crate::raw_write(&mut self.disk, offset, bytes);
            let record = self.disk.end_op();
            written?;
            self.history.push(record.clone());
            Ok(record)
        }
        fn disk(&self) -> &Disk {
            &self.disk
        }
```

Replace (lines 84-97):

```rust
        fn history(&self) -> &[OpRecord] {
            &[]
        }
    }

    #[test]
    fn trait_is_object_safe() {
        let fs: Box<dyn FileSystem> = Box::new(NullFs {
            disk: Disk::new(512, 1),
        });
        assert_eq!(fs.fs_type(), "null");
        assert_eq!(fs.layout()[0].kind, crate::layout::RegionKind::Other);
        assert_eq!(fs.list_dir("/").unwrap(), Vec::new());
    }
```

with:

```rust
        fn history(&self) -> &[OpRecord] {
            &self.history
        }
    }

    #[test]
    fn trait_is_object_safe() {
        let fs: Box<dyn FileSystem> = Box::new(NullFs {
            disk: Disk::new(512, 1),
            history: Vec::new(),
        });
        assert_eq!(fs.fs_type(), "null");
        assert_eq!(fs.layout()[0].kind, crate::layout::RegionKind::Other);
        assert_eq!(fs.list_dir("/").unwrap(), Vec::new());
    }

    #[test]
    fn write_raw_is_journaled_through_the_trait() {
        let mut fs: Box<dyn FileSystem> = Box::new(NullFs {
            disk: Disk::new(512, 1),
            history: Vec::new(),
        });
        let record = fs.write_raw(0, &[9]).unwrap();
        assert_eq!(record.op, "write_raw 0x0 +1");
        assert_eq!(record.event_kinds(), vec!["raw_write"]);
        assert_eq!(fs.history().len(), 1);
        assert_eq!(fs.disk().read(0, 1), &[9]);
        assert_eq!(
            fs.write_raw(512, &[1]).unwrap_err(),
            Error::OutOfBounds {
                offset: 512,
                len: 1,
                disk_len: 512
            }
        );
        assert_eq!(fs.history().len(), 1);
        assert!(!fs.disk().op_open());
    }
```

(`OpRecord` does not derive `PartialEq`, so compare with `unwrap_err()` rather than `assert_eq!(result, Err(..))`.)

**(b) `crates/fat/src/fs.rs`**, end of `mod tests`. Replace the tail of `works_through_the_trait_object` and the module's closing brace (lines 1876-1879):

```rust
        fs.set_now(DateTime::new(2000, 1, 1, 0, 0, 0));
        assert!(!fs.annotate_sector(0).is_empty());
    }
}
```

with:

```rust
        fs.set_now(DateTime::new(2000, 1, 1, 0, 0, 0));
        assert!(!fs.annotate_sector(0).is_empty());
        let rec = fs.write_raw(0x2000, b"x").unwrap();
        assert_eq!(rec.op, "write_raw 0x2000 +1");
        assert_eq!(fs.history().len(), 6);
    }

    #[test]
    fn write_raw_journals_and_appears_in_history() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let rec = fs.write_raw(0x1000, b"hello").unwrap();
        assert_eq!(rec.op, "write_raw 0x1000 +5");
        assert_eq!(
            rec.changes,
            vec![fs_core::ByteChange {
                offset: 0x1000,
                before: vec![0; 5],
                after: b"hello".to_vec(),
            }]
        );
        assert_eq!(rec.event_kinds(), vec!["raw_write"]);
        assert_eq!(rec.events[0].region(), Some(0x1000..0x1005));
        assert_eq!(fs.history().len(), 1);
        assert_eq!(fs.history()[0].op, "write_raw 0x1000 +5");
        assert_eq!(fs.disk().read(0x1000, 5), b"hello");
        assert!(fs.corruption().is_none());
        assert!(!fs.disk().op_open());
    }

    #[test]
    fn write_raw_out_of_bounds_leaves_no_history_and_no_open_op() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let len = fs.disk().len() as u64;
        assert_eq!(
            fs.write_raw(len - 1, &[1, 2]).unwrap_err(),
            Error::OutOfBounds {
                offset: len - 1,
                len: 2,
                disk_len: len
            }
        );
        assert!(matches!(
            fs.write_raw(u64::MAX, &[1]),
            Err(Error::OutOfBounds { .. })
        ));
        assert!(fs.history().is_empty());
        assert!(!fs.disk().op_open());
        assert_eq!(fs.disk().read(len as usize - 1, 1), &[0]);
        fs.create_file("/A", b"").unwrap();
        assert_eq!(fs.history().len(), 1);
    }

    #[test]
    fn write_raw_to_the_boot_sector_that_still_parses_is_adopted() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        let geo_before = fs.geometry().clone();
        let rec = fs.write_raw(43, b"RAWLABEL   ").unwrap();
        assert_eq!(rec.op, "write_raw 0x2b +11");
        assert_eq!(fs.boot_sector().volume_label, *b"RAWLABEL   ");
        assert_eq!(fs.geometry(), &geo_before);
        assert!(fs.corruption().is_none());
        let label = fs
            .annotate_sector(0)
            .into_iter()
            .find(|a| a.label == "volume label")
            .unwrap();
        assert_eq!(label.value, "RAWLABEL   ");
        assert!(fs.list_dir("/").is_ok());
    }

    #[test]
    fn write_raw_that_changes_geometry_is_adopted_and_layout_follows() {
        let mut fs = tiny_fs();
        let before = fs.geometry().first_data_sector;
        assert_eq!(fs.layout()[3].name, "root directory");
        assert_eq!(fs.layout()[3].sectors.end, before);
        fs.write_raw(17, &32u16.to_le_bytes()).unwrap();
        assert!(fs.corruption().is_none());
        assert_eq!(fs.boot_sector().root_entries, 32);
        assert_eq!(fs.geometry().root_entries, 32);
        assert_eq!(fs.geometry().first_data_sector, before + 1);
        assert_eq!(fs.layout()[3].sectors.end, before + 1);
        assert_eq!(fs.layout()[4].sectors.start, before + 1);
        assert_eq!(fs.raw_dir_entries("/").unwrap().len(), 32);
    }

    #[test]
    fn write_raw_that_breaks_the_signature_unmounts_until_repaired() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        fs.create_file("/A", b"a").unwrap();
        let layout_before = fs.layout();
        let geo_before = fs.geometry().clone();
        fs.write_raw(510, &[0, 0]).unwrap();
        assert!(matches!(fs.corruption(), Some(Error::CorruptImage(_))));
        let gate = "boot sector no longer parses after a raw write";
        let results: Vec<Result<()>> = vec![
            fs.list_dir("/").map(|_| ()),
            fs.stat("/A").map(|_| ()),
            fs.read_file("/A").map(|_| ()),
            fs.raw_dir_entries("/").map(|_| ()),
            fs.create_file("/B", b"").map(|_| ()),
            fs.write_file("/A", b"x").map(|_| ()),
            fs.delete_file("/A").map(|_| ()),
            fs.create_dir("/D").map(|_| ()),
            fs.remove_dir("/A").map(|_| ()),
        ];
        for r in results {
            match r {
                Err(Error::CorruptImage(m)) => assert!(m.starts_with(gate), "message was {m}"),
                other => panic!("expected the corruption gate, got {other:?}"),
            }
        }
        assert_eq!(fs.layout(), layout_before);
        assert_eq!(fs.geometry(), &geo_before);
        let boot = fs.annotate_sector(0);
        assert_eq!(boot.last().unwrap().label, "boot signature");
        assert_eq!(boot.last().unwrap().value, "00 00");
        let data = fs.annotate_sector(geo_before.first_data_sector);
        assert_eq!(data[0].value, "data of /A");
        assert_eq!(fs.disk().read(510, 2), &[0, 0]);
        assert_eq!(fs.history().len(), 2);
        fs.write_raw(510, &[0x55, 0xAA]).unwrap();
        assert!(fs.corruption().is_none());
        assert_eq!(fs.read_file("/A").unwrap(), b"a");
        assert_eq!(fs.annotate_sector(0).last().unwrap().value, "55 AA");
        assert_eq!(fs.history().len(), 3);
    }

    #[test]
    fn write_raw_with_a_geometry_that_does_not_fit_the_disk_is_corrupt() {
        let mut fs = FatFs::format(FormatOptions::default()).unwrap();
        fs.write_raw(11, &1024u16.to_le_bytes()).unwrap();
        let msg = fs.corruption().unwrap().to_string();
        assert!(
            msg.contains("says 1024 bytes per sector"),
            "message was {msg}"
        );
        assert_eq!(fs.geometry().bytes_per_sector, 512);
        fs.write_raw(11, &512u16.to_le_bytes()).unwrap();
        assert!(fs.corruption().is_none());
        // 32800 sectors still yields a FAT16 geometry the FAT can hold, but
        // the disk only has 32768.
        fs.write_raw(19, &32800u16.to_le_bytes()).unwrap();
        let msg = fs.corruption().unwrap().to_string();
        assert!(
            msg.contains("describes 32800 sectors but the disk has 32768"),
            "message was {msg}"
        );
        assert_eq!(fs.geometry().total_sectors, 32768);
        assert!(matches!(fs.list_dir("/"), Err(Error::CorruptImage(_))));
        fs.write_raw(19, &32768u16.to_le_bytes()).unwrap();
        assert!(fs.corruption().is_none());
        assert!(fs.list_dir("/").is_ok());
        assert_eq!(fs.history().len(), 4);
    }

    #[test]
    fn write_raw_reparses_on_the_512_byte_boundary_not_the_sector_size() {
        let mut fs = FatFs::format(FormatOptions {
            bytes_per_sector: 4096,
            sectors_per_cluster: 1,
            total_sectors: 8192,
            ..Default::default()
        })
        .unwrap();
        let boot_before = fs.boot_sector().clone();
        fs.write_raw(600, &[1]).unwrap();
        assert!(fs.corruption().is_none());
        assert_eq!(fs.boot_sector(), &boot_before);
        fs.write_raw(300, &[1]).unwrap();
        assert!(fs.corruption().is_none());
        assert_eq!(fs.boot_sector(), &boot_before);
        fs.write_raw(511, &[0]).unwrap();
        assert!(fs.corruption().is_some());
        fs.write_raw(511, &[0xAA]).unwrap();
        assert!(fs.corruption().is_none());
    }
}
```

Notes on why these numbers are right: `tiny_fs()` (128 sectors, 1 sector per cluster, 16 root entries, 2 FATs of 1 sector) has layout `[boot 0..1, FAT 0 1..2, FAT 1 2..3, root 3..4, data 4..128]`, so index 3 is the root region and 32 root entries add one root sector. On the default volume, `total_sectors_16 = 65535` would fail `geometry()`'s FAT-size check (32 sectors of FAT hold 8192 entries) before reaching the disk-fit check, so the test uses 32800, which passes `geometry()` (8175 clusters) and then fails the fit check against the 32768-sector disk. The 4096-byte-sector volume has 8179 clusters (inside the FAT16 range), so `format` accepts it.

**(c) `crates/fat/tests/fat16.rs`**. In `every_mutating_op_records_changes_events_and_history`, replace (lines 196-202):

```rust
    let records = vec![
        fs.create_dir("/D").unwrap(),
        fs.create_file("/D/F", b"1").unwrap(),
        fs.write_file("/D/F", b"22").unwrap(),
        fs.delete_file("/D/F").unwrap(),
        fs.remove_dir("/D").unwrap(),
    ];
```

with:

```rust
    let records = vec![
        fs.create_dir("/D").unwrap(),
        fs.create_file("/D/F", b"1").unwrap(),
        fs.write_file("/D/F", b"22").unwrap(),
        fs.delete_file("/D/F").unwrap(),
        fs.remove_dir("/D").unwrap(),
        fs.write_raw(0x4000, b"raw").unwrap(),
    ];
```

and replace (lines 213-217):

```rust
    assert!(records[3].event_kinds().contains(&"dir_entry_deleted"));
    assert_eq!(fs.history().len(), 5);
    assert_eq!(fs.history()[4].op, "remove_dir /D");
    assert!(fs.list_dir("/").is_ok());
    assert_eq!(fs.history().len(), 5);
```

with:

```rust
    assert!(records[3].event_kinds().contains(&"dir_entry_deleted"));
    assert_eq!(records[5].op, "write_raw 0x4000 +3");
    assert_eq!(records[5].event_kinds(), vec!["raw_write"]);
    assert_eq!(fs.history().len(), 6);
    assert_eq!(fs.history()[4].op, "remove_dir /D");
    assert_eq!(fs.history()[5].op, "write_raw 0x4000 +3");
    assert!(fs.list_dir("/").is_ok());
    assert_eq!(fs.history().len(), 6);
```

Append to the end of the file (after `usable_as_a_trait_object`, line 231):

```rust

#[test]
fn raw_write_rewinds_byte_for_byte_from_its_record() {
    let mut fs = default_fs();
    fs.create_file("/F", b"hello world").unwrap();
    let image_before = fs.disk().as_bytes().to_vec();
    let rec = fs.write_raw(0x1234, b"RAW").unwrap();
    assert_eq!(rec.changes.len(), 1);
    assert_eq!(rec.changes[0].offset, 0x1234);
    assert_eq!(rec.changes[0].before, vec![0, 0, 0]);
    assert_eq!(rec.changes[0].after, b"RAW".to_vec());
    assert_ne!(fs.disk().as_bytes(), &image_before[..]);
    // Rewinding is what the explorer's timeline does: put `before` back.
    let mut rewound = fs.disk().as_bytes().to_vec();
    for change in rec.changes.iter().rev() {
        rewound[change.offset..change.offset + change.before.len()].copy_from_slice(&change.before);
    }
    assert_eq!(rewound, image_before);
}

#[test]
fn raw_write_into_a_file_cluster_changes_what_read_file_returns() {
    let mut fs = default_fs();
    fs.create_file("/F", b"hello world").unwrap();
    let first = fs.cluster_chain(2).unwrap()[0];
    let off = fs.geometry().cluster_offset(first);
    fs.write_raw(off as u64, b"HELLO").unwrap();
    assert_eq!(fs.read_file("/F").unwrap(), b"HELLO world");
    assert_eq!(fs.stat("/F").unwrap().size, 11);
    assert_eq!(fs.history().len(), 2);
}

#[test]
fn clobbering_the_boot_sector_matches_from_image_judgement() {
    let mut fs = default_fs();
    fs.create_file("/A", b"a").unwrap();
    fs.write_raw(510, &[0, 0]).unwrap();
    assert!(matches!(fs.list_dir("/"), Err(Error::CorruptImage(_))));
    assert!(matches!(fs.corruption(), Some(Error::CorruptImage(_))));
    assert!(matches!(
        FatFs::from_image(fs.disk().as_bytes().to_vec()),
        Err(Error::CorruptImage(_))
    ));
    assert_eq!(fs.layout().len(), 5);
    assert!(!fs.annotate_sector(0).is_empty());
    fs.write_raw(510, &[0x55, 0xAA]).unwrap();
    assert!(fs.corruption().is_none());
    assert_eq!(fs.list_dir("/").unwrap().len(), 1);
    let again = FatFs::from_image(fs.disk().as_bytes().to_vec()).unwrap();
    assert_eq!(again.read_file("/A").unwrap(), b"a");
}

#[test]
fn raw_write_out_of_bounds_via_the_trait() {
    let mut fs: Box<dyn FileSystem> = Box::new(default_fs());
    assert!(matches!(
        fs.write_raw(u64::MAX, &[0]),
        Err(Error::OutOfBounds { .. })
    ));
    let len = fs.disk().len() as u64;
    assert_eq!(
        fs.write_raw(len, &[0]).unwrap_err(),
        Error::OutOfBounds {
            offset: len,
            len: 1,
            disk_len: len
        }
    );
    assert!(fs.history().is_empty());
    fs.write_raw(len - 1, &[7]).unwrap();
    assert_eq!(fs.history().len(), 1);
    assert_eq!(fs.disk().read(len as usize - 1, 1), &[7]);
}
```

(`FatFs`, `FormatOptions`, `Error`, `FileSystem`, `default_fs` and `tiny_fs` are already in scope at the top of that file.)

- [ ] **Step 22: Run them and confirm the failures**

Run: `cargo test -p fs-core fs:: 2>&1 | head -20; cargo test -p fat 2>&1 | head -20`

Expected: fs-core fails with `error[E0407]: method `write_raw` is not a member of trait `FileSystem`` (the `NullFs` impl) and `error[E0599]: no method named `write_raw` found for struct `Box<dyn fs::FileSystem>``; fat fails with `error[E0599]: no method named `corruption` found for struct `FatFs`` (13 sites) and `no method named `write_raw` found for struct `Box<dyn FileSystem>`` (the `fat16` trait test); the inherent `fs.write_raw(...)` calls in the fat unit tests report the same E0599 on `FatFs`.

- [ ] **Step 23: Implement the trait method and the `FatFs` raw write**

**(a) `crates/fs-core/src/fs.rs`**, the trait (line 24). Replace:

```rust
    /// The timestamp subsequent operations stamp onto entries.
    fn set_now(&mut self, now: DateTime);

    fn disk(&self) -> &Disk;
```

with:

```rust
    /// The timestamp subsequent operations stamp onto entries.
    fn set_now(&mut self, now: DateTime);
    /// Write bytes at an absolute disk offset, journaled like any other
    /// operation (op name from `raw_write_op`). `OutOfBounds` if the range
    /// runs past the disk. The filesystem re-reads any metadata the range
    /// covers; a write that leaves its boot sector or superblock unparsable
    /// makes path operations fail with `CorruptImage` until a later raw
    /// write repairs it.
    fn write_raw(&mut self, offset: u64, bytes: &[u8]) -> Result<OpRecord>;

    fn disk(&self) -> &Disk;
```

**(b) `crates/fat/src/fs.rs`**.

Import (line 4). Replace:

```rust
use crate::boot_sector::{BootSector, FormatOptions, Geometry};
```

with:

```rust
use crate::boot_sector::{BootSector, FormatOptions, Geometry, BOOT_SECTOR_LEN};
```

Struct (lines 18-24). Replace:

```rust
pub struct FatFs {
    disk: Disk,
    boot: BootSector,
    geo: Geometry,
    now: DateTime,
    history: Vec<OpRecord>,
}
```

with:

```rust
pub struct FatFs {
    disk: Disk,
    boot: BootSector,
    geo: Geometry,
    now: DateTime,
    history: Vec<OpRecord>,
    /// The gate error (`corrupt image: boot sector no longer parses after a raw write: {cause}`), or `None`
    /// while mounted. `boot` and `geo` keep their last good values so
    /// layout and annotation stay stable.
    corrupt: Option<Error>,
}
```

Both constructors. In `format` (lines 37-43) and in `from_image` (lines 65-71) replace each occurrence of:

```rust
            now: DateTime::default(),
            history: Vec::new(),
        })
```

with:

```rust
            now: DateTime::default(),
            history: Vec::new(),
            corrupt: None,
        })
```

Accessor. After `geometry()` (lines 78-80) replace:

```rust
    pub fn geometry(&self) -> &Geometry {
        &self.geo
    }

    pub fn fs_type(&self) -> &'static str {
```

with:

```rust
    pub fn geometry(&self) -> &Geometry {
        &self.geo
    }

    /// `Some(error)` while the boot sector does not parse or does not fit
    /// the disk (after a raw write); path operations fail with
    /// `CorruptImage` until a later raw write repairs it.
    pub fn corruption(&self) -> Option<&Error> {
        self.corrupt.as_ref()
    }

    pub fn fs_type(&self) -> &'static str {
```

Gate, re-parse and the op. After the end of `run_op` (line 151) replace:

```rust
        self.history.push(record.clone());
        Ok(record)
    }

    /// `/` for the root, otherwise `/A/B` from components.
    fn dir_display_name(parts: &[String]) -> String {
```

with:

```rust
        self.history.push(record.clone());
        Ok(record)
    }

    /// The gate every path-based method passes first: `CorruptImage` while
    /// a raw write has left the boot sector unparsable.
    fn ensure_mounted(&self) -> Result<()> {
        match &self.corrupt {
            None => Ok(()),
            Some(e) => Err(e.clone()),
        }
    }

    /// Re-read the boot sector after a raw write touched it. If it parses
    /// and its geometry fits the physical disk (same bytes per sector, no
    /// more sectors than the disk has), adopt it: the disk is the truth.
    /// Otherwise keep the last good `boot`/`geo` and mark the volume
    /// corrupt. Never fails: the bytes stay as written.
    fn reparse_boot_sector(&mut self) {
        let disk = &self.disk;
        let parsed = BootSector::parse(disk.read(0, BOOT_SECTOR_LEN))
            .and_then(|boot| boot.geometry().map(|geo| (boot, geo)))
            .and_then(|(boot, geo)| {
                if geo.bytes_per_sector != disk.sector_size() {
                    return Err(Error::CorruptImage(format!(
                        "boot sector says {} bytes per sector but the disk has {}-byte sectors",
                        geo.bytes_per_sector,
                        disk.sector_size()
                    )));
                }
                if geo.total_sectors > disk.sector_count() {
                    return Err(Error::CorruptImage(format!(
                        "boot sector describes {} sectors but the disk has {}",
                        geo.total_sectors,
                        disk.sector_count()
                    )));
                }
                Ok((boot, geo))
            });
        match parsed {
            Ok((boot, geo)) => {
                self.boot = boot;
                self.geo = geo;
                self.corrupt = None;
            }
            Err(e) => {
                let cause = match e {
                    Error::CorruptImage(cause) => cause,
                    other => other.to_string(),
                };
                self.corrupt = Some(Error::CorruptImage(format!(
                    "boot sector no longer parses after a raw write: {cause}"
                )));
            }
        }
    }

    /// Write bytes anywhere on the disk, journaled like every other
    /// operation. A write that starts inside the first `BOOT_SECTOR_LEN`
    /// bytes re-parses the boot sector (see `reparse_boot_sector`). Works
    /// while the volume is corrupt, so a later write can repair it.
    pub fn write_raw(&mut self, offset: u64, bytes: &[u8]) -> Result<OpRecord> {
        self.run_op(fs_core::raw_write_op(offset, bytes.len()), |fs| {
            let range = fs_core::raw_write(&mut fs.disk, offset, bytes)?;
            if range.start < BOOT_SECTOR_LEN {
                fs.reparse_boot_sector();
            }
            Ok(())
        })
    }

    /// `/` for the root, otherwise `/A/B` from components.
    fn dir_display_name(parts: &[String]) -> String {
```

(`let disk = &self.disk;` is bound before the closure chain and the `self.boot`/`self.geo` writes happen only after `parsed` is fully computed; that is what keeps the borrow checker happy. Keep the `and_then` chain rather than `if let ... &&` let-chains, which need Rust 1.88.)

The nine gates. Add `self.ensure_mounted()?;` as the first statement of each of these methods, i.e. make these exact replacements:

```rust
    pub fn list_dir(&self, path: &str) -> Result<Vec<EntryInfo>> {
        let parts = path::parse(path)?;
```
→
```rust
    pub fn list_dir(&self, path: &str) -> Result<Vec<EntryInfo>> {
        self.ensure_mounted()?;
        let parts = path::parse(path)?;
```

```rust
    pub fn stat(&self, path: &str) -> Result<EntryInfo> {
        match self.resolve(path)? {
```
→
```rust
    pub fn stat(&self, path: &str) -> Result<EntryInfo> {
        self.ensure_mounted()?;
        match self.resolve(path)? {
```

```rust
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        let located = self.resolve(path)?.ok_or(Error::IsADirectory)?;
```
→
```rust
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        self.ensure_mounted()?;
        let located = self.resolve(path)?.ok_or(Error::IsADirectory)?;
```

```rust
    pub fn create_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        let (parent_parts, name) = path::split_parent(path)?;
```
→
```rust
    pub fn create_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        self.ensure_mounted()?;
        let (parent_parts, name) = path::split_parent(path)?;
```

```rust
    pub fn create_dir(&mut self, path: &str) -> Result<OpRecord> {
        let (parent_parts, name) = path::split_parent(path)?;
```
→
```rust
    pub fn create_dir(&mut self, path: &str) -> Result<OpRecord> {
        self.ensure_mounted()?;
        let (parent_parts, name) = path::split_parent(path)?;
```

```rust
    pub fn delete_file(&mut self, path: &str) -> Result<OpRecord> {
        let located = self.resolve(path)?.ok_or(Error::InvalidPath)?;
```
→
```rust
    pub fn delete_file(&mut self, path: &str) -> Result<OpRecord> {
        self.ensure_mounted()?;
        let located = self.resolve(path)?.ok_or(Error::InvalidPath)?;
```

```rust
    pub fn remove_dir(&mut self, path: &str) -> Result<OpRecord> {
        let located = self.resolve(path)?.ok_or(Error::InvalidPath)?;
```
→
```rust
    pub fn remove_dir(&mut self, path: &str) -> Result<OpRecord> {
        self.ensure_mounted()?;
        let located = self.resolve(path)?.ok_or(Error::InvalidPath)?;
```

```rust
    pub fn write_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        let located = self.resolve(path)?.ok_or(Error::InvalidPath)?;
```
→
```rust
    pub fn write_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        self.ensure_mounted()?;
        let located = self.resolve(path)?.ok_or(Error::InvalidPath)?;
```

```rust
    pub fn raw_dir_entries(&self, path: &str) -> Result<Vec<RawEntry>> {
        let parts = path::parse(path)?;
```
→
```rust
    pub fn raw_dir_entries(&self, path: &str) -> Result<Vec<RawEntry>> {
        self.ensure_mounted()?;
        let parts = path::parse(path)?;
```

Not gated, by design: `layout`, `annotate_sector`, `annotate_sector_with`, `cluster_owners`, `fat_entries`, `cluster_chain`, `boot_sector`, `geometry`, `disk`, `history`, `write_raw`.

Live boot-sector annotations. In `annotate_boot_sector` (lines 677-679) replace:

```rust
    fn annotate_boot_sector(&self) -> Vec<Annotation> {
        let b = &self.boot;
        let jump = self.disk.read(0, 3);
        let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).to_string();
```

with:

```rust
    /// Always describes the live bytes, mounted or not, so sector-0
    /// annotations match what is on screen even while the volume is corrupt.
    fn annotate_boot_sector(&self) -> Vec<Annotation> {
        let raw = self.disk.read(0, BOOT_SECTOR_LEN);
        let decoded = BootSector::decode(raw);
        let b = &decoded;
        let jump = &raw[0..3];
        let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).to_string();
```

and the last two entries of the returned `vec!` (lines 722-723):

```rust
            annotation(62..510, "boot code", "unused"),
            annotation(510..512, "boot signature", "55 AA"),
        ]
```

with:

```rust
            annotation(62..510, "boot code", "unused"),
            annotation(
                510..512,
                "boot signature",
                format!("{:02X} {:02X}", raw[510], raw[511]),
            ),
        ]
```

Trait forwarding. In `impl FileSystem for FatFs` (lines 947-952) replace:

```rust
    fn set_now(&mut self, now: DateTime) {
        FatFs::set_now(self, now)
    }
    fn disk(&self) -> &Disk {
        FatFs::disk(self)
    }
```

with:

```rust
    fn set_now(&mut self, now: DateTime) {
        FatFs::set_now(self, now)
    }
    fn write_raw(&mut self, offset: u64, bytes: &[u8]) -> Result<OpRecord> {
        FatFs::write_raw(self, offset, bytes)
    }
    fn disk(&self) -> &Disk {
        FatFs::disk(self)
    }
```

- [ ] **Step 24: Format, then run every gate**

Run:

```
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --target wasm32-unknown-unknown
```

Expected: fmt reports nothing; clippy finishes with no warnings; `cargo test --workspace` prints, in some order, `fs-core` `25 passed`, `fat` lib `82 passed` (the seven new `fs::tests::write_raw_*` tests plus the extended `works_through_the_trait_object`), `fat16` `16 passed` (the four new tests plus the extended `every_mutating_op_records_changes_events_and_history`), `mount_macos` `1 ignored`, `fs-emulator-wasm` `11 passed` (`every_variant_has_its_own_code` still green); the wasm32 build finishes.

If `cargo test --workspace` fails to build the wasm crate's dev-dependencies offline, `cargo test -p fs-core -p fat` plus `cargo check -p fs-emulator-wasm --all-targets` is the equivalent check; CI runs the full workspace.

- [ ] **Step 25: Commit**

```
git add crates/fs-core/src/fs.rs crates/fat/src/fs.rs crates/fat/tests/fat16.rs
git commit -m "feat(fat): journaled FatFs::write_raw with boot-sector re-parse and CorruptImage gate"
```

---

**What the next tasks build on**

- Task 2 (wasm): `Volume.writeRaw(offset: u32, bytes: &[u8])` forwards to `self.fs_mut().write_raw(offset as u64, bytes)` through `self.op(...)`; `corruption()` in the FAT-only block returns `self.fat()?.corruption().map(|e| e.to_string())`; `code_of` already maps `OutOfBounds`, so task 2 only extends the uniqueness test with `Error::OutOfBounds { offset: 1, len: 2, disk_len: 3 }` as element 13 and adds the `wasm-pack test --node` cases.
- The gate message any shell test matches on starts with `boot sector no longer parses after a raw write`; `Error::to_string()` of the gate error therefore reads `corrupt image: boot sector no longer parses after a raw write: ...`, and the wasm `message` will be that full string.
- The op string every timeline entry shows is `write_raw 0x{offset:x} +{len}` (lowercase hex, e.g. `write_raw 0x2b +11` for the scenario's label patch at offset 43).


---

### Task 2: wasm: writeRaw, readRaw, corruption

Spec: `docs/superpowers/specs/2026-09-22-terminal-access-design.md`, section "2. wasm" and "Testing / wasm". Everything in this task lives in `crates/wasm`; the Rust core it calls into landed in Task 1.

**Files:**
- Modify: `/Users/bsmall/dev/fs-emulator/crates/wasm/src/error.rs` (the `code_of` match at lines 8-23; the native test array at lines 42-56 and its index assertions at lines 58-59)
- Modify: `/Users/bsmall/dev/fs-emulator/crates/wasm/src/volume.rs` (generic `#[wasm_bindgen] impl Volume` block: insert after `sector`, which ends at line 259, before `/// A copy of the whole disk image.` at line 261; FAT block: insert after `geometry`, which ends at line 278)
- Test: `/Users/bsmall/dev/fs-emulator/crates/wasm/tests/volume.rs` (append three `#[wasm_bindgen_test]` functions after `serializer_contract_null_and_bytes`, which ends at line 277, the last line of the file)
- Modify: `/Users/bsmall/dev/fs-emulator/crates/wasm/README.md` (lines 16-17, 21-25, 27-32)
- Regenerate (gitignored, not committed): `/Users/bsmall/dev/fs-emulator/crates/wasm/pkg/*` via `wasm-pack build crates/wasm --target bundler`

**Interfaces:**

Consumes (from Task 1; verify they exist before starting, step 1):
- `fs_core::Error::OutOfBounds { offset: u64, len: u64, disk_len: u64 }` — the 14th and last variant of `fs_core::Error`; Display `out of bounds: {len} bytes at offset {offset} run past the end of the {disk_len}-byte disk`
- `fs_core::FileSystem::write_raw(&mut self, offset: u64, bytes: &[u8]) -> fs_core::Result<fs_core::OpRecord>` — required trait method; the record's `op` is `write_raw 0x{offset:x} +{len}`, one `ByteChange`, one event of kind `raw_write` with text `wrote {len} raw bytes at 0x{offset:x}` and region `offset..offset+len`
- `fat::FatFs::corruption(&self) -> Option<&fs_core::Error>` — `Some(Error::CorruptImage(..))` while sector 0 no longer parses after a raw write; `None` while mounted
- `fat::FatFs::list_dir` / `raw_dir_entries` / `read_file` etc. return `Error::CorruptImage("boot sector no longer parses after a raw write: {cause}")` while corrupt (the `ensure_mounted` gate); `layout`, `boot_sector`, `geometry`, `annotate_sector`, `write_raw` keep working
- Existing wasm helpers, unchanged: `crate::error::{code_of, js_error, to_js}`, `Volume::fs()`, `Volume::fs_mut()`, `Volume::fat()`, `Volume::op()` (all in `crates/wasm/src/volume.rs` lines 67-91), and the test helpers `get`, `code`, `obj`, `fresh` in `crates/wasm/tests/volume.rs` lines 8-27

Produces (exact names later tasks rely on):
- Rust, in `crates/wasm/src/error.rs`: the `code_of` arm `OutOfBounds { .. } => "OutOfBounds"` (JS error `code === "OutOfBounds"`)
- Rust, in `crates/wasm/src/volume.rs`, generic block:
  - `pub fn write_raw(&mut self, offset: u32, bytes: &[u8]) -> Result<JsValue, JsValue>` exported as `writeRaw`
  - `pub fn read_raw(&self, offset: u32, len: u32) -> Result<Vec<u8>, JsValue>` exported as `readRaw`
- Rust, in `crates/wasm/src/volume.rs`, FAT block: `pub fn corruption(&self) -> Result<JsValue, JsValue>` exported as `corruption`
- TypeScript, in the regenerated `crates/wasm/pkg/fs_emulator_wasm.d.ts` (consumed by `web/ui` through `file:../../crates/wasm/pkg` in Tasks 3-6):
  - `writeRaw(offset: number, bytes: Uint8Array): OpRecord;` — throws an `FsError` with `code === "OutOfBounds"` (message starts `out of bounds:`) when `offset + bytes.length` exceeds the disk; a write into the first 512 bytes re-parses the boot sector (Task 1 rule)
  - `readRaw(offset: number, len: number): Uint8Array;` — throws `code === "BadArgument"` when `offset + len` exceeds the disk; `readRaw(diskLen, 0)` is an empty array
  - `corruption(): string | null;` — the `CorruptImage` message (starts `corrupt image:`) while the boot sector does not parse, else `null`; throws `NotFat` on a non-FAT volume like every other FAT-only method
  - wasm-bindgen does not validate `u32` arguments: callers pass non-negative integers only (the shell's `addr.ts`/`dd.ts` guarantee this). `u32` suffices because `MAX_VOLUME_BYTES` is 256 MiB.
- `crates/wasm/src/types.rs` and `crates/wasm/src/dto.rs` are unchanged: `OpRecord` is already declared, and `EventRecord::from_event` is generic over `&dyn Event`, so `raw_write` events cross the boundary with `kind`/`text`/`region` for free.

Global constraints that apply here: `crates/wasm` uses wasm-bindgen 0.2 and serde-wasm-bindgen 0.6 (already in `Cargo.toml`; no dependency change); every error crossing the boundary carries a `code` string; `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` must pass; rust-version 1.87 (the `let ... else` used below is stable since 1.65). Commit messages are conventional with no attribution lines.

---

- [ ] **Step 1: Confirm Task 1 is in the tree**

Run:

```bash
cd /Users/bsmall/dev/fs-emulator
grep -n "OutOfBounds" crates/fs-core/src/error.rs
grep -n "fn write_raw" crates/fs-core/src/fs.rs crates/fat/src/fs.rs
grep -n "pub fn corruption" crates/fat/src/fs.rs
grep -n "OutOfBounds" crates/wasm/src/error.rs
```

Expected: the first three greps each print at least one line. The fourth prints nothing (the wasm crate has not been touched) **or** prints a single line `        OutOfBounds { .. } => "OutOfBounds",` if Task 1 had to add the match arm to keep `cargo clippy --workspace` compiling (with the new variant and no arm, `crates/wasm` fails with `E0004 non-exhaustive patterns`). Either state is fine; step 2 and step 3 say what differs.

- [ ] **Step 2: Extend the native code-uniqueness test (test first)**

In `/Users/bsmall/dev/fs-emulator/crates/wasm/src/error.rs`, inside `every_variant_has_its_own_code`, replace the end of the `all` array and the index assertions.

Old (lines 54-59):

```rust
            Error::InvalidGeometry("x".into()),
            Error::CorruptImage("x".into()),
            Error::Unsupported("x".into()),
        ];
        let codes: Vec<&str> = all.iter().map(code_of).collect();
        assert_eq!(codes[0], "NotFound");
        assert_eq!(codes[11], "CorruptImage");
```

New:

```rust
            Error::InvalidGeometry("x".into()),
            Error::CorruptImage("x".into()),
            Error::Unsupported("x".into()),
            Error::OutOfBounds {
                offset: 1,
                len: 2,
                disk_len: 3,
            },
        ];
        let codes: Vec<&str> = all.iter().map(code_of).collect();
        assert_eq!(codes[0], "NotFound");
        assert_eq!(codes[11], "CorruptImage");
        assert_eq!(codes[13], "OutOfBounds");
```

Run `cargo test -p fs-emulator-wasm`.

Expected when the arm is absent: the crate does not compile:

```
error[E0004]: non-exhaustive patterns: `&fs_core::Error::OutOfBounds { .. }` not covered
error: could not compile `fs-emulator-wasm` (lib) due to 1 previous error
```

Expected when Task 1 already added the arm: `test error::tests::every_variant_has_its_own_code ... ok` and `test result: ok. 11 passed`. In that case skip step 3.

- [ ] **Step 3: Map `OutOfBounds` in `code_of`**

In `/Users/bsmall/dev/fs-emulator/crates/wasm/src/error.rs`, `code_of` (lines 8-23).

Old:

```rust
        InvalidGeometry(_) => "InvalidGeometry",
        CorruptImage(_) => "CorruptImage",
        Unsupported(_) => "Unsupported",
    }
}
```

New:

```rust
        InvalidGeometry(_) => "InvalidGeometry",
        CorruptImage(_) => "CorruptImage",
        Unsupported(_) => "Unsupported",
        OutOfBounds { .. } => "OutOfBounds",
    }
}
```

Run `cargo test -p fs-emulator-wasm`. Expected: `test error::tests::every_variant_has_its_own_code ... ok` and `test result: ok. 11 passed; 0 failed` (the lib unit tests: the dto tests plus this one).

- [ ] **Step 4: Write the three wasm-bindgen tests (test first)**

Append to the end of `/Users/bsmall/dev/fs-emulator/crates/wasm/tests/volume.rs` (after the closing brace of `serializer_contract_null_and_bytes`, line 277). The file already imports `Array, Object, Reflect, Uint8Array`, `JsCast, JsValue`, `wasm_bindgen_test::*`, and defines `get`, `code`, `obj`, `fresh`; nothing else is needed.

```rust

#[wasm_bindgen_test]
fn raw_write_and_read_round_trip() {
    let mut v = fresh();
    let rec = v.write_raw(0x1000, &[1, 2, 3]).unwrap();
    assert_eq!(get(&rec, "op").as_string().unwrap(), "write_raw 0x1000 +3");
    let changes = Array::from(&get(&rec, "changes"));
    assert_eq!(changes.length(), 1);
    let c0 = changes.get(0);
    assert_eq!(get(&c0, "offset").as_f64(), Some(4096.0));
    let before = get(&c0, "before");
    let after = get(&c0, "after");
    assert!(before.is_instance_of::<Uint8Array>());
    assert!(after.is_instance_of::<Uint8Array>());
    assert_eq!(Uint8Array::from(before).to_vec(), vec![0, 0, 0]);
    assert_eq!(Uint8Array::from(after).to_vec(), vec![1, 2, 3]);
    let events = Array::from(&get(&rec, "events"));
    assert_eq!(events.length(), 1);
    let e0 = events.get(0);
    assert_eq!(get(&e0, "kind").as_string().unwrap(), "raw_write");
    assert_eq!(
        get(&e0, "text").as_string().unwrap(),
        "wrote 3 raw bytes at 0x1000"
    );
    let region = get(&e0, "region");
    assert_eq!(get(&region, "start").as_f64(), Some(4096.0));
    assert_eq!(get(&region, "end").as_f64(), Some(4099.0));
    assert_eq!(v.read_raw(0x1000, 3).unwrap(), vec![1, 2, 3]);
    assert_eq!(&v.sector(8).unwrap()[..3], &[1, 2, 3]);
    assert_eq!(v.history_length(), 1);
    assert_eq!(
        get(&v.history_at(0).unwrap(), "op").as_string().unwrap(),
        "write_raw 0x1000 +3"
    );
}

#[wasm_bindgen_test]
fn raw_errors_carry_codes() {
    let mut v = fresh();
    let disk_len: u32 = 32768 * 512;
    let err = v.write_raw(u32::MAX, &[1]).unwrap_err();
    assert_eq!(code(err.clone()), "OutOfBounds");
    assert!(get(&err, "message")
        .as_string()
        .unwrap()
        .starts_with("out of bounds:"));
    assert_eq!(
        code(v.write_raw(disk_len - 1, &[1, 2]).unwrap_err()),
        "OutOfBounds"
    );
    assert_eq!(code(v.read_raw(u32::MAX, 1).unwrap_err()), "BadArgument");
    assert_eq!(code(v.read_raw(0, u32::MAX).unwrap_err()), "BadArgument");
    assert_eq!(code(v.read_raw(disk_len, 1).unwrap_err()), "BadArgument");
    assert_eq!(v.read_raw(disk_len - 1, 1).unwrap().len(), 1);
    assert!(v.read_raw(disk_len, 0).unwrap().is_empty());
    assert_eq!(v.history_length(), 0);
}

#[wasm_bindgen_test]
fn corruption_is_null_until_sector_zero_breaks_and_clears_when_repaired() {
    let mut v = fresh();
    v.create_file("/A", b"a").unwrap();
    assert!(v.corruption().unwrap().is_null());
    let saved = v.sector(0).unwrap();
    v.write_raw(0, &vec![0u8; 512]).unwrap();
    let msg = v.corruption().unwrap();
    assert!(msg.is_string(), "expected a message, got {:?}", msg);
    assert!(msg.as_string().unwrap().starts_with("corrupt image:"));
    let err = v.list_dir("/").unwrap_err();
    assert_eq!(code(err.clone()), "CorruptImage");
    assert!(get(&err, "message")
        .as_string()
        .unwrap()
        .contains("boot sector no longer parses after a raw write"));
    assert_eq!(code(v.raw_dir_entries("/").unwrap_err()), "CorruptImage");
    assert_eq!(Array::from(&v.layout().unwrap()).length(), 5);
    assert_eq!(
        get(&v.boot_sector().unwrap(), "fsType")
            .as_string()
            .unwrap(),
        "FAT16"
    );
    assert_eq!(&v.sector(0).unwrap()[510..], &[0, 0]);
    v.write_raw(0, &saved).unwrap();
    assert!(v.corruption().unwrap().is_null());
    assert_eq!(v.read_file("/A").unwrap(), b"a");
    assert_eq!(v.history_length(), 3);
}
```

What each test pins: the first is the `OpRecord` shape from the spec (op string, one change with `Uint8Array` before/after, one `raw_write` event with a region) plus a `readRaw`/`sector` round trip and the history entry; the second is the `OutOfBounds` code and message prefix, `readRaw`'s `BadArgument` at both overflow edges (`u32::MAX` offset, `u32::MAX` len, one past the end), the two legal edge reads, and that failed calls leave no history; the third is `corruption()` `null` → message → `null`, the gate message on path methods, and that `layout`, `bootSector` (last good), `sector`, and `writeRaw` keep working while corrupt. `history_length() == 3` counts `createFile`, the wipe, and the restore; the two error calls in between recorded nothing.

Run `wasm-pack test --node crates/wasm`.

Expected: compilation of the test target fails with 14 errors of three kinds and no test runs:

```
error[E0599]: no method named `write_raw` found for struct `Volume` in the current scope
error[E0599]: no method named `read_raw` found for struct `Volume` in the current scope
error[E0599]: no method named `corruption` found for struct `Volume` in the current scope
error: could not compile `fs-emulator-wasm` (test "volume") due to 14 previous errors
```

- [ ] **Step 5: Add `writeRaw` and `readRaw` to the generic block**

In `/Users/bsmall/dev/fs-emulator/crates/wasm/src/volume.rs`, in the first `#[wasm_bindgen] impl Volume` block, directly after `sector` (the method ending at line 259) and before `/// A copy of the whole disk image.` (line 261).

Old (lines 249-264):

```rust
    /// A copy of one sector's bytes.
    pub fn sector(&self, n: u32) -> Result<Vec<u8>, JsValue> {
        let disk = self.fs().disk();
        if n as u64 >= disk.sector_count() {
            return Err(js_error(
                "BadArgument",
                &format!("sector {n} is out of range (count {})", disk.sector_count()),
            ));
        }
        Ok(disk.sector(n as u64).to_vec())
    }

    /// A copy of the whole disk image.
    pub fn image(&self) -> Vec<u8> {
        self.fs().disk().as_bytes().to_vec()
    }
```

New:

```rust
    /// A copy of one sector's bytes.
    pub fn sector(&self, n: u32) -> Result<Vec<u8>, JsValue> {
        let disk = self.fs().disk();
        if n as u64 >= disk.sector_count() {
            return Err(js_error(
                "BadArgument",
                &format!("sector {n} is out of range (count {})", disk.sector_count()),
            ));
        }
        Ok(disk.sector(n as u64).to_vec())
    }

    /// Write bytes at an absolute byte offset, journaled like every other
    /// operation. Throws `OutOfBounds` if the range runs past the disk.
    #[wasm_bindgen(js_name = writeRaw, unchecked_return_type = "OpRecord")]
    pub fn write_raw(&mut self, offset: u32, bytes: &[u8]) -> Result<JsValue, JsValue> {
        let r = self.fs_mut().write_raw(offset as u64, bytes);
        self.op(r)
    }

    /// A copy of `len` bytes starting at an absolute byte offset.
    #[wasm_bindgen(js_name = readRaw)]
    pub fn read_raw(&self, offset: u32, len: u32) -> Result<Vec<u8>, JsValue> {
        let disk = self.fs().disk();
        let disk_len = disk.len() as u64;
        let end = (offset as u64)
            .checked_add(len as u64)
            .filter(|&end| end <= disk_len);
        let Some(end) = end else {
            return Err(js_error(
                "BadArgument",
                &format!(
                    "range {offset}..{} is out of range (disk is {disk_len} bytes)",
                    offset as u64 + len as u64
                ),
            ));
        };
        Ok(disk
            .read(offset as usize, (end - offset as u64) as usize)
            .to_vec())
    }

    /// A copy of the whole disk image.
    pub fn image(&self) -> Vec<u8> {
        self.fs().disk().as_bytes().to_vec()
    }
```

Notes on why it is shaped this way: `write_raw` is the `writeFile` template (lines 156-160) routed through `Volume::op`, so `OutOfBounds` from the core becomes a JS error via `to_js`/`code_of`, and the record crosses as a `dto::OpRecord` like every other mutation. `read_raw` is the `sector` template: `Disk::read` indexes unchecked and would panic (poisoning the wasm instance), so the bounds check runs first, with `checked_add` in `u64` so `u32::MAX + u32::MAX` cannot overflow; `offset as u64 + len as u64` in the message is also overflow-free in `u64`. `Vec<u8>` returns as `Uint8Array` and `&[u8]` accepts a `Uint8Array`, exactly like `sector` and `writeFile`. `unchecked_return_type = "OpRecord"` is required or the `.d.ts` says `any`.

- [ ] **Step 6: Add `corruption()` to the FAT block**

In `/Users/bsmall/dev/fs-emulator/crates/wasm/src/volume.rs`, in the second `#[wasm_bindgen] impl Volume` block (the one under `/// FAT-specific inspection.`), directly after `geometry` (originally lines 275-278; after step 5 the block has shifted down by 30 lines) and before the `fatEntries` doc comment.

Old:

```rust
    #[wasm_bindgen(unchecked_return_type = "Geometry")]
    pub fn geometry(&self) -> Result<JsValue, JsValue> {
        to_value(&dto::Geometry::from(self.fat()?.geometry()))
    }

    /// Every entry of one FAT copy, indexed by cluster; empty for a copy that does not exist.
```

New:

```rust
    #[wasm_bindgen(unchecked_return_type = "Geometry")]
    pub fn geometry(&self) -> Result<JsValue, JsValue> {
        to_value(&dto::Geometry::from(self.fat()?.geometry()))
    }

    /// The `CorruptImage` message while the boot sector does not parse after
    /// a `writeRaw`, or `null` while the volume is mounted.
    #[wasm_bindgen(unchecked_return_type = "string | null")]
    pub fn corruption(&self) -> Result<JsValue, JsValue> {
        Ok(match self.fat()?.corruption() {
            Some(err) => JsValue::from_str(&err.to_string()),
            None => JsValue::NULL,
        })
    }

    /// Every entry of one FAT copy, indexed by cluster; empty for a copy that does not exist.
```

`self.fat()?` keeps the FAT-only contract (a future non-FAT volume throws `NotFat` here like `bootSector` does). The Rust name and the JS name are both `corruption`, so no `js_name` attribute is needed. The message is the gate error's Display (`corrupt image: boot sector no longer parses after a raw write: boot sector signature is not 55 AA` after a zeroed sector 0), which is what the shell's `ls`/`df`/`mount`/`stat` print.

- [ ] **Step 7: Run the wasm tests and the Rust gates**

```bash
cd /Users/bsmall/dev/fs-emulator
cargo fmt --all
wasm-pack test --node crates/wasm
cargo test -p fs-emulator-wasm
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --target wasm32-unknown-unknown
cargo fmt --all -- --check
```

Expected from `wasm-pack test --node crates/wasm`:

```
running 9 tests
test serializer_contract_null_and_bytes ... ok
test raw_write_and_read_round_trip ... ok
test raw_errors_carry_codes ... ok
test history_layout_bytes_and_time ... ok
test format_and_basic_info ... ok
test fat_inspection ... ok
test errors_carry_codes ... ok
test create_list_read_stat_round_trip ... ok
test corruption_is_null_until_sector_zero_breaks_and_clears_when_repaired ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 filtered out
```

Expected from `cargo test -p fs-emulator-wasm`: `test result: ok. 11 passed`. Clippy, the wasm32 build, and the fmt check finish with no warnings or diffs. (`tests/volume.rs` is `#![cfg(target_arch = "wasm32")]`, so host clippy skips it; `wasm-pack test` is the only thing that compiles it.)

- [ ] **Step 8: Document the three methods in the crate README**

In `/Users/bsmall/dev/fs-emulator/crates/wasm/README.md`, three edits.

Edit 1, lines 16-17. Old:

```markdown
Errors are `Error` objects with a `code` property (`NotFound`, `DiskFull`,
`CorruptImage`, ..., plus `NotFat` and `BadArgument`).
```

New:

```markdown
Errors are `Error` objects with a `code` property (`NotFound`, `DiskFull`,
`CorruptImage`, `OutOfBounds`, ..., plus `NotFat` and `BadArgument`).
```

Edit 2, lines 21-25 (the generic method list gains `writeRaw` and `readRaw`, followed by a new paragraph). Old:

```markdown
`Volume` wraps the `FileSystem` trait from `fs-core`, so `createFile`,
`writeFile`, `readFile`, `deleteFile`, `createDir`, `removeDir`, `listDir`,
`stat`, `setNow`, `layout`, `annotateSector`, `historyLength`, `historyAt`,
`sectorSize`, `sectorCount`, `sector`, and `image` work on every filesystem
the crate will ever hold. `fsType()` names the one inside (`"FAT16"` today).
```

New:

```markdown
`Volume` wraps the `FileSystem` trait from `fs-core`, so `createFile`,
`writeFile`, `readFile`, `deleteFile`, `createDir`, `removeDir`, `listDir`,
`stat`, `setNow`, `layout`, `annotateSector`, `historyLength`, `historyAt`,
`sectorSize`, `sectorCount`, `sector`, `writeRaw`, `readRaw`, and `image`
work on every filesystem the crate will ever hold. `fsType()` names the one
inside (`"FAT16"` today).

`writeRaw(offset, bytes)` writes anywhere on the disk and returns an
`OpRecord` like every other mutating call, so the timeline and diff cover it;
a range past the end throws `OutOfBounds` and writes nothing. A write that
touches the first 512 bytes re-parses the boot sector: if it still parses and
fits the disk, the new geometry is adopted (the disk is the truth); if not,
the path methods (`createFile`, `writeFile`, `readFile`, `deleteFile`,
`createDir`, `removeDir`, `listDir`, `stat`, `rawDirEntries`) throw
`CorruptImage` with a message beginning `boot sector no longer parses after a
raw write` until a later `writeRaw` repairs it. `layout`, `annotateSector`,
`sector`, `readRaw`, `image`, and `writeRaw` keep working meanwhile, and
`bootSector()`/`geometry()` return the last good values. `readRaw(offset,
len)` returns a copy of the bytes and throws `BadArgument` past the end. Both
take byte offsets, not sector numbers, as non-negative integers (wasm-bindgen
does not validate them).
```

Edit 3, the FAT-only paragraph at lines 27-32 (now shifted down by the paragraph above). Old:

```markdown
`bootSector`, `geometry`, `fatEntries`, `clusterChain`, `rawDirEntries`,
`clusterOwners`, and `annotateSectorWith` are FAT-only and throw an error
with code `NotFat` on any other volume. When FAT32 lands it reuses them; when
ext2 lands it adds its own constructor (`formatExt2`) and its own inspection
methods that throw `NotExt`. A UI should branch on `fsType()` before calling
the specific ones. The plan is in `docs/ROADMAP.md`.
```

New:

```markdown
`bootSector`, `geometry`, `corruption`, `fatEntries`, `clusterChain`,
`rawDirEntries`, `clusterOwners`, and `annotateSectorWith` are FAT-only and
throw an error with code `NotFat` on any other volume. `corruption()` returns
the `CorruptImage` message while the boot sector does not parse after a raw
write, or `null` while the volume is mounted. When FAT32 lands it reuses
them; when ext2 lands it adds its own constructor (`formatExt2`) and its own
inspection methods that throw `NotExt`. A UI should branch on `fsType()`
before calling the specific ones. The plan is in `docs/ROADMAP.md`.
```

- [ ] **Step 9: Rebuild the npm package so `pkg/*.d.ts` carries the three methods**

```bash
cd /Users/bsmall/dev/fs-emulator
wasm-pack build crates/wasm --target bundler
grep -n "writeRaw\|readRaw\|corruption" crates/wasm/pkg/fs_emulator_wasm.d.ts
```

Expected: `[INFO]: 📦   Your wasm pkg is ready to publish at crates/wasm/pkg.` and the grep shows the three signatures inside `export class Volume` (wasm-bindgen sorts methods alphabetically and copies the doc comments):

```ts
    /**
     * The `CorruptImage` message while the boot sector does not parse after
     * a `writeRaw`, or `null` while the volume is mounted.
     */
    corruption(): string | null;
```

between `clusterOwners(): ClusterOwner[];` and `createDir(path: string): OpRecord;`,

```ts
    /**
     * A copy of `len` bytes starting at an absolute byte offset.
     */
    readRaw(offset: number, len: number): Uint8Array;
```

between `readFile(path: string): Uint8Array;` and `removeDir(path: string): OpRecord;`, and

```ts
    /**
     * Write bytes at an absolute byte offset, journaled like every other
     * operation. Throws `OutOfBounds` if the range runs past the disk.
     */
    writeRaw(offset: number, bytes: Uint8Array): OpRecord;
```

after `writeFile(path: string, data: Uint8Array): OpRecord;` as the last member of the class. `crates/wasm/pkg/` is gitignored (`.gitignore` line 2), so this step changes nothing in git; it exists so that `web/ui` (which depends on `file:../../crates/wasm/pkg`) type-checks against the new methods in Tasks 3-6. CI regenerates `pkg` itself (`.github/workflows/ci.yml`, the `wasm` job runs `wasm-pack build crates/wasm --target bundler` before the UI build).

- [ ] **Step 10: Commit**

```bash
cd /Users/bsmall/dev/fs-emulator
git add crates/wasm/src/error.rs crates/wasm/src/volume.rs crates/wasm/tests/volume.rs crates/wasm/README.md
git status --short
git commit -m "feat(wasm): expose writeRaw, readRaw, and corruption on Volume"
```

Expected `git status --short` before the commit: exactly four `M` lines for those files (and nothing under `crates/wasm/pkg`, which is ignored). If step 3 was skipped because Task 1 already added the `code_of` arm, `error.rs` still shows as modified from the test change in step 2.


---

### Task 3: UI store: geometry refresh, empty-change guard, OutOfBounds copy

Spec section 3 of `docs/superpowers/specs/2026-09-22-terminal-access-design.md`. After Task 1, a raw write whose range starts below 512 makes `FatFs` re-parse the boot sector and, when the new geometry fits the disk, adopt it: the root directory and data regions move. `VolumeStore.run` today refreshes only owners and FAT (`refreshMeta`), so the ribbon, attribution, and inspector would keep the pre-write layout. This task makes `run` re-read `geometry` and `layout` whenever a change lands in the boot sector, makes `changedSectors` ignore empty changes (a zero-length raw write at the end of the disk would otherwise name sector `sectorCount`), and adds the `OutOfBounds` sentence to the StatusLine copy.

Testing constraint you must know: `web/ui/vitest.config.ts` runs in node with only the wasm plugins, no Svelte plugin, and no test imports a `*.svelte.ts` store. The store's decision therefore lives in a pure helper, `touchesBootSector` in `src/core/patch.ts`, which the tests cover; the store change itself is verified by `svelte-check` (part of `pnpm build`) and by the integration test that replays the same wasm calls `run` makes.

**Files:**
- Modify: `web/ui/src/core/patch.ts` (lines 14–23, `changedSectors`; append `BOOT_SECTOR_LEN` and `touchesBootSector` after it)
- Modify: `web/ui/src/state/volume.svelte.ts` (line 3 import; lines 61–63 inside `run`; line 70 doc comment of `seek`)
- Modify: `web/ui/src/components/StatusLine.svelte` (line 19, the `FRIENDLY` map)
- Test: `web/ui/tests/patch.test.ts` (extend)
- Test: `web/ui/tests/integration.test.ts` (extend: one `writeRaw` op in the seek round trip; new root-entries layout test)

**Interfaces:**
- Consumes (Task 2, `crates/wasm/pkg/fs_emulator_wasm.d.ts` after `wasm-pack build crates/wasm --target bundler`): `Volume.writeRaw(offset: number, bytes: Uint8Array): OpRecord`, throwing an `FsError` with `code === "OutOfBounds"` past the end of the disk. Consumes Task 1's semantics behind it: the record has one `ByteChange { offset, before, after }` for the written range, and when `offset < 512` `FatFs` re-parses the boot sector and adopts a geometry that fits the disk, so `Volume.geometry()` and `Volume.layout()` reflect it.
- Consumes (existing): `ByteChangeLike { offset: number; before: Uint8Array; after: Uint8Array }`, `applyChanges`, `changedSectors` from `src/core/patch.ts`; `Volume.formatFat16(options: FormatOptions | undefined)`, `Volume.geometry(): Geometry`, `Volume.layout(): Region[]`, `Region { name: string; sectors: { start: number; end: number }; kind: RegionKind }`.
- Produces (in `web/ui/src/core/patch.ts`):
  - `export const BOOT_SECTOR_LEN = 512;`
  - `export function touchesBootSector(changes: ByteChangeLike[]): boolean` — true when any change's `offset < BOOT_SECTOR_LEN`.
  - `changedSectors(changes: ByteChangeLike[], sectorSize: number): number[]` keeps its signature; changes with `after.length === 0` contribute no sectors.
- Produces (behaviour later tasks rely on): `VolumeStore.run(fn: (v: Volume) => OpRecord): OpRecord | null` keeps its signature and its failure contract (returns `null` and sets `status = { text, code }`); after a successful op that touches the boot sector, `volume.geometry`, `volume.layout`, and the derived `attribution` are current. Task 7's `storeHost.svelte.ts` builds on exactly this. `StatusLine` shows "That range runs past the end of the disk." for `code === "OutOfBounds"`.

---

- [ ] **Step 1: Confirm the wasm package carries `writeRaw` and the baseline suite is green**

Run from the repository root:

```bash
grep -n "writeRaw\|readRaw\|corruption" /Users/bsmall/dev/fs-emulator/crates/wasm/pkg/fs_emulator_wasm.d.ts
```

Expected: three lines, including `writeRaw(offset: number, bytes: Uint8Array): OpRecord;`. If `grep` prints nothing, Task 2's build has not been run in this checkout; run it and refresh the copy pnpm keeps of the `file:` dependency (pnpm copies `file:` packages into `node_modules/.pnpm/fs-emulator-wasm@file+..+..+crates+wasm+pkg/`, it does not symlink them):

```bash
cd /Users/bsmall/dev/fs-emulator && wasm-pack build crates/wasm --target bundler
cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm install
grep -c "writeRaw" /Users/bsmall/dev/fs-emulator/web/ui/node_modules/fs-emulator-wasm/fs_emulator_wasm.d.ts
```

Expected: `pnpm install` reports the lockfile as up to date (the `file:` spec did not change) and the final `grep -c` prints `1`. If it prints `0`, the copy is stale: `rm -rf /Users/bsmall/dev/fs-emulator/web/ui/node_modules/.pnpm/fs-emulator-wasm@file+..+..+crates+wasm+pkg && cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm install` and grep again.

Then the baseline:

```bash
cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm test
```

Expected: `Test Files  12 passed (12)`, `Tests  46 passed (46)`.

- [ ] **Step 2: Write the failing tests for the patch helpers**

Replace the whole of `/Users/bsmall/dev/fs-emulator/web/ui/tests/patch.test.ts` with:

```ts
import { describe, expect, it } from "vitest";
import { applyChanges, changedSectors, touchesBootSector } from "../src/core/patch";

const c = (offset: number, before: number[], after: number[]) => ({ offset, before: new Uint8Array(before), after: new Uint8Array(after) });

describe("applyChanges", () => {
  it("forward then reverse is the identity, including overlapping writes", () => {
    const buf = new Uint8Array(32);
    const changes = [c(4, [0, 0, 0], [1, 2, 3]), c(5, [2, 3], [9, 9]), c(30, [0, 0], [7, 7])];
    applyChanges(buf, changes, "forward");
    expect(Array.from(buf.slice(4, 8))).toEqual([1, 9, 9, 0]);
    expect(Array.from(buf.slice(30))).toEqual([7, 7]);
    applyChanges(buf, changes, "reverse");
    expect(buf.every((b) => b === 0)).toBe(true);
  });
  it("lists changed sectors sorted and unique", () => {
    const changes = [c(1000, [0], [1]), c(20, [0, 0], [1, 1]), c(1020, [0], [1]), c(511, [0, 0], [1, 1])];
    expect(changedSectors(changes, 512)).toEqual([0, 1]);
  });
  it("skips changes with an empty `after`: a zero-length raw write at the end of the disk names no sector", () => {
    // 4096 is the disk length of an 8-sector disk; sector 8 does not exist.
    expect(changedSectors([c(4096, [], [])], 512)).toEqual([]);
    expect(changedSectors([c(4096, [], []), c(20, [0], [1])], 512)).toEqual([0]);
  });
});

describe("touchesBootSector", () => {
  it("is true when any change starts inside the first 512 bytes", () => {
    expect(touchesBootSector([c(17, [16, 0], [32, 0])])).toBe(true);
    expect(touchesBootSector([c(1000, [0], [1]), c(511, [0], [1])])).toBe(true);
    expect(touchesBootSector([c(512, [0], [1]), c(4096, [0], [1])])).toBe(false);
    expect(touchesBootSector([])).toBe(false);
  });
});
```

- [ ] **Step 3: Run the patch tests and watch them fail**

```bash
cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm exec vitest run tests/patch.test.ts
```

Expected: the file fails to run with a TypeScript/ESM error that `touchesBootSector` is not exported by `../src/core/patch` (vitest reports `SyntaxError: The requested module '../src/core/patch' does not provide an export named 'touchesBootSector'`). Had the import existed, the empty-`after` test would fail with `expected [ 8 ] to deeply equal []`, because the current `Math.max(c.after.length, 1)` turns an empty change at 4096 into sector 8.

- [ ] **Step 4: Implement the guard and the predicate**

In `/Users/bsmall/dev/fs-emulator/web/ui/src/core/patch.ts`, replace the `changedSectors` function (lines 14–23) with the two functions plus the constant below. Old text:

```ts
/** Sorted, de-duplicated sector numbers touched by the changes. */
export function changedSectors(changes: ByteChangeLike[], sectorSize: number): number[] {
  const set = new Set<number>();
  for (const c of changes) {
    const first = Math.floor(c.offset / sectorSize);
    const last = Math.floor((c.offset + Math.max(c.after.length, 1) - 1) / sectorSize);
    for (let s = first; s <= last; s++) set.add(s);
  }
  return [...set].sort((a, b) => a - b);
}
```

New text:

```ts
/** Sorted, de-duplicated sector numbers touched by the changes. An empty change
 *  (the core allows a zero-length raw write, even at the very end of the disk)
 *  touches no sector. */
export function changedSectors(changes: ByteChangeLike[], sectorSize: number): number[] {
  const set = new Set<number>();
  for (const c of changes) {
    if (c.after.length === 0) continue;
    const first = Math.floor(c.offset / sectorSize);
    const last = Math.floor((c.offset + c.after.length - 1) / sectorSize);
    for (let s = first; s <= last; s++) set.add(s);
  }
  return [...set].sort((a, b) => a - b);
}

/** Bytes the FAT core re-parses as the boot sector after a raw write. */
export const BOOT_SECTOR_LEN = 512;

/** True when any change starts inside the boot sector. The core may then have adopted
 *  a new geometry, so the store must re-read `geometry` and `layout`. */
export function touchesBootSector(changes: ByteChangeLike[]): boolean {
  return changes.some((c) => c.offset < BOOT_SECTOR_LEN);
}
```

The resulting file is:

```ts
export interface ByteChangeLike { offset: number; before: Uint8Array; after: Uint8Array }

/** Apply an operation's byte changes to a cached image. Forward writes `after` in
 *  order; reverse writes `before` in reverse order, so reverse∘forward is the identity
 *  even when writes overlap. */
export function applyChanges(buf: Uint8Array, changes: ByteChangeLike[], direction: "forward" | "reverse"): void {
  if (direction === "forward") {
    for (const c of changes) buf.set(c.after, c.offset);
  } else {
    for (let i = changes.length - 1; i >= 0; i--) buf.set(changes[i].before, changes[i].offset);
  }
}

/** Sorted, de-duplicated sector numbers touched by the changes. An empty change
 *  (the core allows a zero-length raw write, even at the very end of the disk)
 *  touches no sector. */
export function changedSectors(changes: ByteChangeLike[], sectorSize: number): number[] {
  const set = new Set<number>();
  for (const c of changes) {
    if (c.after.length === 0) continue;
    const first = Math.floor(c.offset / sectorSize);
    const last = Math.floor((c.offset + c.after.length - 1) / sectorSize);
    for (let s = first; s <= last; s++) set.add(s);
  }
  return [...set].sort((a, b) => a - b);
}

/** Bytes the FAT core re-parses as the boot sector after a raw write. */
export const BOOT_SECTOR_LEN = 512;

/** True when any change starts inside the boot sector. The core may then have adopted
 *  a new geometry, so the store must re-read `geometry` and `layout`. */
export function touchesBootSector(changes: ByteChangeLike[]): boolean {
  return changes.some((c) => c.offset < BOOT_SECTOR_LEN);
}
```

- [ ] **Step 5: Run the patch tests, then commit**

```bash
cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm exec vitest run tests/patch.test.ts
```

Expected: `tests/patch.test.ts (4 tests)` passes.

```bash
cd /Users/bsmall/dev/fs-emulator && git add web/ui/src/core/patch.ts web/ui/tests/patch.test.ts && git commit -m "fix(ui): skip empty byte changes and detect boot-sector writes in the patch helpers"
```

- [ ] **Step 6: Write the failing integration tests (a `writeRaw` op in the seek round trip; the root-entries layout refresh)**

In `/Users/bsmall/dev/fs-emulator/web/ui/tests/integration.test.ts`, make three edits.

Edit 6a, the imports (lines 3 and 7). Old text:

```ts
import { applyChanges, changedSectors } from "../src/core/patch";
```

New text:

```ts
import { applyChanges, changedSectors, touchesBootSector } from "../src/core/patch";
```

Old text:

```ts
import type { OpRecord, Volume as VolumeType } from "../src/lib/wasm";
```

New text:

```ts
import type { OpRecord, Region, Volume as VolumeType } from "../src/lib/wasm";
```

Edit 6b, the op list of the seek round trip. Old text:

```ts
    const ops: ((v: VolumeType) => OpRecord)[] = [
      (v) => v.createFile("/A.TXT", new TextEncoder().encode("alpha")),
      (v) => v.createDir("/D"),
      (v) => v.createFile("/D/B.BIN", new Uint8Array(3000)),
      (v) => v.deleteFile("/A.TXT"),
    ];
    const history: OpRecord[] = [];
    const snaps: Buffer[] = [];
    for (const op of ops) { history.push(op(vol)); snaps.push(Buffer.from(vol.image())); }
```

New text:

```ts
    const ops: ((v: VolumeType) => OpRecord)[] = [
      (v) => v.createFile("/A.TXT", new TextEncoder().encode("alpha")),
      (v) => v.createDir("/D"),
      (v) => v.writeRaw(43, new TextEncoder().encode("SHELLDISK  ")), // the volume label, inside sector 0
      (v) => v.createFile("/D/B.BIN", new Uint8Array(3000)),
      (v) => v.deleteFile("/A.TXT"),
    ];
    const history: OpRecord[] = [];
    const snaps: Buffer[] = [];
    for (const op of ops) { history.push(op(vol)); snaps.push(Buffer.from(vol.image())); }
    // The raw write journals one change for exactly the written range and lands in sector 0.
    expect(history[2].changes).toHaveLength(1);
    expect(history[2].changes[0].offset).toBe(43);
    expect(Array.from(history[2].changes[0].after)).toEqual(Array.from(new TextEncoder().encode("SHELLDISK  ")));
    expect(changedSectors(history[2].changes, 512)).toEqual([0]);
    expect(touchesBootSector(history[2].changes)).toBe(true);
    expect(history.filter((_, i) => i !== 2).some((r) => touchesBootSector(r.changes))).toBe(false);
```

Edit 6c, a new test appended inside `describe("package integration", ...)`, after the `it("finds a file's directory entry slots, ...")` block and before the closing `});` of the describe. New text:

```ts
  it("a raw write that changes the root-entry count is adopted: geometry and layout follow", () => {
    // 16 root entries fill exactly one 512-byte sector; 32 fill two. VolumeStore.run re-reads
    // geometry and layout whenever touchesBootSector(rec.changes) is true; this replays the
    // same wasm calls and checks that the volume reports the grown root region.
    const vol = Volume.formatFat16({ rootEntries: 16 });
    const before = vol.geometry();
    const layoutBefore = vol.layout();
    expect(before.rootEntries).toBe(16);
    expect(before.rootDirSectors).toBe(1);

    const rec = vol.writeRaw(17, new Uint8Array([32, 0])); // BPB_RootEntCnt, u16 little-endian
    expect(rec.changes).toHaveLength(1);
    expect(rec.changes[0].offset).toBe(17);
    expect(Array.from(rec.changes[0].before)).toEqual([16, 0]);
    expect(Array.from(rec.changes[0].after)).toEqual([32, 0]);
    expect(touchesBootSector(rec.changes)).toBe(true);

    const after = vol.geometry();
    const layoutAfter = vol.layout();
    expect(after.rootEntries).toBe(32);
    expect(after.rootDirSectors).toBe(2);
    expect(after.firstRootDirSector).toBe(before.firstRootDirSector);
    expect(after.firstDataSector).toBe(before.firstDataSector + 1);
    expect(after.bytesPerSector).toBe(before.bytesPerSector);
    expect(after.totalSectors).toBe(before.totalSectors);

    const root = (l: Region[]) => l.find((r) => r.kind === "directory")!;
    const data = (l: Region[]) => l.find((r) => r.kind === "data")!;
    expect(root(layoutBefore).sectors.start).toBe(before.firstRootDirSector);
    expect(root(layoutBefore).sectors.end).toBe(before.firstDataSector);
    expect(root(layoutAfter).sectors.start).toBe(before.firstRootDirSector);
    expect(root(layoutAfter).sectors.end).toBe(before.firstDataSector + 1); // grew by one sector
    expect(data(layoutAfter).sectors.start).toBe(data(layoutBefore).sectors.start + 1);
    expect(data(layoutAfter).sectors.end).toBe(data(layoutBefore).sectors.end);

    // Path operations keep working on the adopted geometry, and an ordinary op
    // (root entry, FAT, data cluster) never lands in sector 0.
    expect(vol.listDir("/")).toEqual([]);
    const rec2 = vol.createFile("/A.TXT", new TextEncoder().encode("a"));
    expect(touchesBootSector(rec2.changes)).toBe(false);
    expect(vol.listDir("/").map((e) => e.name)).toEqual(["A.TXT"]);
  });
```

- [ ] **Step 7: Run the integration tests**

```bash
cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm exec vitest run tests/integration.test.ts
```

Expected: `tests/integration.test.ts (5 tests)` passes. These tests pin the wasm contract the store relies on (Tasks 1 and 2 already shipped it), so they go green on the first run; the red step for this task's own logic was Step 3. If the round trip fails at `expect(history[2].changes).toHaveLength(1)`, or the layout test fails at `expect(after.rootEntries).toBe(32)`, the `node_modules` copy of the wasm package is stale: repeat the refresh in Step 1.

- [ ] **Step 8: Refresh geometry and layout in `VolumeStore.run`**

In `/Users/bsmall/dev/fs-emulator/web/ui/src/state/volume.svelte.ts`, make three edits.

Edit 8a, line 3. Old text:

```ts
import { applyChanges, changedSectors } from "../core/patch";
```

New text:

```ts
import { applyChanges, changedSectors, touchesBootSector } from "../core/patch";
```

Edit 8b, inside `run` (lines 61–63). Old text:

```ts
    this.history = [...this.history, rec];
    this.cursor = this.history.length - 1;
    this.refreshMeta();
```

New text:

```ts
    this.history = [...this.history, rec];
    this.cursor = this.history.length - 1;
    // A raw write into sector 0 may have been adopted as a new boot sector (the core
    // re-parses it), moving the root directory and data regions. Bytes per sector cannot
    // change (the core rejects that), so `sectorSize` and `zeros` stay valid.
    if (touchesBootSector(rec.changes)) { this.geometry = this.vol.geometry(); this.layout = this.vol.layout(); }
    this.refreshMeta();
```

Edit 8c, the `seek` doc comment (line 70). Old text:

```ts
  /** View the disk as it was after `step` (0-based). No wasm calls; patches the cached image. */
```

New text:

```ts
  /** View the disk as it was after `step` (0-based). No wasm calls; patches the cached image.
   *  `geometry` and `layout` stay at the latest state, like the tree and the layers, so a
   *  rewound view of a boot-sector change shows the older bytes under the newest layout. */
```

The `run` method now reads:

```ts
  /** Run a mutation at the latest state. Returns the record, or null on failure (status set). */
  run(fn: (v: Volume) => OpRecord): OpRecord | null {
    if (!this.atLatest) this.backToNow();
    let rec: OpRecord;
    try { rec = fn(this.vol); } catch (e) { this.fail(e); return null; }
    applyChanges(this.image, rec.changes, "forward");
    rescanSectors(this.zeros, this.image, this.sectorSize, changedSectors(rec.changes, this.sectorSize));
    this.history = [...this.history, rec];
    this.cursor = this.history.length - 1;
    // A raw write into sector 0 may have been adopted as a new boot sector (the core
    // re-parses it), moving the root directory and data regions. Bytes per sector cannot
    // change (the core rejects that), so `sectorSize` and `zeros` stay valid.
    if (touchesBootSector(rec.changes)) { this.geometry = this.vol.geometry(); this.layout = this.vol.layout(); }
    this.refreshMeta();
    this.status = null;
    this.epoch++;
    if (import.meta.env.DEV) this.checkInvariant();
    return rec;
  }
```

`attribution` is `$derived(buildAttribution(this.geometry, this.layout, this.owners))`, so assigning the two `$state.raw` fields is enough for the ribbon, inspector, and hex-dump attribution to follow.

- [ ] **Step 9: Add the `OutOfBounds` sentence to the StatusLine copy**

In `/Users/bsmall/dev/fs-emulator/web/ui/src/components/StatusLine.svelte`, line 19. Old text:

```ts
    CorruptImage: "That image doesn't look like a valid FAT16 volume.",
```

New text:

```ts
    CorruptImage: "That image doesn't look like a valid FAT16 volume.",
    OutOfBounds: "That range runs past the end of the disk.",
```

No vitest covers Svelte components; `svelte-check` in the next step type-checks the map and the component.

- [ ] **Step 10: Run the full gate and commit**

```bash
cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm test && pnpm build
```

Expected: `Test Files  12 passed (12)`, `Tests  49 passed (49)` (46 baseline + 2 in `patch.test.ts` + 1 in `integration.test.ts`); then `svelte-check found 0 errors and 0 warnings` and `vite build` completes with `✓ built in ...`.

```bash
cd /Users/bsmall/dev/fs-emulator && git add web/ui/src/state/volume.svelte.ts web/ui/src/components/StatusLine.svelte web/ui/tests/integration.test.ts && git commit -m "feat(ui): refresh geometry and layout after a raw write into the boot sector"
```

Manual check (optional now, required in Task 7's script): with `pnpm dev` and after Task 7 wires the terminal, `echo 'SHELLDISK  ' | dd --of=/dev/hda --bs=1 --seek=43` must add a `write_raw 0x2b +11` step and the sector-0 annotation must show the new label; `dd --if=/dev/zero --of=/dev/hda --count=1` must leave the hex dump rendering while `ls /mnt` reports the corruption; a `dd` with `--seek` past the disk end must print `/dev/hda: Range runs past the end of the disk` in the terminal and leave the StatusLine and timeline unchanged (the shell checks the range before calling the store; the `OutOfBounds` StatusLine copy is the fallback for a range the shell let through).


---

### Task 4: Shell pure modules and dependencies

Spec: `docs/superpowers/specs/2026-09-22-terminal-access-design.md`, section 4 (through `dd.ts`'s `parseDd`) and the dependency/vite lines of section 5. Everything in this task is plain TypeScript under `web/ui/src/shell/`, tested under node by vitest through the real `fs-emulator-wasm` package. No Rust changes. Nothing here imports `@benjamin-small/browser-terminal` at runtime: every reference is `import type`, which `verbatimModuleSyntax` erases. This task does not need Task 2's `readRaw`/`writeRaw`/`corruption()` (the `pkg` on disk is enough); it can run beside Tasks 1–3.

All commands below run from `/Users/bsmall/dev/fs-emulator/web/ui` unless stated otherwise. Prerequisite: `crates/wasm/pkg` must exist (`wasm-pack build crates/wasm --target bundler` from the repo root if it is missing), because `package.json` depends on `file:../../crates/wasm/pkg`.

**Files:**

- Modify: `web/ui/package.json` and `web/ui/pnpm-lock.yaml` (via the two `pnpm add` commands in Step 1; never by hand)
- Modify: `web/ui/vite.config.ts` (line 11, `optimizeDeps`)
- Create: `web/ui/src/shell/types.ts`
- Create: `web/ui/src/shell/errors.ts`
- Create: `web/ui/src/shell/host.ts`
- Create: `web/ui/src/shell/bytes.ts`
- Create: `web/ui/src/shell/addr.ts`
- Create: `web/ui/src/shell/vfs.ts`
- Create: `web/ui/src/shell/xxd.ts`
- Create: `web/ui/src/shell/dd.ts`
- Modify: `web/ui/src/components/HexView.svelte` (import block lines 1–9; `jump()` at lines 105–113)
- Create: `web/ui/tests/fixtures/geometry.ts`
- Modify: `web/ui/tests/attribution.test.ts` (lines 4–15: the fixture definitions move out)
- Test: `web/ui/tests/shell/errors.test.ts`, `web/ui/tests/shell/bytes.test.ts`, `web/ui/tests/shell/addr.test.ts`, `web/ui/tests/shell/vfs.test.ts`, `web/ui/tests/shell/xxd.test.ts`, `web/ui/tests/shell/dd.test.ts`

**Interfaces:**

Consumes (all already on disk):

- `web/ui/src/lib/wasm.ts` barrel: `Volume` (class; `Volume.formatFat16(options: FormatOptions | undefined): Volume`, `listDir(path: string): EntryInfo[]`, `createDir(path: string): OpRecord`, `createFile(path: string, data: Uint8Array): OpRecord`, `readFile(path: string): Uint8Array`), and the types `FormatOptions`, `Geometry`, `OpRecord`, `Region`, `FsError extends Error { code: string }`.
- `@benjamin-small/browser-terminal@0.2.0` `dist/index.d.ts` type exports (verified in `packages/browser-terminal/src/index.ts` and `src/types.ts` of that repo): `Value = null | boolean | number | string | Value[] | { [key: string]: Value }`, `CommandSpec { name; summary?; required?: PosArg[]; optional?: PosArg[]; rest?: PosArg; flags?: FlagSpec[] }`, `CommandArgs { positionals: Value[]; flags: Record<string, Value> }`, `CommandCtx { signal: AbortSignal; log: ChannelWriter; err: ChannelWriter; emit: ChannelWriter }`, `CommandFn = (args: CommandArgs, input: Value, ctx: CommandCtx) => unknown | Promise<unknown>`.
- `tests/attribution.test.ts` currently exports `geo: Geometry` and `layout: Region[]` (lines 6–13); they move to the fixture file.

Produces (exact names later tasks rely on):

```ts
// src/shell/types.ts
export type { CommandArgs, CommandCtx, CommandFn, CommandSpec, FlagSpec, PosArg, Value } from "@benjamin-small/browser-terminal";
export interface CommandDef { spec: CommandSpec; fn: CommandFn }

// src/shell/errors.ts
export class ShellError extends Error { help?: string; code?: string; constructor(message: string, opts?: { help?: string; code?: string }) }
export function fsPhrase(code: string | undefined, raw: string): string
export function wrapFs(display: string, e: unknown): ShellError      // `${display}: ${phrase}`; a ShellError passes through unchanged

// src/shell/host.ts
export interface ShellHost { readonly vol: Volume; readonly cursor: number; readonly historyLength: number; run(fn: (v: Volume) => OpRecord): OpRecord; format(options: FormatOptions): void; select(path: string | null): void; jumpTo(offset: number): void; closeTerminal(): void }
export const atLatest: (h: ShellHost) => boolean
export function statusToError(s: { text: string; code?: string } | null): Error & { code?: string }

// src/shell/bytes.ts
export type BytesBlob = { bytes: string; length: number }   // a type alias so it is assignable to Value
export function isBlob(v: unknown): v is BytesBlob
export function fromBytes(b: Uint8Array): BytesBlob
export function toBytes(v: Value): Uint8Array
export function decodeText(b: Uint8Array): string
export function hex(b: Uint8Array): string
export function unhex(s: string): Uint8Array
export const BYTES_HELP: string

// src/shell/addr.ts
export const ADDR_HELP: string
export const SIZE_HELP: string
export type AddrGeometry = Pick<Geometry, "bytesPerSector" | "sectorsPerCluster" | "firstDataSector">
export function parseAddr(v: string | number, g: AddrGeometry): number   // non-negative integer or throws ShellError(`bad address '…'`, { help: ADDR_HELP })
export function parseSize(v: string | number): number                    // non-negative integer or throws ShellError(`bad size '…'`, { help: SIZE_HELP })

// src/shell/vfs.ts
export type Resolved = { kind: "root" } | { kind: "dev" } | { kind: "raw" } | { kind: "zero" } | { kind: "null" } | { kind: "volume"; path: string }
export const MOUNT = "/mnt"
export const PATH_HELP: string
export const DEVICE_HELP: string
export class Vfs { cwd: string; normalize(input: string): string; resolve(input: string): Resolved; display(r: Resolved): string; toVirtual(volumePath: string): string }
export function canonicalize(vol: Volume, volumePath: string): string
export function basename(p: string): string
export function joinVolume(dir: string, name: string): string

// src/shell/xxd.ts
export function formatXxd(bytes: Uint8Array, base: number, cols?: number): string   // throws ShellError("--cols must be 1..64") outside that range

// src/shell/dd.ts
export const DD_MAX_BYTES: number            // 1 << 20
export const DD_KEYS: readonly ["if", "of", "bs", "count", "skip", "seek"]
export type DdKey = "if" | "of" | "bs" | "count" | "skip" | "seek"
export interface DdOpts { if?: string; of?: string; bs: number; count?: number; skip: number; seek: number }
export const OPERAND_HELP: string
export function parseDd(flags: Record<string, Value>, operands: Value[]): DdOpts
export interface DdWindow { start: number; len: number }
export function planWindow(opts: Pick<DdOpts, "bs" | "count" | "skip">, available: number): DdWindow   // enforces DD_MAX_BYTES on the read window
export function formatRecords(len: number, bs: number): string   // "1+0 records"

// tests/fixtures/geometry.ts
export const geo: Geometry
export const layout: Region[]
```

---

- [ ] **Step 1: Add the terminal dependencies and the Vite exclude**

Run exactly these commands (the `^` is quoted so zsh does not expand it):

```sh
cd /Users/bsmall/dev/fs-emulator/web/ui
pnpm add --save-exact @benjamin-small/browser-terminal@0.2.0
pnpm add "@xterm/xterm@^6.0.0"
```

Expected: `package.json` `dependencies` becomes (pnpm keeps keys sorted):

```json
  "dependencies": {
    "@benjamin-small/browser-terminal": "0.2.0",
    "@xterm/xterm": "^6.0.0",
    "fs-emulator-wasm": "file:../../crates/wasm/pkg"
  },
```

and `pnpm-lock.yaml` gains `@benjamin-small/browser-terminal` (0.2.0, with its `@xterm/addon-fit` and `@xterm/xterm` dependencies) and `@xterm/xterm` entries. If `@xterm/xterm` lands with a different range, edit only that one value in `package.json` back to `^6.0.0` and rerun `pnpm install` so the lockfile's `specifier` matches.

Then edit `web/ui/vite.config.ts`. Old text (line 11):

```ts
  optimizeDeps: { exclude: ["fs-emulator-wasm"] },
```

New text:

```ts
  // Both packages load their .wasm via `new URL(..., import.meta.url)`; pre-bundling would break that.
  optimizeDeps: { exclude: ["fs-emulator-wasm", "@benjamin-small/browser-terminal"] },
```

Verify: `pnpm install --frozen-lockfile && pnpm test && pnpm build` all pass (49 tests, 12 files after Task 3; svelte-check reports 0 errors). Confirm the installed types exist: `ls node_modules/@benjamin-small/browser-terminal/dist/index.d.ts`.

Commit:

```sh
git add web/ui/package.json web/ui/pnpm-lock.yaml web/ui/vite.config.ts
git commit -m "chore(ui): add browser-terminal 0.2.0 and xterm, exclude the terminal from pre-bundling"
```

- [ ] **Step 2: Move the geometry fixtures out of the attribution test**

Importing `geo` from `tests/attribution.test.ts` in another test would re-collect its `describe` blocks, so the fixtures get their own module.

Create `web/ui/tests/fixtures/geometry.ts`:

```ts
import type { Geometry, Region } from "../../src/lib/wasm";

// The default `Volume.formatFat16(undefined)` disk: 16 MiB, 512-byte sectors, 4 sectors per
// cluster, two 32-sector FATs, a 32-sector root directory, data from sector 97.
export const geo: Geometry = { variant: "fat16", bytesPerSector: 512, sectorsPerCluster: 4, reservedSectors: 1, fatCount: 2, sectorsPerFat: 32, rootEntries: 512, rootDirSectors: 32, firstRootDirSector: 65, firstDataSector: 97, totalSectors: 32768, clusterCount: 8167 };
export const layout: Region[] = [
  { name: "reserved (boot sector)", sectors: { start: 0, end: 1 }, kind: "boot" },
  { name: "FAT 0", sectors: { start: 1, end: 33 }, kind: "allocationTable" },
  { name: "FAT 1", sectors: { start: 33, end: 65 }, kind: "allocationTable" },
  { name: "root directory", sectors: { start: 65, end: 97 }, kind: "directory" },
  { name: "data", sectors: { start: 97, end: 32768 }, kind: "data" },
];
```

Edit `web/ui/tests/attribution.test.ts`. Old text (lines 4–13):

```ts
import type { ClusterOwner, Geometry, Region } from "../src/lib/wasm";

export const geo: Geometry = { variant: "fat16", bytesPerSector: 512, sectorsPerCluster: 4, reservedSectors: 1, fatCount: 2, sectorsPerFat: 32, rootEntries: 512, rootDirSectors: 32, firstRootDirSector: 65, firstDataSector: 97, totalSectors: 32768, clusterCount: 8167 };
export const layout: Region[] = [
  { name: "reserved (boot sector)", sectors: { start: 0, end: 1 }, kind: "boot" },
  { name: "FAT 0", sectors: { start: 1, end: 33 }, kind: "allocationTable" },
  { name: "FAT 1", sectors: { start: 33, end: 65 }, kind: "allocationTable" },
  { name: "root directory", sectors: { start: 65, end: 97 }, kind: "directory" },
  { name: "data", sectors: { start: 97, end: 32768 }, kind: "data" },
];
```

New text:

```ts
import type { ClusterOwner } from "../src/lib/wasm";
import { geo, layout } from "./fixtures/geometry";

```

(The blank line before `const owners` stays.) Run `pnpm test tests/attribution.test.ts`: 4 tests pass. Commit:

```sh
git add web/ui/tests/fixtures/geometry.ts web/ui/tests/attribution.test.ts
git commit -m "test(ui): move the geometry fixtures to tests/fixtures"
```

- [ ] **Step 3: Write the failing test for errors.ts and host.ts**

Create `web/ui/tests/shell/errors.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { ShellError, fsPhrase, wrapFs } from "../../src/shell/errors";
import { atLatest, statusToError, type ShellHost } from "../../src/shell/host";

describe("ShellError", () => {
  it("is an Error carrying help and code", () => {
    const e = new ShellError("nothing to write", { help: "pipe text in", code: "X" });
    expect(e).toBeInstanceOf(Error);
    expect(e.message).toBe("nothing to write");
    expect(e.help).toBe("pipe text in");
    expect(e.code).toBe("X");
    expect(e.name).toBe("ShellError");
  });
  it("leaves help and code unset when not given (the engine reads them by Reflect.get)", () => {
    const e = new ShellError("plain");
    expect("help" in e).toBe(false);
    expect("code" in e).toBe(false);
  });
});

describe("wrapFs", () => {
  it("uses coreutils phrases for the mapped codes and the raw message otherwise", () => {
    expect(fsPhrase("NotFound", "x")).toBe("No such file or directory");
    expect(fsPhrase("AlreadyExists", "x")).toBe("File exists");
    expect(fsPhrase("IsADirectory", "x")).toBe("Is a directory");
    expect(fsPhrase("NotADirectory", "x")).toBe("Not a directory");
    expect(fsPhrase("DirectoryNotEmpty", "x")).toBe("Directory not empty");
    expect(fsPhrase("DiskFull", "x")).toBe("No space left on device");
    expect(fsPhrase("OutOfBounds", "x")).toBe("Range runs past the end of the disk");
    expect(fsPhrase("CorruptImage", "boot sector no longer parses")).toBe("boot sector no longer parses");
    expect(fsPhrase(undefined, "raw text")).toBe("raw text");
  });
  it("wraps a real wasm error as `display: phrase` with no command prefix and keeps the code", () => {
    const vol = Volume.formatFat16(undefined);
    let caught: unknown;
    try { vol.readFile("/NOPE.TXT"); } catch (e) { caught = e; }
    const w = wrapFs("/mnt/NOPE.TXT", caught);
    expect(w).toBeInstanceOf(ShellError);
    expect(w.message).toBe("/mnt/NOPE.TXT: No such file or directory");
    expect(w.code).toBe("NotFound");
  });
  it("passes a ShellError through untouched and stringifies non-errors", () => {
    const own = new ShellError("give --at <addr>", { help: "h" });
    expect(wrapFs("/dev/hda", own)).toBe(own);
    expect(wrapFs("/mnt/a", "boom").message).toBe("/mnt/a: boom");
    expect(wrapFs("/mnt/a", { message: "custom", code: "Weird" }).message).toBe("/mnt/a: custom");
    expect(wrapFs("/mnt/a", { message: "custom", code: "Weird" }).code).toBe("Weird");
  });
});

describe("host helpers", () => {
  const stub = (cursor: number, historyLength: number): ShellHost => ({
    vol: Volume.formatFat16(undefined), cursor, historyLength,
    run: () => { throw new Error("unused"); }, format: () => {}, select: () => {}, jumpTo: () => {}, closeTerminal: () => {},
  });
  it("atLatest is true before any op and at the last step only", () => {
    expect(atLatest(stub(-1, 0))).toBe(true);
    expect(atLatest(stub(2, 3))).toBe(true);
    expect(atLatest(stub(1, 3))).toBe(false);
  });
  it("statusToError turns the store's status into a throwable with the code", () => {
    const e = statusToError({ text: "disk full", code: "DiskFull" });
    expect(e).toBeInstanceOf(Error);
    expect(e.message).toBe("disk full");
    expect(e.code).toBe("DiskFull");
    expect(statusToError(null).message).toBe("the operation failed without a message");
  });
});
```

Run `pnpm test tests/shell/errors.test.ts`. Expected failure: `Error: Failed to resolve import "../../src/shell/errors" from "tests/shell/errors.test.ts". Does the file exist?` (the suite fails to load).

- [ ] **Step 4: Create types.ts, errors.ts, and host.ts**

Create `web/ui/src/shell/types.ts`:

```ts
// Type-only re-exports of browser-terminal's public command types. `verbatimModuleSyntax`
// erases all of this at runtime, so nothing under src/shell/ loads the terminal package;
// only TerminalPanel.svelte (via storeHost.svelte.ts) ever imports it for real.
import type { CommandFn, CommandSpec } from "@benjamin-small/browser-terminal";

export type { CommandArgs, CommandCtx, CommandFn, CommandSpec, FlagSpec, PosArg, Value } from "@benjamin-small/browser-terminal";

/** One registered command. The drawer wires it with `bt.registerCommand(def.spec, def.fn)`. */
export interface CommandDef {
  spec: CommandSpec;
  fn: CommandFn;
}
```

Create `web/ui/src/shell/errors.ts`:

```ts
/**
 * Errors thrown by shell commands. browser-terminal reads `message` and `help` off a thrown
 * object (js_command.rs `js_error_to_shell`) and prefixes the command name itself, so no
 * message here ever starts with a command name.
 */
export class ShellError extends Error {
  // `declare`: with ES2022 class fields a plain `help?: string;` would define an own
  // `undefined` property; these stay absent until set, so `"help" in e` mirrors what was given.
  declare help?: string;
  declare code?: string;

  constructor(message: string, opts: { help?: string; code?: string } = {}) {
    super(message);
    this.name = "ShellError";
    if (opts.help !== undefined) this.help = opts.help;
    if (opts.code !== undefined) this.code = opts.code;
  }
}

/** Coreutils phrasing for the wasm error codes a shell user meets most; other codes keep the wasm text. */
const PHRASES: Record<string, string> = {
  NotFound: "No such file or directory",
  AlreadyExists: "File exists",
  IsADirectory: "Is a directory",
  NotADirectory: "Not a directory",
  DirectoryNotEmpty: "Directory not empty",
  DiskFull: "No space left on device",
  OutOfBounds: "Range runs past the end of the disk",
};

export function fsPhrase(code: string | undefined, raw: string): string {
  return (code !== undefined && PHRASES[code]) || raw;
}

/** `${display}: ${phrase}` for anything a Volume call threw. A ShellError is already phrased and passes through. */
export function wrapFs(display: string, e: unknown): ShellError {
  if (e instanceof ShellError) return e;
  const err = typeof e === "object" && e !== null ? (e as { message?: unknown; code?: unknown }) : null;
  const raw = typeof err?.message === "string" ? err.message : String(e);
  const code = typeof err?.code === "string" ? err.code : undefined;
  return new ShellError(`${display}: ${fsPhrase(code, raw)}`, { code });
}
```

Create `web/ui/src/shell/host.ts`:

```ts
import type { FormatOptions, OpRecord, Volume } from "../lib/wasm";
import { ShellError } from "./errors";

/**
 * The seam between the commands and the explorer. The app implements it over the runes
 * stores (storeHost.svelte.ts); tests implement it over a plain Volume with an in-memory
 * history. Paths given to `select` are volume paths ("/A/B"), never "/mnt/A/B".
 */
export interface ShellHost {
  /** The latest volume; every read goes here even while the timeline is rewound. */
  readonly vol: Volume;
  /** Timeline position: -1 before any op, `historyLength - 1` at the latest step. */
  readonly cursor: number;
  readonly historyLength: number;
  /** Run one journaled mutation at the latest state. Throws `{ message, code? }` on failure. */
  run(fn: (v: Volume) => OpRecord): OpRecord;
  /** Replace the volume; the store discards the history. Throws `{ message, code? }` on failure. */
  format(options: FormatOptions): void;
  select(path: string | null): void;
  jumpTo(offset: number): void;
  closeTerminal(): void;
}

export const atLatest = (h: ShellHost): boolean => h.cursor === h.historyLength - 1;

/** What the store adapter throws when `VolumeStore.run`/`format` left a status instead of a record. */
export function statusToError(s: { text: string; code?: string } | null): Error & { code?: string } {
  if (s === null) return new ShellError("the operation failed without a message");
  return new ShellError(s.text, { code: s.code });
}
```

Run `pnpm test tests/shell/errors.test.ts`: 7 tests pass. Commit:

```sh
git add web/ui/src/shell/types.ts web/ui/src/shell/errors.ts web/ui/src/shell/host.ts web/ui/tests/shell/errors.test.ts
git commit -m "feat(ui): shell error, host, and command types"
```

- [ ] **Step 5: Write the failing test for bytes.ts**

Create `web/ui/tests/shell/bytes.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { BYTES_HELP, decodeText, fromBytes, hex, isBlob, toBytes, unhex } from "../../src/shell/bytes";
import { ShellError } from "../../src/shell/errors";

const u8 = (...b: number[]) => new Uint8Array(b);

describe("hex and blobs", () => {
  it("hex/unhex round trip including 0x00 and 0xff", () => {
    expect(hex(u8(0, 255, 16))).toBe("00ff10");
    expect(unhex("00ff10")).toEqual(u8(0, 255, 16));
    expect(unhex("")).toEqual(u8());
    expect(hex(u8())).toBe("");
  });
  it("fromBytes makes a lowercase-hex blob and toBytes reads it back", () => {
    const blob = fromBytes(u8(0, 255, 16));
    expect(blob).toEqual({ bytes: "00ff10", length: 3 });
    expect(isBlob(blob)).toBe(true);
    expect(toBytes(blob)).toEqual(u8(0, 255, 16));
  });
  it("isBlob rejects odd hex, uppercase, a mismatched length, and non-records", () => {
    expect(isBlob({ bytes: "abc", length: 1 })).toBe(false);
    expect(isBlob({ bytes: "AB", length: 1 })).toBe(false);
    expect(isBlob({ bytes: "ab", length: 2 })).toBe(false);
    expect(isBlob({ bytes: "", length: 0 })).toBe(true);
    expect(isBlob(null)).toBe(false);
    expect(isBlob("ab")).toBe(false);
    expect(isBlob(["ab", 1])).toBe(false);
    expect(isBlob({ bytes: 12, length: 1 })).toBe(false);
  });
});

describe("toBytes (the pipe convention)", () => {
  it("strings are UTF-8", () => {
    expect(toBytes("héllo")).toEqual(u8(104, 195, 169, 108, 108, 111));
    expect(toBytes("")).toEqual(u8());
  });
  it("numbers and booleans are their String() form", () => {
    expect(toBytes(12345)).toEqual(new TextEncoder().encode("12345"));
    expect(toBytes(true)).toEqual(new TextEncoder().encode("true"));
  });
  it("a list of scalars is joined by one space, exactly what `echo a b` produces", () => {
    expect(decodeText(toBytes(["hello", "world"]))).toBe("hello world");
    expect(decodeText(toBytes([72, 105]))).toBe("72 105"); // never a byte list
    expect(decodeText(toBytes(["a", 1, true, null]))).toBe("a 1 true ");
    expect(toBytes([])).toEqual(u8());
  });
  it("null (nothing piped) is empty", () => {
    expect(toBytes(null)).toEqual(u8());
  });
  it("records and nested lists throw a ShellError with help", () => {
    for (const v of [{ a: 1 }, [["x"]], [{ a: 1 }]]) {
      let caught: unknown;
      try { toBytes(v as never); } catch (e) { caught = e; }
      expect(caught).toBeInstanceOf(ShellError);
      expect((caught as ShellError).message).toMatch(/^expected text or bytes, found /);
      expect((caught as ShellError).help).toBe(BYTES_HELP);
    }
    let caught: unknown;
    try { toBytes({ a: 1 }); } catch (e) { caught = e; }
    expect((caught as ShellError).message).toBe("expected text or bytes, found a record");
    try { toBytes([["x"]]); } catch (e) { caught = e; }
    expect((caught as ShellError).message).toBe("expected text or bytes, found a list with nested values");
  });
});

describe("decodeText", () => {
  it("decodes UTF-8 and never throws on bad sequences", () => {
    expect(decodeText(u8(104, 195, 169))).toBe("hé");
    expect(decodeText(u8(0xff, 0x41))).toBe("\uFFFDA");
  });
});
```

Run `pnpm test tests/shell/bytes.test.ts`. Expected failure: `Failed to resolve import "../../src/shell/bytes"`.

- [ ] **Step 6: Create bytes.ts**

Create `web/ui/src/shell/bytes.ts`:

```ts
import type { Value } from "./types";
import { ShellError } from "./errors";

/**
 * The pipe convention. browser-terminal's `Value` has no bytes type, so a string is UTF-8
 * text and a `BytesBlob` record carries raw bytes losslessly as lowercase hex. A blob that
 * reaches the terminal renders as a key/value record; users pipe it into `xxd` or `write`.
 */
// A `type`, not an `interface`: only object type literals get the implicit index signature
// that makes a blob assignable to browser-terminal's `Value` record type.
export type BytesBlob = { bytes: string; length: number };

export const BYTES_HELP = "pipe text (echo hi), a blob from `cat --bytes` or `dd`, or a list of words";

const HEX_RE = /^([0-9a-f]{2})*$/;

export function isBlob(v: unknown): v is BytesBlob {
  if (typeof v !== "object" || v === null || Array.isArray(v)) return false;
  const { bytes, length } = v as { bytes?: unknown; length?: unknown };
  return typeof bytes === "string" && typeof length === "number" && HEX_RE.test(bytes) && length === bytes.length / 2;
}

export function hex(b: Uint8Array): string {
  let s = "";
  for (let i = 0; i < b.length; i++) s += b[i].toString(16).padStart(2, "0");
  return s;
}

export function unhex(s: string): Uint8Array {
  const out = new Uint8Array(s.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(s.slice(2 * i, 2 * i + 2), 16);
  return out;
}

export function fromBytes(b: Uint8Array): BytesBlob {
  return { bytes: hex(b), length: b.length };
}

export function decodeText(b: Uint8Array): string {
  return new TextDecoder("utf-8", { fatal: false }).decode(b);
}

type Scalar = string | number | boolean | null;
const isScalar = (v: Value): v is Scalar => v === null || typeof v !== "object";
const scalarText = (v: Scalar): string => (v === null ? "" : String(v));

function describe(v: Value): string {
  if (v === null) return "null";
  if (Array.isArray(v)) return "a list with nested values";
  if (typeof v === "object") return "a record";
  return typeof v;
}

/**
 * string → UTF-8; number or boolean → `String(v)`; list of scalars → items joined by one
 * space (what `echo a b` yields); blob → its bytes; null → empty; anything else throws.
 */
export function toBytes(v: Value): Uint8Array {
  if (v === null) return new Uint8Array(0);
  if (typeof v === "string") return new TextEncoder().encode(v);
  if (typeof v === "number" || typeof v === "boolean") return new TextEncoder().encode(String(v));
  if (isBlob(v)) return unhex(v.bytes);
  if (Array.isArray(v) && v.every(isScalar)) return new TextEncoder().encode(v.map(scalarText).join(" "));
  throw new ShellError(`expected text or bytes, found ${describe(v)}`, { help: BYTES_HELP });
}
```

Run `pnpm test tests/shell/bytes.test.ts`: 9 tests pass. Commit:

```sh
git add web/ui/src/shell/bytes.ts web/ui/tests/shell/bytes.test.ts
git commit -m "feat(ui): shell byte conventions (UTF-8 text, hex blobs, echo lists)"
```

- [ ] **Step 7: Write the failing test for addr.ts**

Create `web/ui/tests/shell/addr.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { ADDR_HELP, SIZE_HELP, parseAddr, parseSize } from "../../src/shell/addr";
import { ShellError } from "../../src/shell/errors";
import { geo } from "../fixtures/geometry";

function thrown(fn: () => unknown): ShellError {
  try { fn(); } catch (e) { return e as ShellError; }
  throw new Error("expected a throw");
}

describe("parseAddr", () => {
  it("accepts sectors, clusters, hex, decimal strings, and integers (HexView's g prompt forms)", () => {
    expect(parseAddr("s:65", geo)).toBe(65 * 512);
    expect(parseAddr("S:0", geo)).toBe(0);
    expect(parseAddr("c:2", geo)).toBe(97 * 512);
    expect(parseAddr("c:3", geo)).toBe(97 * 512 + 2048);
    expect(parseAddr("0x1F", geo)).toBe(31);
    expect(parseAddr("0X1f", geo)).toBe(31);
    expect(parseAddr("512", geo)).toBe(512);
    expect(parseAddr(512, geo)).toBe(512);
    expect(parseAddr(0, geo)).toBe(0);
  });
  it("throws a ShellError naming the input, with ADDR_HELP", () => {
    for (const bad of ["-1", "s:x", "1k", "", "c:", "0x", "12 34"]) {
      const e = thrown(() => parseAddr(bad, geo));
      expect(e).toBeInstanceOf(ShellError);
      expect(e.message).toBe(`bad address '${bad}'`);
      expect(e.help).toBe(ADDR_HELP);
    }
    expect(thrown(() => parseAddr(-3, geo)).message).toBe("bad address '-3'");
    expect(thrown(() => parseAddr(1.5, geo)).message).toBe("bad address '1.5'");
    // c:0 lands before the data region on this geometry: 97*512 - 2*2048 is still >= 0, but a
    // tiny geometry can go negative, and negative is never an address.
    expect(thrown(() => parseAddr("c:0", { bytesPerSector: 512, sectorsPerCluster: 8, firstDataSector: 4 })).message).toBe("bad address 'c:0'");
  });
});

describe("parseSize", () => {
  it("accepts plain, k/M-suffixed, and hex sizes, and integers", () => {
    expect(parseSize("512")).toBe(512);
    expect(parseSize("1k")).toBe(1024);
    expect(parseSize("2K")).toBe(2048);
    expect(parseSize("4M")).toBe(4 * 1048576);
    expect(parseSize("1m")).toBe(1048576);
    expect(parseSize("0x200")).toBe(512);
    expect(parseSize(512)).toBe(512);
    expect(parseSize("0")).toBe(0);
  });
  it("throws a ShellError with SIZE_HELP", () => {
    for (const bad of ["abc", "1kb", "-1", "", "k"]) {
      const e = thrown(() => parseSize(bad));
      expect(e).toBeInstanceOf(ShellError);
      expect(e.message).toBe(`bad size '${bad}'`);
      expect(e.help).toBe(SIZE_HELP);
    }
    expect(thrown(() => parseSize(-1)).message).toBe("bad size '-1'");
    expect(thrown(() => parseSize(2.5)).message).toBe("bad size '2.5'");
  });
});
```

Run `pnpm test tests/shell/addr.test.ts`. Expected failure: `Failed to resolve import "../../src/shell/addr"`.

- [ ] **Step 8: Create addr.ts and make HexView's jump delegate to it**

Create `web/ui/src/shell/addr.ts`:

```ts
import type { Geometry } from "../lib/wasm";
import { ShellError } from "./errors";

export const ADDR_HELP = "addresses: 0x1f (hex), 512 (decimal), s:65 (sector), c:3 (cluster)";
export const SIZE_HELP = "sizes: 512, 1k, 4M, 0x200";

export type AddrGeometry = Pick<Geometry, "bytesPerSector" | "sectorsPerCluster" | "firstDataSector">;

/**
 * The address forms HexView's `g` prompt accepts: `s:N` (sector), `c:N` (cluster, data
 * clusters start at 2), `0x…`, or decimal. A number passes through when it is a
 * non-negative integer. Bounds are the caller's business (HexView checks the image length,
 * `seek` the disk size).
 */
export function parseAddr(v: string | number, g: AddrGeometry): number {
  const bad = () => new ShellError(`bad address '${v}'`, { help: ADDR_HELP });
  let off: number;
  if (typeof v === "number") off = v;
  else if (/^s:\d+$/i.test(v)) off = Number(v.slice(2)) * g.bytesPerSector;
  else if (/^c:\d+$/i.test(v)) off = g.firstDataSector * g.bytesPerSector + (Number(v.slice(2)) - 2) * g.bytesPerSector * g.sectorsPerCluster;
  else if (/^0x[0-9a-f]+$/i.test(v)) off = parseInt(v, 16);
  else if (/^\d+$/.test(v)) off = Number(v);
  else throw bad();
  if (!Number.isInteger(off) || off < 0) throw bad();
  return off;
}

/** Byte counts: `512`, `1k`, `4M`, `0x200`, or a non-negative integer. Zero is allowed; callers decide. */
export function parseSize(v: string | number): number {
  const bad = () => new ShellError(`bad size '${v}'`, { help: SIZE_HELP });
  let n: number;
  if (typeof v === "number") n = v;
  else {
    const m = /^(\d+)([kKmM]?)$/.exec(v);
    if (m) n = Number(m[1]) * (m[2] === "" ? 1 : m[2].toLowerCase() === "k" ? 1024 : 1048576);
    else if (/^0x[0-9a-f]+$/i.test(v)) n = parseInt(v, 16);
    else throw bad();
  }
  if (!Number.isInteger(n) || n < 0) throw bad();
  return n;
}
```

Edit `web/ui/src/components/HexView.svelte`. First the import block. Old text (lines 8–9):

```ts
  import { contains } from "../core/intervals";
  import HexRow from "./HexRow.svelte";
```

New text:

```ts
  import { contains } from "../core/intervals";
  import { parseAddr } from "../shell/addr";
  import HexRow from "./HexRow.svelte";
```

Then `jump`. Old text (lines 106–114 after the import insertion; the function starting `function jump(v: string) {`):

```ts
  function jump(v: string) {
    const g = volume.geometry;
    let off: number | null = null;
    if (/^s:\d+$/i.test(v)) off = Number(v.slice(2)) * g.bytesPerSector;
    else if (/^c:\d+$/i.test(v)) off = g.firstDataSector * g.bytesPerSector + (Number(v.slice(2)) - 2) * g.bytesPerSector * g.sectorsPerCluster;
    else if (/^0x[0-9a-f]+$/i.test(v)) off = parseInt(v, 16);
    else if (/^\d+$/.test(v)) off = Number(v);
    if (off !== null && off >= 0 && off < volume.image.length) selection.jumpTo(off);
  }
```

New text:

```ts
  function jump(v: string) {
    let off: number;
    try { off = parseAddr(v, volume.geometry); } catch { return; } // the prompt ignores bad input silently, as before
    if (off >= 0 && off < volume.image.length) selection.jumpTo(off);
  }
```

Run `pnpm test tests/shell/addr.test.ts` (4 tests pass) and `pnpm build` (svelte-check: 0 errors; the Svelte component compiles). Commit:

```sh
git add web/ui/src/shell/addr.ts web/ui/src/components/HexView.svelte web/ui/tests/shell/addr.test.ts
git commit -m "feat(ui): extract address and size parsing from HexView into shell/addr"
```

- [ ] **Step 9: Write the failing test for vfs.ts**

Create `web/ui/tests/shell/vfs.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { ShellError } from "../../src/shell/errors";
import { DEVICE_HELP, MOUNT, PATH_HELP, Vfs, basename, canonicalize, joinVolume } from "../../src/shell/vfs";

function thrown(fn: () => unknown): ShellError {
  try { fn(); } catch (e) { return e as ShellError; }
  throw new Error("expected a throw");
}

describe("Vfs.normalize", () => {
  it("starts at /mnt and joins relative paths to cwd", () => {
    const v = new Vfs();
    expect(v.cwd).toBe(MOUNT);
    expect(v.normalize("")).toBe("/mnt");
    expect(v.normalize("a")).toBe("/mnt/a");
    expect(v.normalize("a/b")).toBe("/mnt/a/b");
    expect(v.normalize("./a")).toBe("/mnt/a");
    expect(v.normalize(".")).toBe("/mnt");
  });
  it("drops ., pops .., and keeps / at the root", () => {
    const v = new Vfs();
    expect(v.normalize("..")).toBe("/");
    expect(v.normalize("../..")).toBe("/");
    expect(v.normalize("/..")).toBe("/");
    expect(v.normalize("/mnt/a/../b")).toBe("/mnt/b");
    v.cwd = "/mnt/DOCS";
    expect(v.normalize("..")).toBe("/mnt");
    expect(v.normalize("../X")).toBe("/mnt/X");
  });
  it("treats \\ as /, collapses separators, and strips a trailing /", () => {
    const v = new Vfs();
    expect(v.normalize("/mnt//a/./b/")).toBe("/mnt/a/b");
    expect(v.normalize("\\mnt\\a")).toBe("/mnt/a");
    expect(v.normalize("/")).toBe("/");
    expect(v.normalize("///")).toBe("/");
    expect(v.normalize("/mnt/")).toBe("/mnt");
  });
  it("preserves case as typed", () => {
    expect(new Vfs().normalize("/mnt/Hello World.txt")).toBe("/mnt/Hello World.txt");
  });
});

describe("Vfs.resolve", () => {
  const v = new Vfs();
  it("classifies the virtual root, /dev, the devices, and volume paths", () => {
    expect(v.resolve("/")).toEqual({ kind: "root" });
    expect(v.resolve("/dev")).toEqual({ kind: "dev" });
    expect(v.resolve("/dev/")).toEqual({ kind: "dev" });
    expect(v.resolve("/dev/hda")).toEqual({ kind: "raw" });
    expect(v.resolve("/dev/zero")).toEqual({ kind: "zero" });
    expect(v.resolve("/dev/null")).toEqual({ kind: "null" });
    expect(v.resolve("/mnt")).toEqual({ kind: "volume", path: "/" });
    expect(v.resolve("/mnt/")).toEqual({ kind: "volume", path: "/" });
    expect(v.resolve("/mnt/A/B")).toEqual({ kind: "volume", path: "/A/B" });
    expect(v.resolve("A/B")).toEqual({ kind: "volume", path: "/A/B" });
    expect(v.resolve("..")).toEqual({ kind: "root" });
  });
  it("rejects unknown devices and paths outside /mnt and /dev, with help", () => {
    const dev = thrown(() => v.resolve("/dev/hdb"));
    expect(dev).toBeInstanceOf(ShellError);
    expect(dev.message).toBe("no such device: /dev/hdb");
    expect(dev.help).toBe(DEVICE_HELP);
    const foo = thrown(() => v.resolve("/foo"));
    expect(foo.message).toBe("no such file or directory: /foo");
    expect(foo.help).toBe(PATH_HELP);
    expect(thrown(() => v.resolve("/mnt2/x")).message).toBe("no such file or directory: /mnt2/x");
    expect(thrown(() => v.resolve("/dev/hda/x")).message).toBe("no such device: /dev/hda/x");
    expect(thrown(() => v.resolve("/MNT/x")).message).toBe("no such file or directory: /MNT/x");
  });
  it("display and toVirtual invert resolve", () => {
    for (const p of ["/", "/dev", "/dev/hda", "/dev/zero", "/dev/null", "/mnt", "/mnt/A/B"]) expect(v.display(v.resolve(p))).toBe(p);
    expect(v.toVirtual("/")).toBe("/mnt");
    expect(v.toVirtual("/A/B")).toBe("/mnt/A/B");
  });
});

describe("canonicalize over a real volume", () => {
  it("fixes the case of every component to the on-disk name", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createFile("/Hello world.txt", new TextEncoder().encode("hi"));
    vol.createDir("/DOCS");
    vol.createFile("/DOCS/N.TXT", new Uint8Array(3));
    expect(canonicalize(vol, "/hello world.txt")).toBe("/Hello world.txt");
    expect(canonicalize(vol, "/HELLO WORLD.TXT")).toBe("/Hello world.txt");
    expect(canonicalize(vol, "/docs/n.txt")).toBe("/DOCS/N.TXT");
    expect(canonicalize(vol, "/docs")).toBe("/DOCS");
    expect(canonicalize(vol, "/")).toBe("/");
  });
  it("keeps the typed spelling for components it cannot find or read", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createDir("/DOCS");
    expect(canonicalize(vol, "/docs/missing.txt")).toBe("/DOCS/missing.txt");
    expect(canonicalize(vol, "/nope/deeper")).toBe("/nope/deeper");
  });
});

describe("path helpers", () => {
  it("basename and joinVolume", () => {
    expect(basename("/A/B.TXT")).toBe("B.TXT");
    expect(basename("B.TXT")).toBe("B.TXT");
    expect(basename("/")).toBe("");
    expect(joinVolume("/", "A")).toBe("/A");
    expect(joinVolume("/D", "A")).toBe("/D/A");
  });
});
```

Run `pnpm test tests/shell/vfs.test.ts`. Expected failure: `Failed to resolve import "../../src/shell/vfs"`.

- [ ] **Step 10: Create vfs.ts**

Create `web/ui/src/shell/vfs.ts`:

```ts
import type { Volume } from "../lib/wasm";
import { ShellError } from "./errors";

/** Where a virtual path lands. `volume.path` is the path fs-core sees ("/" for /mnt itself). */
export type Resolved =
  | { kind: "root" }
  | { kind: "dev" }
  | { kind: "raw" }
  | { kind: "zero" }
  | { kind: "null" }
  | { kind: "volume"; path: string };

export const MOUNT = "/mnt";
const DEV = "/dev";
export const PATH_HELP = "the volume is mounted at /mnt; the raw disk is /dev/hda (with /dev/zero and /dev/null beside it)";
export const DEVICE_HELP = "devices: /dev/hda (the whole disk), /dev/zero, /dev/null";

/**
 * The virtual tree the shell shows: `/` holds `dev` and `mnt`. Normalization is client-side
 * because fs-core rejects `.` and `..`. One cwd per page: commands cannot learn their session
 * from browser-terminal's ctx, and there is one terminal instance anyway.
 */
export class Vfs {
  cwd = MOUNT;

  /** Absolute virtual path: `\` as `/`, separators collapsed, joined to cwd, `.` dropped, `..` popped (root stays root), no trailing `/`. */
  normalize(input: string): string {
    const slashed = input.replace(/\\/g, "/");
    const joined = slashed.startsWith("/") ? slashed : `${this.cwd}/${slashed}`;
    const parts: string[] = [];
    for (const part of joined.split("/")) {
      if (part === "" || part === ".") continue;
      if (part === "..") { parts.pop(); continue; }
      parts.push(part);
    }
    return "/" + parts.join("/");
  }

  resolve(input: string): Resolved {
    const p = this.normalize(input);
    if (p === "/") return { kind: "root" };
    if (p === DEV) return { kind: "dev" };
    if (p === `${DEV}/hda`) return { kind: "raw" };
    if (p === `${DEV}/zero`) return { kind: "zero" };
    if (p === `${DEV}/null`) return { kind: "null" };
    if (p === MOUNT) return { kind: "volume", path: "/" };
    if (p.startsWith(`${MOUNT}/`)) return { kind: "volume", path: p.slice(MOUNT.length) };
    if (p.startsWith(`${DEV}/`)) throw new ShellError(`no such device: ${p}`, { help: DEVICE_HELP });
    throw new ShellError(`no such file or directory: ${p}`, { help: PATH_HELP });
  }

  display(r: Resolved): string {
    switch (r.kind) {
      case "root": return "/";
      case "dev": return DEV;
      case "raw": return `${DEV}/hda`;
      case "zero": return `${DEV}/zero`;
      case "null": return `${DEV}/null`;
      case "volume": return this.toVirtual(r.path);
    }
  }

  toVirtual(volumePath: string): string {
    return volumePath === "/" ? MOUNT : `${MOUNT}${volumePath}`;
  }
}

const namesMatch = (a: string, b: string): boolean => a.toUpperCase() === b.toUpperCase();

/**
 * Re-spell each component of a volume path the way the directory stores it (FAT lookups are
 * case-insensitive, so `/docs/n.txt` is `/DOCS/N.TXT` on disk). A component that is missing,
 * or whose parent cannot be listed (a file in the middle, a corrupt volume), keeps the typed
 * spelling; callers validate existence with `stat` separately.
 */
export function canonicalize(vol: Volume, volumePath: string): string {
  const out: string[] = [];
  let dir = "/";
  for (const part of volumePath.split("/")) {
    if (part === "") continue;
    let name = part;
    try {
      const hit = vol.listDir(dir).find((e) => namesMatch(e.name, part));
      if (hit) name = hit.name;
    } catch {
      // unreadable parent: keep the typed spelling for this and the remaining components
    }
    out.push(name);
    dir = joinVolume(dir, name);
  }
  return "/" + out.join("/");
}

export function basename(p: string): string {
  const i = p.lastIndexOf("/");
  return i < 0 ? p : p.slice(i + 1);
}

export function joinVolume(dir: string, name: string): string {
  return dir === "/" ? `/${name}` : `${dir}/${name}`;
}
```

Run `pnpm test tests/shell/vfs.test.ts`: 10 tests pass. Commit:

```sh
git add web/ui/src/shell/vfs.ts web/ui/tests/shell/vfs.test.ts
git commit -m "feat(ui): virtual filesystem with /mnt, /dev, cwd normalization, and canonical names"
```

- [ ] **Step 11: Write the failing test for xxd.ts**

Create `web/ui/tests/shell/xxd.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { ShellError } from "../../src/shell/errors";
import { formatXxd } from "../../src/shell/xxd";

const text = (s: string) => new TextEncoder().encode(s);

describe("formatXxd", () => {
  it("prints the classic xxd layout: 8-digit address, byte pairs, padded hex area, ASCII", () => {
    const bytes = new Uint8Array([...text("Hello, FAT16!"), 0, 0, 0, 0, 0]); // 18 bytes: two rows
    expect(formatXxd(bytes, 0)).toBe(
      "00000000: 4865 6c6c 6f2c 2046 4154 3136 2100 0000  Hello, FAT16!...\n" +
      "00000010: 0000                                     ..",
    );
  });
  it("pads a single short row so the ASCII column stays aligned", () => {
    expect(formatXxd(text("Hello, FAT16!"), 0)).toBe("00000000: 4865 6c6c 6f2c 2046 4154 3136 21         Hello, FAT16!");
  });
  it("uses the base as the absolute address and honours cols", () => {
    const bytes = Uint8Array.from({ length: 20 }, (_, i) => i);
    expect(formatXxd(bytes, 0x8200, 8)).toBe(
      "00008200: 0001 0203 0405 0607  ........\n" +
      "00008208: 0809 0a0b 0c0d 0e0f  ........\n" +
      "00008210: 1011 1213            ....",
    );
    expect(formatXxd(text("abc"), 0, 3)).toBe("00000000: 6162 63  abc");
  });
  it("shows non-printables as . and empty input as an empty string", () => {
    expect(formatXxd(new Uint8Array([0x1b, 0x7f, 0x20, 0x7e, 0xff]), 0)).toBe("00000000: 1b7f 207e ff                             .. ~.");
    expect(formatXxd(new Uint8Array(0), 0)).toBe("");
  });
  it("widens the address past 8 digits when needed and rejects bad cols", () => {
    expect(formatXxd(new Uint8Array([1]), 0x1_0000_0000)).toBe("100000000: 01                                       .");
    for (const cols of [0, 65, 1.5, -1]) {
      let caught: unknown;
      try { formatXxd(new Uint8Array([1]), 0, cols); } catch (e) { caught = e; }
      expect(caught).toBeInstanceOf(ShellError);
      expect((caught as ShellError).message).toBe("--cols must be 1..64");
    }
  });
});
```

Run `pnpm test tests/shell/xxd.test.ts`. Expected failure: `Failed to resolve import "../../src/shell/xxd"`.

- [ ] **Step 12: Create xxd.ts**

Create `web/ui/src/shell/xxd.ts`:

```ts
import { ShellError } from "./errors";

/**
 * Classic `xxd` layout, lines joined by "\n" with no trailing newline:
 *
 *   00000000: 4865 6c6c 6f2c 2046 4154 3136 2100 0000  Hello, FAT16!...
 *   00000010: 0000                                     ..
 *
 * Address = `base + row offset` as at least 8 lowercase hex digits; bytes in pairs with one
 * space between pairs; the hex area is padded to a full row so the ASCII column aligns; two
 * spaces; ASCII with `.` for anything outside 0x20..0x7e (the renderer would strip control
 * bytes anyway). `cols` is the bytes per row, 1..64.
 */
export function formatXxd(bytes: Uint8Array, base: number, cols = 16): string {
  if (!Number.isInteger(cols) || cols < 1 || cols > 64) throw new ShellError("--cols must be 1..64");
  const width = cols * 2 + Math.ceil(cols / 2) - 1;
  const lines: string[] = [];
  for (let i = 0; i < bytes.length; i += cols) {
    const row = bytes.subarray(i, i + cols);
    const pairs: string[] = [];
    for (let j = 0; j < row.length; j += 2) {
      let pair = row[j].toString(16).padStart(2, "0");
      if (j + 1 < row.length) pair += row[j + 1].toString(16).padStart(2, "0");
      pairs.push(pair);
    }
    let ascii = "";
    for (let j = 0; j < row.length; j++) ascii += row[j] >= 0x20 && row[j] <= 0x7e ? String.fromCharCode(row[j]) : ".";
    lines.push(`${(base + i).toString(16).padStart(8, "0")}: ${pairs.join(" ").padEnd(width)}  ${ascii}`);
  }
  return lines.join("\n");
}
```

Run `pnpm test tests/shell/xxd.test.ts`: 5 tests pass. Commit:

```sh
git add web/ui/src/shell/xxd.ts web/ui/tests/shell/xxd.test.ts
git commit -m "feat(ui): xxd formatter for the shell"
```

- [ ] **Step 13: Write the failing test for dd.ts**

Create `web/ui/tests/shell/dd.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { SIZE_HELP } from "../../src/shell/addr";
import { DD_MAX_BYTES, OPERAND_HELP, formatRecords, parseDd, planWindow } from "../../src/shell/dd";
import { ShellError } from "../../src/shell/errors";

function thrown(fn: () => unknown): ShellError {
  try { fn(); } catch (e) { return e as ShellError; }
  throw new Error("expected a throw");
}

describe("parseDd", () => {
  it("defaults: bs 512, skip 0, seek 0, no if/of/count", () => {
    expect(parseDd({}, [])).toEqual({ bs: 512, skip: 0, seek: 0 });
  });
  it("reads --if/--of/--bs/--count/--skip/--seek flags (strings from the 'str' shape, or ints)", () => {
    expect(parseDd({ if: "/dev/hda", of: "/mnt/boot.bin", bs: "512", count: "1", skip: "65", seek: 2 }, [])).toEqual({
      if: "/dev/hda", of: "/mnt/boot.bin", bs: 512, count: 1, skip: 65, seek: 2,
    });
    expect(parseDd({ bs: "1k", count: "2" }, [])).toEqual({ bs: 1024, count: 2, skip: 0, seek: 0 });
  });
  it("accepts quoted classic key=value operands and merges them with flags", () => {
    expect(parseDd({}, ["if=/dev/zero", "of=/dev/hda", "bs=512", "seek=0", "count=1"])).toEqual({
      if: "/dev/zero", of: "/dev/hda", bs: 512, count: 1, skip: 0, seek: 0,
    });
    expect(parseDd({ if: "/dev/hda" }, ["count=1"])).toEqual({ if: "/dev/hda", bs: 512, count: 1, skip: 0, seek: 0 });
  });
  it("rejects a key given twice, across flags and operands", () => {
    const e = thrown(() => parseDd({ if: "/dev/hda" }, ["if=/dev/zero"]));
    expect(e).toBeInstanceOf(ShellError);
    expect(e.message).toBe("'if' given twice");
    expect(thrown(() => parseDd({}, ["bs=1", "bs=2"])).message).toBe("'bs' given twice");
  });
  it("rejects unknown or malformed operands with help", () => {
    for (const op of ["foo", "conv=sync", "if", "=x"]) {
      const e = thrown(() => parseDd({}, [op]));
      expect(e.message).toBe(`unrecognized operand '${op}'`);
      expect(e.help).toBe(OPERAND_HELP);
    }
    expect(thrown(() => parseDd({}, [7])).message).toBe("unrecognized operand '7'");
  });
  it("validates sizes and paths", () => {
    const bs = thrown(() => parseDd({ bs: "x" }, []));
    expect(bs.message).toBe("invalid block size 'x'");
    expect(bs.help).toBe(SIZE_HELP);
    expect(thrown(() => parseDd({ bs: "0" }, [])).message).toBe("invalid block size '0'");
    expect(thrown(() => parseDd({}, ["count=-1"])).message).toBe("invalid count '-1'");
    expect(thrown(() => parseDd({ skip: "1.5" }, [])).message).toBe("invalid skip '1.5'");
    expect(thrown(() => parseDd({ seek: "abc" }, [])).message).toBe("invalid seek 'abc'");
    expect(thrown(() => parseDd({ if: "" }, [])).message).toBe("'if' needs a path");
    expect(thrown(() => parseDd({}, ["of="])).message).toBe("'of' needs a path");
    expect(thrown(() => parseDd({ if: null }, [])).message).toBe("'if' needs a path");
  });
});

describe("planWindow and the 1 MiB cap", () => {
  it("DD_MAX_BYTES is 1 MiB", () => {
    expect(DD_MAX_BYTES).toBe(1048576);
  });
  it("computes skip*bs and count*bs clipped to what is available", () => {
    expect(planWindow({ bs: 512, count: 1, skip: 65 }, 16 * 1048576)).toEqual({ start: 65 * 512, len: 512 });
    expect(planWindow({ bs: 512, count: undefined, skip: 0 }, 1000)).toEqual({ start: 0, len: 1000 });
    expect(planWindow({ bs: 512, count: 4, skip: 1 }, 1000)).toEqual({ start: 512, len: 488 });
    expect(planWindow({ bs: 512, count: 1, skip: 10 }, 1000)).toEqual({ start: 5120, len: 0 }); // skip past the end
    expect(planWindow({ bs: 512, count: 3, skip: 0 }, Infinity)).toEqual({ start: 0, len: 1536 }); // /dev/zero
  });
  it("refuses a read window over DD_MAX_BYTES before anything is written", () => {
    expect(planWindow({ bs: 1048576, count: 1, skip: 0 }, Infinity).len).toBe(DD_MAX_BYTES);
    const e = thrown(() => planWindow({ bs: 512, count: 2049, skip: 0 }, Infinity));
    expect(e).toBeInstanceOf(ShellError);
    expect(e.message).toBe("refusing to copy 1049088 bytes in one dd; the limit is 1048576 (1 MiB)");
    expect(e.help).toBe("lower --count or --bs, or copy in several runs with --skip and --seek");
    expect(thrown(() => planWindow({ bs: 512, count: undefined, skip: 0 }, 16 * 1048576)).message).toMatch(/^refusing to copy 16777216 bytes/);
  });
});

describe("formatRecords", () => {
  it("counts full and partial blocks like dd", () => {
    expect(formatRecords(512, 512)).toBe("1+0 records");
    expect(formatRecords(1000, 512)).toBe("1+1 records");
    expect(formatRecords(0, 512)).toBe("0+0 records");
    expect(formatRecords(3, 512)).toBe("0+1 records");
  });
});
```

Run `pnpm test tests/shell/dd.test.ts`. Expected failure: `Failed to resolve import "../../src/shell/dd"`.

- [ ] **Step 14: Create dd.ts**

Create `web/ui/src/shell/dd.ts`:

```ts
import type { Value } from "./types";
import { ShellError } from "./errors";
import { SIZE_HELP, parseSize } from "./addr";

/**
 * Per-invocation cap on the bytes `dd` reads (and therefore writes). Every journaled byte is
 * stored twice in Rust (before/after) and again in two histories, and a blob is 2x as hex,
 * so the shell caps here rather than in the core. `cat` without `--bytes` uses the same limit.
 */
export const DD_MAX_BYTES = 1 << 20;

export const DD_KEYS = ["if", "of", "bs", "count", "skip", "seek"] as const;
export type DdKey = (typeof DD_KEYS)[number];

export interface DdOpts {
  if?: string;
  of?: string;
  bs: number;
  count?: number;
  skip: number;
  seek: number;
}

export const OPERAND_HELP =
  "operands: if= of= bs= count= skip= seek= (quote them, e.g. 'if=/dev/hda', because = is reserved by the shell; or write --if=/dev/hda)";

const isDdKey = (k: string): k is DdKey => (DD_KEYS as readonly string[]).includes(k);

/**
 * Merge `--if/--of/--bs/--count/--skip/--seek` flags (the documented form is `--if=/dev/hda`)
 * with quoted classic `'if=/dev/hda'` operands. The same key from both sources, or twice
 * among the operands, is an error; so is any operand that is not `key=value` with a known key.
 * `bs` defaults to 512 and must be positive; `count`, `skip`, `seek` take sizes too (so
 * `--count=1k` works) and must be non-negative.
 */
export function parseDd(flags: Record<string, Value>, operands: Value[]): DdOpts {
  const given = new Map<DdKey, Value>();
  const put = (key: DdKey, value: Value) => {
    if (given.has(key)) throw new ShellError(`'${key}' given twice`, { help: "give each of if/of/bs/count/skip/seek once, as a flag or a quoted operand" });
    given.set(key, value);
  };
  for (const key of DD_KEYS) if (key in flags) put(key, flags[key]);
  for (const op of operands) {
    const eq = typeof op === "string" ? op.indexOf("=") : -1;
    const key = typeof op === "string" && eq > 0 ? op.slice(0, eq) : "";
    if (typeof op !== "string" || !isDdKey(key)) throw new ShellError(`unrecognized operand '${String(op)}'`, { help: OPERAND_HELP });
    put(key, op.slice(eq + 1));
  }

  const path = (key: "if" | "of"): string | undefined => {
    if (!given.has(key)) return undefined;
    const v = given.get(key);
    if (typeof v !== "string" || v === "") throw new ShellError(`'${key}' needs a path`, { help: OPERAND_HELP });
    return v;
  };
  const size = (key: "bs" | "count" | "skip" | "seek", what: string): number | undefined => {
    if (!given.has(key)) return undefined;
    const v = given.get(key);
    const bad = new ShellError(`invalid ${what} '${String(v)}'`, { help: SIZE_HELP });
    if (typeof v !== "string" && typeof v !== "number") throw bad;
    try { return parseSize(v); } catch { throw bad; }
  };

  const bs = size("bs", "block size") ?? 512;
  if (bs === 0) throw new ShellError("invalid block size '0'", { help: SIZE_HELP });
  const opts: DdOpts = { bs, skip: size("skip", "skip") ?? 0, seek: size("seek", "seek") ?? 0 };
  const src = path("if"), dst = path("of"), count = size("count", "count");
  if (src !== undefined) opts.if = src;
  if (dst !== undefined) opts.of = dst;
  if (count !== undefined) opts.count = count;
  return opts;
}

export interface DdWindow {
  start: number;
  len: number;
}

/**
 * The read window: `skip*bs` for `count*bs` bytes, or to the end when `count` is absent,
 * clipped to `available` (the source length; `Infinity` for /dev/zero, whose callers require
 * `count`). Throws when the window exceeds DD_MAX_BYTES, before any write happens.
 */
export function planWindow(opts: Pick<DdOpts, "bs" | "count" | "skip">, available: number): DdWindow {
  const start = opts.skip * opts.bs;
  const avail = Math.max(0, available - start);
  const len = opts.count === undefined ? avail : Math.min(opts.count * opts.bs, avail);
  if (len > DD_MAX_BYTES) {
    throw new ShellError(`refusing to copy ${len} bytes in one dd; the limit is ${DD_MAX_BYTES} (1 MiB)`, {
      help: "lower --count or --bs, or copy in several runs with --skip and --seek",
    });
  }
  return { start, len };
}

/** dd's `N+P records` count: N full blocks and P (0 or 1) partial block. */
export function formatRecords(len: number, bs: number): string {
  return `${Math.floor(len / bs)}+${len % bs === 0 ? 0 : 1} records`;
}
```

Run `pnpm test tests/shell/dd.test.ts`: 10 tests pass. Commit:

```sh
git add web/ui/src/shell/dd.ts web/ui/tests/shell/dd.test.ts
git commit -m "feat(ui): dd operand parsing, read-window planning, and the 1 MiB cap"
```

- [ ] **Step 15: Run the task gate**

From `web/ui`:

```sh
pnpm install --frozen-lockfile
pnpm test
pnpm build
```

Expected: vitest reports 18 files passed (the 12 existing plus `tests/shell/{errors,bytes,addr,vfs,xxd,dd}.test.ts`), 94 tests passed (49 existing + 7 + 9 + 4 + 10 + 5 + 10); `svelte-check` reports 0 errors and 0 warnings; `vite build` writes `dist/`. Also confirm the runtime rule with `grep -rn "browser-terminal" src/shell/`: every hit is an `import type` or `export type` line in `src/shell/types.ts` only. `git status` is clean apart from nothing (every change is committed above). No Rust files changed, so the Rust gates (`cargo fmt`, `clippy`) are untouched by this task.


---

### Task 5: Shell read commands and test harness

**Files:**
- Create `web/ui/src/shell/commands.ts` — `createCommands(host, vfs = new Vfs())`, the read commands (`ls`, `dir`, `cd`, `pwd`, `cat`, `stat`, `df`, `mount`, `seek`, `select`, `exit`), the rewound warning, and the helpers Task 6 reuses.
- Create `web/ui/tests/shell/helpers.ts` — `TestHost`/`makeHost` over a plain `Volume`, `call`, `callErr`.
- Create `web/ui/tests/shell/read-commands.test.ts`.
- No existing file is modified.

**Prerequisites (verify before step 1):**
- Task 2 merged and `wasm-pack build crates/wasm --target bundler` re-run, so `crates/wasm/pkg/fs_emulator_wasm.d.ts` declares `corruption(): string | null`, `writeRaw(offset: number, bytes: Uint8Array): OpRecord`, `readRaw(offset: number, len: number): Uint8Array`. The UI consumes the package as `file:../../crates/wasm/pkg`; pnpm copies `file:` packages at install time, so after rebuilding run `cd web/ui && pnpm install` and confirm `grep corruption node_modules/fs-emulator-wasm/fs_emulator_wasm.d.ts` prints a line.
- Task 4 merged: `web/ui/src/shell/{errors,host,bytes,addr,vfs,dd}.ts` exist and `@benjamin-small/browser-terminal@0.2.0` is in `web/ui/package.json` (type-only imports resolve).

**Interfaces:**

Consumes (exact names; from Task 4 unless noted):
- `web/ui/src/shell/host.ts`: `interface ShellHost { readonly vol: Volume; readonly cursor: number; readonly historyLength: number; run(fn: (v: Volume) => OpRecord): OpRecord; format(options: FormatOptions): void; select(path: string | null): void; jumpTo(offset: number): void; closeTerminal(): void }` and `atLatest(h: ShellHost): boolean`.
- `web/ui/src/shell/errors.ts`: `class ShellError extends Error { help?: string; code?: string; constructor(message: string, opts?: { help?: string; code?: string }) }` and `wrapFs(display: string, e: unknown): ShellError` (message `${display}: ${phrase}`, keeps `code`).
- `web/ui/src/shell/bytes.ts`: `fromBytes(b: Uint8Array): BytesBlob`, `decodeText(b: Uint8Array): string`.
- `web/ui/src/shell/addr.ts`: `parseAddr(v: string | number, g: Geometry): number` (throws `ShellError` with `help: ADDR_HELP`), `ADDR_HELP: string`.
- `web/ui/src/shell/vfs.ts`: `class Vfs { cwd: string; resolve(input: string): Resolved; display(r: Resolved): string; toVirtual(volumePath: string): string }`, `type Resolved = { kind: "root" } | { kind: "dev" } | { kind: "raw" } | { kind: "zero" } | { kind: "null" } | { kind: "volume"; path: string }`, `canonicalize(vol: Volume, volumePath: string): string`.
- `web/ui/src/shell/dd.ts`: `DD_MAX_BYTES` (`1 << 20`).
- Task 2 (`fs-emulator-wasm`): `Volume.corruption(): string | null`; tests also use `Volume.writeRaw(offset, bytes): OpRecord` and the existing `Volume.sector(n): Uint8Array`.
- Existing `web/ui/src/core`: `clusterByteRange(g: Geometry, cluster: number): { start: number; end: number }` (`attribution.ts`), `findEntrySlots(vol: Volume, g: Geometry, fat: FatEntry[], owners: ClusterOwner[], path: string): { start: number; end: number } | null` (`direntry.ts`), `buildChain(fat: FatEntry[], first: number): number[]` (`fatchain.ts`).
- `@benjamin-small/browser-terminal` (types only): `CommandSpec`, `CommandArgs`, `CommandCtx`, `PosArg`, `FlagSpec`, `Value`. Facts that bind the code: a switch flag that is absent is absent from `args.flags` (not `false`); flags are keyed by their `long` name even when given as `-l`; shape `"str"` coerces ints/bools to strings; a missing optional positional is absent from `positionals`; the engine prefixes a thrown `message` with the command name, so no message below starts with a command name; a thrown object's `help` renders under the message.

Produces (Task 6 and Task 7 rely on these):
- `web/ui/src/shell/commands.ts`:
  - `export type { CommandDef } from "./types";` (re-exported from Task 4's `types.ts`; not declared here)
  - `export function createCommands(host: ShellHost, vfs?: Vfs): CommandDef[]` (Task 6 appends `...writeCommands(host, vfs)` to the array literal inside it)
  - `export function readCommands(host: ShellHost, vfs: Vfs): CommandDef[]`
  - `export function rewoundWarning(host: ShellHost): string`
  - `export function warnIfRewound(host: ShellHost, ctx: CommandCtx): void`
  - `export function assertMounted(host: ShellHost, display: string): void` (throws `ShellError` code `CorruptImage`)
  - `export function posStr(args: CommandArgs, i: number): string | undefined`
  - `export function reqStr(args: CommandArgs, i: number, name: string): string`
  - `export function flagOn(args: CommandArgs, name: string): boolean`
  - `export function fmtDate(d: DateTime | null): string`
  - `export function hexAddr(n: number): string`
  - `export function looksBinary(bytes: Uint8Array): boolean`
  - `export const P: (name: string, desc: string) => PosArg`, `export const F: (long: string, desc: string, extra?: Partial<FlagSpec>) => FlagSpec`
  - `export const RAW_DEVICE_HELP: string`, `export const CORRUPT_HELP: string`
- `web/ui/tests/shell/helpers.ts`:
  - `export class TestHost implements ShellHost { vol: Volume; history: OpRecord[]; cursor: number; selected: (string | null)[]; jumps: number[]; formats: FormatOptions[]; closed: boolean; get historyLength(): number; run(fn): OpRecord; format(options): void; select(path): void; jumpTo(offset): void; closeTerminal(): void; rewind(step: number): void }`
  - `export function makeHost(vol?: Volume): TestHost`
  - `export type CallArgs = Value[] | { positionals?: Value[]; flags?: Record<string, Value> }`
  - `export interface CallResult { value: unknown; log: string[]; err: string[] }`
  - `export function call(defs: CommandDef[], name: string, args?: CallArgs, input?: Value): Promise<CallResult>`
  - `export interface ShellFailure { message: string; help?: string; code?: string }`
  - `export function callErr(defs: CommandDef[], name: string, args?: CallArgs, input?: Value): Promise<ShellFailure>`

Exact user-facing strings this task fixes (Task 6 and the README quote them):
- Rewound warning (via `ctx.err`, once per read command when `!atLatest(host)`): `showing the latest state, not step ${cursor + 1} of ${historyLength}; click "Back to now" or run a write command`
- Corruption (thrown by `ls`, `df`, `stat` on volume paths): `${display}: ${vol.corruption()}` with code `CorruptImage` and help `CORRUPT_HELP` = `the boot sector no longer parses; rewind on the timeline, or write the saved sector back with: <blob> | dd --of=/dev/hda`
- `cat` on `/dev/hda` or `/dev/zero`: `${display}: is a raw device`, help `RAW_DEVICE_HELP` = `read a range with: dd --if=/dev/hda --bs=512 --skip=0 --count=1 | xxd`
- `cat` oversize: `${display}: file is ${n} bytes; cat prints at most ${DD_MAX_BYTES} bytes`, help `read part of it with: dd --if=${display} --bs=512 --count=1 | xxd`
- `cat` binary warning (via `ctx.err`): `binary file; try cat --bytes ${display} | xxd`
- `cd` to a non-directory: `${display}: Not a directory` (code `NotADirectory`)
- `seek` past the end: `${hexAddr(off)} is past the end of the disk (${limit} bytes)`
- `select` on a non-volume path: `${display}: only volume paths can be selected`, help `select /mnt/<file>, or run select with no argument to clear`

---

- [ ] **Step 1: Write the test harness `web/ui/tests/shell/helpers.ts`**

This is test support, not a test; it is type-checked by `svelte-check` (the tsconfig includes `tests`). It imports `CommandDef` from the module Step 3 creates, so nothing compiles until then — that is the intended red state.

```ts
import { Volume, type FormatOptions, type OpRecord } from "../../src/lib/wasm";
import type { ShellHost } from "../../src/shell/host";
import type { CommandDef } from "../../src/shell/commands";
import type { CommandCtx, Value } from "@benjamin-small/browser-terminal";

/**
 * A ShellHost over a plain Volume. `run` mirrors VolumeStore.run: it snaps
 * back to the latest step, applies the op, appends the record, and leaves the
 * cursor at the end. `rewind` only moves the cursor (the volume itself always
 * holds the latest state, exactly as in the app).
 */
export class TestHost implements ShellHost {
  vol: Volume;
  history: OpRecord[] = [];
  cursor = -1;
  selected: (string | null)[] = [];
  jumps: number[] = [];
  formats: FormatOptions[] = [];
  closed = false;

  constructor(vol: Volume) {
    this.vol = vol;
  }

  get historyLength(): number {
    return this.history.length;
  }

  run(fn: (v: Volume) => OpRecord): OpRecord {
    this.cursor = this.history.length - 1;
    const rec = fn(this.vol); // a wasm FsError ({ message, code }) propagates as-is
    this.history.push(rec);
    this.cursor = this.history.length - 1;
    return rec;
  }

  format(options: FormatOptions): void {
    this.vol = Volume.formatFat16(options);
    this.history = [];
    this.cursor = -1;
    this.formats.push(options);
  }

  select(path: string | null): void {
    this.selected.push(path);
  }

  jumpTo(offset: number): void {
    this.jumps.push(offset);
  }

  closeTerminal(): void {
    this.closed = true;
  }

  /** View step `step` (-1 = before any op) without touching the volume. */
  rewind(step: number): void {
    this.cursor = Math.max(-1, Math.min(step, this.history.length - 1));
  }
}

export function makeHost(vol: Volume = Volume.formatFat16(undefined)): TestHost {
  return new TestHost(vol);
}

export type CallArgs = Value[] | { positionals?: Value[]; flags?: Record<string, Value> };
export interface CallResult { value: unknown; log: string[]; err: string[] }
export interface ShellFailure { message: string; help?: string; code?: string }

/** A ChannelWriter whose cooked calls and raw writes both land in `sink`. */
function writer(sink: string[]): CommandCtx["log"] {
  return Object.assign(
    (line: string) => { sink.push(line); },
    {
      write(s: string) { sink.push(s); },
      flush() {},
      mode() {},
    },
  );
}

function bindArgs(args: CallArgs): { positionals: Value[]; flags: Record<string, Value> } {
  if (Array.isArray(args)) return { positionals: args, flags: {} };
  return { positionals: args.positionals ?? [], flags: args.flags ?? {} };
}

/** Run one registered command the way the engine would: bound args, piped input, a fake ctx. */
export async function call(defs: CommandDef[], name: string, args: CallArgs = [], input: Value = null): Promise<CallResult> {
  const def = defs.find((d) => d.spec.name === name);
  if (!def) throw new Error(`no command named "${name}" is registered`);
  const log: string[] = [];
  const err: string[] = [];
  const controller = new AbortController();
  const ctx: CommandCtx = { signal: controller.signal, log: writer(log), err: writer(err), emit: writer(log) };
  const value = await def.fn(bindArgs(args), input, ctx);
  return { value, log, err };
}

/** Like `call`, but the command must throw; returns what the engine would render. */
export async function callErr(defs: CommandDef[], name: string, args: CallArgs = [], input: Value = null): Promise<ShellFailure> {
  try {
    await call(defs, name, args, input);
  } catch (e) {
    const f = e as Partial<ShellFailure>;
    return { message: String(f.message ?? e), help: f.help, code: f.code };
  }
  throw new Error(`${name} did not throw`);
}
```

- [ ] **Step 2: Write the failing tests `web/ui/tests/shell/read-commands.test.ts`**

```ts
import { describe, expect, it } from "vitest";
import { clusterByteRange } from "../../src/core/attribution";
import { findEntrySlots } from "../../src/core/direntry";
import { ADDR_HELP } from "../../src/shell/addr";
import { fromBytes } from "../../src/shell/bytes";
import { createCommands, rewoundWarning } from "../../src/shell/commands";
import { DD_MAX_BYTES } from "../../src/shell/dd";
import { Vfs } from "../../src/shell/vfs";
import { call, callErr, makeHost } from "./helpers";

const enc = (s: string) => new TextEncoder().encode(s);
const DISK_BYTES = 32768 * 512; // the default 16 MB volume

/** A host with three steps: a directory, a two-cluster file in it, and a long-named root file. */
function setup() {
  const host = makeHost();
  host.run((v) => v.createDir("/DOCS"));
  host.run((v) => v.createFile("/DOCS/N.TXT", new Uint8Array(3000)));
  host.run((v) => v.createFile("/Hello world.txt", enc("hello from the shell\n")));
  const vfs = new Vfs();
  return { host, vfs, defs: createCommands(host, vfs) };
}

describe("ls and dir", () => {
  it("lists the virtual root and /dev with the disk size on hda", async () => {
    const { defs } = setup();
    expect((await call(defs, "ls", ["/"])).value).toEqual([
      { name: "dev", type: "dir", size: 0 },
      { name: "mnt", type: "dir", size: 0 },
    ]);
    expect((await call(defs, "ls", ["/dev"])).value).toEqual([
      { name: "hda", type: "block", size: DISK_BYTES },
      { name: "zero", type: "char", size: 0 },
      { name: "null", type: "char", size: 0 },
    ]);
    expect((await call(defs, "ls", ["/dev/hda"])).value).toEqual([{ name: "hda", type: "block", size: DISK_BYTES }]);
  });

  it("lists a volume directory in on-disk order and a file as one row", async () => {
    const { defs } = setup();
    expect((await call(defs, "ls", ["/mnt"])).value).toEqual([
      { name: "DOCS", type: "dir", size: 0 },
      { name: "Hello world.txt", type: "file", size: 21 },
    ]);
    expect((await call(defs, "ls", ["/mnt/docs/n.txt"])).value).toEqual([{ name: "N.TXT", type: "file", size: 3000 }]);
    expect((await call(defs, "ls", ["/mnt/DOCS"])).value).toEqual([{ name: "N.TXT", type: "file", size: 3000 }]);
  });

  it("defaults to the working directory and honours -l", async () => {
    const { host, defs } = setup();
    host.vol.setNow({ year: 2026, month: 9, day: 22, hour: 10, minute: 30, second: 0 });
    host.run((v) => v.createFile("/DOCS/T.TXT", enc("t")));
    await call(defs, "cd", ["/mnt/DOCS"]);
    const plain = (await call(defs, "ls")).value as { name: string; modified?: string }[];
    expect(plain.map((r) => r.name)).toEqual(["N.TXT", "T.TXT"]);
    expect(plain[0].modified).toBeUndefined();
    const long = (await call(defs, "ls", { flags: { long: true } })).value as { name: string; modified: string }[];
    expect(long[1]).toEqual({ name: "T.TXT", type: "file", size: 1, modified: "2026-09-22 10:30:00" });
    expect(typeof long[0].modified).toBe("string");
  });

  it("reports missing and non-directory paths with coreutils phrases and no command prefix", async () => {
    const { defs } = setup();
    expect(await callErr(defs, "ls", ["/mnt/nope"])).toMatchObject({ message: "/mnt/nope: No such file or directory", code: "NotFound" });
    expect(await callErr(defs, "ls", ["/mnt/DOCS/N.TXT/x"])).toMatchObject({ message: "/mnt/DOCS/N.TXT/x: Not a directory", code: "NotADirectory" });
    const e = await callErr(defs, "ls", ["/foo"]);
    expect(e.message).toContain("/foo");
    expect(e.help).toContain("/mnt");
  });

  it("dir is an alias of ls", async () => {
    const { defs } = setup();
    expect(defs.find((d) => d.spec.name === "dir")?.spec.summary).toBe("alias of ls");
    expect((await call(defs, "dir", ["/"])).value).toEqual((await call(defs, "ls", ["/"])).value);
  });
});

describe("cd and pwd", () => {
  it("starts at /mnt, stores the canonical case, and walks .. up to the root", async () => {
    const { defs } = setup();
    expect((await call(defs, "pwd")).value).toBe("/mnt");
    await call(defs, "cd", ["/mnt/docs"]);
    expect((await call(defs, "pwd")).value).toBe("/mnt/DOCS");
    await call(defs, "cd", [".."]);
    expect((await call(defs, "pwd")).value).toBe("/mnt");
    await call(defs, "cd", [".."]);
    expect((await call(defs, "pwd")).value).toBe("/");
    await call(defs, "cd", ["dev"]);
    expect((await call(defs, "pwd")).value).toBe("/dev");
    await call(defs, "cd");
    expect((await call(defs, "pwd")).value).toBe("/mnt");
  });

  it("refuses files, devices, and missing paths", async () => {
    const { defs } = setup();
    expect(await callErr(defs, "cd", ["/mnt/Hello world.txt"])).toMatchObject({ message: "/mnt/Hello world.txt: Not a directory", code: "NotADirectory" });
    expect(await callErr(defs, "cd", ["/dev/hda"])).toMatchObject({ message: "/dev/hda: Not a directory", code: "NotADirectory" });
    expect(await callErr(defs, "cd", ["/mnt/nope"])).toMatchObject({ message: "/mnt/nope: No such file or directory", code: "NotFound" });
    expect((await call(defs, "pwd")).value).toBe("/mnt"); // a failed cd leaves cwd alone
  });
});

describe("cat", () => {
  it("prints UTF-8 text, or a blob with --bytes", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "cat", ["/mnt/hello world.txt"]);
    expect(r.value).toBe("hello from the shell\n");
    expect(r.err).toEqual([]);
    const blob = await call(defs, "cat", { positionals: ["/mnt/Hello world.txt"], flags: { bytes: true } });
    expect(blob.value).toEqual(fromBytes(host.vol.readFile("/Hello world.txt")));
    expect((await call(defs, "cat", ["/dev/null"])).value).toBe("");
  });

  it("warns once about binary content and refuses files over DD_MAX_BYTES", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "cat", ["/mnt/DOCS/N.TXT"]);
    expect(r.err).toEqual(["binary file; try cat --bytes /mnt/DOCS/N.TXT | xxd"]);
    expect((r.value as string).length).toBe(3000);
    host.run((v) => v.createFile("/BIG.BIN", new Uint8Array(DD_MAX_BYTES + 1)));
    const e = await callErr(defs, "cat", ["/mnt/BIG.BIN"]);
    expect(e.message).toBe(`/mnt/BIG.BIN: file is ${DD_MAX_BYTES + 1} bytes; cat prints at most ${DD_MAX_BYTES} bytes`);
    expect(e.help).toContain("dd --if=/mnt/BIG.BIN");
    const asBlob = await call(defs, "cat", { positionals: ["/mnt/BIG.BIN"], flags: { bytes: true } });
    expect((asBlob.value as { length: number }).length).toBe(DD_MAX_BYTES + 1);
  });

  it("points devices at dd and directories at their nature", async () => {
    const { defs } = setup();
    expect(await callErr(defs, "cat", ["/mnt"])).toMatchObject({ message: "/mnt: Is a directory", code: "IsADirectory" });
    expect(await callErr(defs, "cat", ["/mnt/DOCS"])).toMatchObject({ message: "/mnt/DOCS: Is a directory", code: "IsADirectory" });
    expect(await callErr(defs, "cat", ["/"])).toMatchObject({ message: "/: Is a directory" });
    const hda = await callErr(defs, "cat", ["/dev/hda"]);
    expect(hda.message).toBe("/dev/hda: is a raw device");
    expect(hda.help).toBe("read a range with: dd --if=/dev/hda --bs=512 --skip=0 --count=1 | xxd");
    expect((await callErr(defs, "cat", ["/dev/zero"])).message).toBe("/dev/zero: is a raw device");
    expect(await callErr(defs, "cat", ["/mnt/nope"])).toMatchObject({ message: "/mnt/nope: No such file or directory", code: "NotFound" });
  });
});

describe("stat", () => {
  it("reports where a file lives: entry slot, FAT entry, chain, data", async () => {
    const { host, defs } = setup();
    const vol = host.vol;
    const g = vol.geometry(), fat = vol.fatEntries(0), owners = vol.clusterOwners();
    const first = owners.find((o) => o.path === "/DOCS/N.TXT")!.firstCluster;
    const slots = findEntrySlots(vol, g, fat, owners, "/DOCS/N.TXT")!;
    const r = (await call(defs, "stat", ["/mnt/docs/n.txt"])).value as Record<string, unknown>;
    expect(r).toMatchObject({
      path: "/mnt/DOCS/N.TXT",
      name: "N.TXT",
      type: "file",
      size: 3000,
      firstCluster: first,
      chain: [first, first + 1],
      clusters: 2,
      entryOffset: `0x${slots.start.toString(16)}`,
      entrySlots: `0x${slots.start.toString(16)}-0x${slots.end.toString(16)}`,
      fatEntryOffset: `0x${(g.reservedSectors * g.bytesPerSector + first * 2).toString(16)}`,
      dataOffset: `0x${clusterByteRange(g, first).start.toString(16)}`,
    });
    expect(typeof r.modified).toBe("string");
    expect(typeof r.created).toBe("string");
    expect(typeof r.accessed).toBe("string");
  });

  it("describes the root, an empty file, and the devices", async () => {
    const { host, defs } = setup();
    const g = host.vol.geometry();
    expect((await call(defs, "stat", ["/mnt"])).value).toMatchObject({
      path: "/mnt", name: "/", type: "dir", size: 0, firstCluster: 0, chain: [], clusters: 0,
      entryOffset: "", fatEntryOffset: "", dataOffset: `0x${(g.firstRootDirSector * g.bytesPerSector).toString(16)}`,
    });
    host.run((v) => v.createFile("/EMPTY", new Uint8Array(0)));
    const empty = (await call(defs, "stat", ["/mnt/empty"])).value as Record<string, unknown>;
    expect(empty).toMatchObject({ path: "/mnt/EMPTY", size: 0, firstCluster: 0, chain: [], fatEntryOffset: "", dataOffset: "" });
    expect(empty.entryOffset).toMatch(/^0x[0-9a-f]+$/);
    expect((await call(defs, "stat", ["/dev/hda"])).value).toEqual({
      path: "/dev/hda", type: "block", size: DISK_BYTES, sectorSize: 512, sectors: 32768, fsType: "FAT16", state: "ok",
    });
    expect((await call(defs, "stat", ["/dev/zero"])).value).toEqual({ path: "/dev/zero", type: "char", size: 0 });
    expect((await call(defs, "stat", ["/"])).value).toEqual({ path: "/", type: "dir", size: 0 });
    expect(await callErr(defs, "stat", ["/mnt/nope"])).toMatchObject({ message: "/mnt/nope: No such file or directory", code: "NotFound" });
  });
});

describe("df and mount", () => {
  it("df counts used clusters from FAT 0", async () => {
    const { host, defs } = setup();
    const g = host.vol.geometry();
    const rows = (await call(defs, "df")).value as Record<string, unknown>[];
    expect(rows).toHaveLength(1);
    // DOCS (1 cluster) + N.TXT (2) + Hello world.txt (1)
    expect(rows[0]).toMatchObject({ filesystem: "/dev/hda", mounted: "/mnt", type: "FAT16", clusterSize: 2048, clusters: g.clusterCount, used: 4, free: g.clusterCount - 4, bytesUsed: 4 * 2048, bytesFree: (g.clusterCount - 4) * 2048 });
    expect(rows[0].use).toMatch(/^\d+%$/);
  });

  it("mount shows one healthy row", async () => {
    const { defs } = setup();
    expect((await call(defs, "mount")).value).toEqual([
      { device: "/dev/hda", mount: "/mnt", type: "FAT16", sectorSize: 512, sectors: 32768, bytes: DISK_BYTES, state: "ok" },
    ]);
  });
});

describe("seek, select, exit", () => {
  it("seek jumps the dump to sector, cluster, hex, and decimal addresses", async () => {
    const { host, defs } = setup();
    const g = host.vol.geometry();
    await call(defs, "seek", ["s:65"]);
    await call(defs, "seek", ["c:3"]);
    await call(defs, "seek", ["0x200"]);
    await call(defs, "seek", ["512"]);
    expect(host.jumps).toEqual([65 * 512, clusterByteRange(g, 3).start, 512, 512]);
  });

  it("seek rejects bad and out-of-range addresses", async () => {
    const { host, defs } = setup();
    expect((await callErr(defs, "seek", ["nope"])).help).toBe(ADDR_HELP);
    expect((await callErr(defs, "seek", ["0x1000000"])).message).toBe(`0x1000000 is past the end of the disk (${DISK_BYTES} bytes)`);
    expect(host.jumps).toEqual([]);
  });

  it("select records the canonical volume path, or null", async () => {
    const { host, defs } = setup();
    await call(defs, "select", ["/mnt/docs/n.txt"]);
    await call(defs, "select");
    await call(defs, "select", ["/mnt"]);
    expect(host.selected).toEqual(["/DOCS/N.TXT", null, null]);
    expect(await callErr(defs, "select", ["/dev/hda"])).toMatchObject({ message: "/dev/hda: only volume paths can be selected" });
    expect(await callErr(defs, "select", ["/mnt/nope"])).toMatchObject({ message: "/mnt/nope: No such file or directory", code: "NotFound" });
    expect(host.selected).toHaveLength(3);
  });

  it("exit closes the terminal", async () => {
    const { host, defs } = setup();
    expect(host.closed).toBe(false);
    await call(defs, "exit");
    expect(host.closed).toBe(true);
  });
});

describe("rewound timeline", () => {
  it("read commands warn once and still show the latest state; navigation does not", async () => {
    const { host, defs } = setup();
    host.rewind(0);
    const warning = 'showing the latest state, not step 1 of 3; click "Back to now" or run a write command';
    expect(rewoundWarning(host)).toBe(warning);
    const ls = await call(defs, "ls", ["/mnt"]);
    expect(ls.err).toEqual([warning]);
    expect((ls.value as { name: string }[]).map((r) => r.name)).toEqual(["DOCS", "Hello world.txt"]);
    for (const [name, args] of [["dir", ["/mnt"]], ["cat", ["/mnt/Hello world.txt"]], ["stat", ["/mnt"]], ["df", []], ["mount", []]] as const) {
      expect((await call(defs, name, [...args])).err, name).toEqual([warning]);
    }
    expect((await call(defs, "pwd")).err).toEqual([]);
    expect((await call(defs, "cd", ["/mnt/DOCS"])).err).toEqual([]);
    host.rewind(-1);
    expect((await call(defs, "ls", ["/"])).err).toEqual(['showing the latest state, not step 0 of 3; click "Back to now" or run a write command']);
    host.rewind(2);
    expect((await call(defs, "ls", ["/"])).err).toEqual([]);
  });
});

describe("corruption", () => {
  it("ls, df, stat report a wiped boot sector; mount says corrupt; restoring it clears the state", async () => {
    const { host, defs } = setup();
    const boot = host.vol.sector(0);
    host.run((v) => v.writeRaw(0, new Uint8Array(512)));
    expect((await call(defs, "mount")).value).toMatchObject([{ state: "corrupt" }]);
    expect((await call(defs, "stat", ["/dev/hda"])).value).toMatchObject({ state: "corrupt" });
    for (const [name, args] of [["ls", ["/mnt"]], ["df", []], ["stat", ["/mnt/DOCS"]]] as const) {
      const e = await callErr(defs, name, [...args]);
      expect(e.message, name).toContain("boot sector no longer parses");
      expect(e.code, name).toBe("CorruptImage");
      expect(e.help, name).toContain("dd --of=/dev/hda");
    }
    expect((await callErr(defs, "ls", ["/mnt"])).message.startsWith("/mnt: ")).toBe(true);
    expect((await call(defs, "ls", ["/dev"])).value).toHaveLength(3); // /dev never touches the volume
    host.run((v) => v.writeRaw(0, boot));
    expect((await call(defs, "mount")).value).toMatchObject([{ state: "ok" }]);
    expect((await call(defs, "ls", ["/mnt"])).value).toHaveLength(2);
  });
});
```

Run it:

```
cd web/ui && pnpm exec vitest run tests/shell/read-commands.test.ts
```

Expected failure: the file fails to load because `../../src/shell/commands` does not exist yet (vitest prints `Error: Failed to load url ../../src/shell/commands` for both the test and `helpers.ts`), so the file is reported as failed and none of its 20 tests run.

- [ ] **Step 3: Write `web/ui/src/shell/commands.ts`**

```ts
import type { CommandArgs, CommandCtx, CommandSpec, FlagSpec, PosArg, Value } from "./types";
import type { DateTime, EntryInfo } from "../lib/wasm";
import { clusterByteRange } from "../core/attribution";
import { findEntrySlots } from "../core/direntry";
import { buildChain } from "../core/fatchain";
import { parseAddr } from "./addr";
import { decodeText, fromBytes } from "./bytes";
import { DD_MAX_BYTES } from "./dd";
import { ShellError, wrapFs } from "./errors";
import { atLatest, type ShellHost } from "./host";
import { canonicalize, Vfs, type Resolved } from "./vfs";

import type { CommandDef } from "./types";
export type { CommandDef } from "./types";

export const RAW_DEVICE_HELP = "read a range with: dd --if=/dev/hda --bs=512 --skip=0 --count=1 | xxd";
export const CORRUPT_HELP =
  "the boot sector no longer parses; rewind on the timeline, or write the saved sector back with: <blob> | dd --of=/dev/hda";

// Spec builders. Every positional is "str" so a bareword like `true` or `512`
// still arrives as text; size and address flags parse their own strings.
export const P = (name: string, desc: string): PosArg => ({ name, shape: "str", desc });
export const F = (long: string, desc: string, extra: Partial<FlagSpec> = {}): FlagSpec => ({ long, desc, ...extra });

/** The one warning every read command emits while the timeline is rewound. */
export function rewoundWarning(host: ShellHost): string {
  return `showing the latest state, not step ${host.cursor + 1} of ${host.historyLength}; click "Back to now" or run a write command`;
}

export function warnIfRewound(host: ShellHost, ctx: CommandCtx): void {
  if (!atLatest(host)) ctx.err(rewoundWarning(host));
}

/** Path-based commands refuse to run while the boot sector does not parse (the Rust gate says the same). */
export function assertMounted(host: ShellHost, display: string): void {
  const c = host.vol.corruption();
  if (c) throw new ShellError(`${display}: ${c}`, { code: "CorruptImage", help: CORRUPT_HELP });
}

export function posStr(args: CommandArgs, i: number): string | undefined {
  const v = args.positionals[i];
  if (v === undefined || v === null) return undefined;
  return typeof v === "string" ? v : String(v);
}

export function reqStr(args: CommandArgs, i: number, name: string): string {
  const v = posStr(args, i);
  if (v === undefined) throw new ShellError(`missing required argument \`${name}\``);
  return v;
}

/** A switch flag is present as `true` or absent entirely; never `false`. */
export function flagOn(args: CommandArgs, name: string): boolean {
  return args.flags[name] === true;
}

export function fmtDate(d: DateTime | null): string {
  if (!d) return "";
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.year}-${p(d.month)}-${p(d.day)} ${p(d.hour)}:${p(d.minute)}:${p(d.second)}`;
}

export const hexAddr = (n: number): string => `0x${n.toString(16)}`;

/** More than 10% control bytes (other than tab, LF, CR). Bytes >= 0x80 count as text so UTF-8 passes. */
export function looksBinary(bytes: Uint8Array): boolean {
  if (bytes.length === 0) return false;
  let odd = 0;
  for (const b of bytes) {
    if ((b < 0x20 && b !== 0x09 && b !== 0x0a && b !== 0x0d) || b === 0x7f) odd++;
  }
  return odd * 10 > bytes.length;
}

type LsRow = { name: string; type: string; size: number; modified?: string };

function lsRows(host: ShellHost, vfs: Vfs, r: Resolved, long: boolean): LsRow[] {
  const vol = host.vol;
  const disk = vol.sectorCount() * vol.sectorSize();
  const row = (name: string, type: string, size: number, modified = ""): LsRow =>
    long ? { name, type, size, modified } : { name, type, size };
  switch (r.kind) {
    case "root":
      return [row("dev", "dir", 0), row("mnt", "dir", 0)];
    case "dev":
      return [row("hda", "block", disk), row("zero", "char", 0), row("null", "char", 0)];
    case "raw":
      return [row("hda", "block", disk)];
    case "zero":
      return [row("zero", "char", 0)];
    case "null":
      return [row("null", "char", 0)];
    case "volume": {
      const display = vfs.display(r);
      assertMounted(host, display);
      let entries: EntryInfo[];
      try {
        const info = vol.stat(r.path);
        entries = info.isDir ? vol.listDir(r.path) : [info];
      } catch (e) {
        throw wrapFs(display, e);
      }
      // On-disk slot order, like the Files pane; not sorted like coreutils.
      return entries.map((e) => row(e.name, e.isDir ? "dir" : "file", e.size, fmtDate(e.modified)));
    }
  }
}

export function readCommands(host: ShellHost, vfs: Vfs): CommandDef[] {
  const lsSpec: CommandSpec = {
    name: "ls",
    summary: "List a directory (/, /dev, /mnt/...)",
    optional: [P("path", "directory or file; defaults to the working directory")],
    flags: [F("long", "add the modified timestamp", { short: "l" })],
  };
  const ls: CommandDef["fn"] = (args, _input, ctx) => {
    warnIfRewound(host, ctx);
    const r = vfs.resolve(posStr(args, 0) ?? vfs.cwd);
    return lsRows(host, vfs, r, flagOn(args, "long"));
  };

  const cd: CommandDef = {
    spec: { name: "cd", summary: "Change the working directory", optional: [P("path", "directory; defaults to /mnt")] },
    fn: (args) => {
      const target = posStr(args, 0);
      if (target === undefined) {
        vfs.cwd = "/mnt";
        return;
      }
      const r = vfs.resolve(target);
      const display = vfs.display(r);
      switch (r.kind) {
        case "root":
          vfs.cwd = "/";
          return;
        case "dev":
          vfs.cwd = "/dev";
          return;
        case "volume": {
          let info: EntryInfo;
          try {
            info = host.vol.stat(r.path);
          } catch (e) {
            throw wrapFs(display, e);
          }
          if (!info.isDir) throw new ShellError(`${display}: Not a directory`, { code: "NotADirectory" });
          vfs.cwd = vfs.toVirtual(canonicalize(host.vol, r.path));
          return;
        }
        default:
          throw new ShellError(`${display}: Not a directory`, { code: "NotADirectory" });
      }
    },
  };

  const pwd: CommandDef = {
    spec: { name: "pwd", summary: "Print the working directory" },
    fn: () => vfs.cwd,
  };

  const cat: CommandDef = {
    spec: {
      name: "cat",
      summary: "Print a file (--bytes for a blob)",
      required: [P("path", "a file under /mnt, or /dev/null")],
      flags: [F("bytes", "return a { bytes, length } blob instead of text")],
    },
    fn: (args, _input, ctx) => {
      warnIfRewound(host, ctx);
      const r = vfs.resolve(reqStr(args, 0, "path"));
      const display = vfs.display(r);
      const asBlob = flagOn(args, "bytes");
      switch (r.kind) {
        case "root":
        case "dev":
          throw new ShellError(`${display}: Is a directory`, { code: "IsADirectory" });
        case "raw":
        case "zero":
          throw new ShellError(`${display}: is a raw device`, { help: RAW_DEVICE_HELP });
        case "null":
          return asBlob ? fromBytes(new Uint8Array(0)) : "";
        case "volume": {
          let bytes: Uint8Array;
          try {
            bytes = host.vol.readFile(r.path);
          } catch (e) {
            throw wrapFs(display, e);
          }
          if (asBlob) return fromBytes(bytes);
          if (bytes.length > DD_MAX_BYTES) {
            throw new ShellError(`${display}: file is ${bytes.length} bytes; cat prints at most ${DD_MAX_BYTES} bytes`, {
              help: `read part of it with: dd --if=${display} --bs=512 --count=1 | xxd`,
            });
          }
          if (looksBinary(bytes)) ctx.err(`binary file; try cat --bytes ${display} | xxd`);
          return decodeText(bytes);
        }
      }
    },
  };

  const stat: CommandDef = {
    spec: { name: "stat", summary: "Where a file lives on disk", required: [P("path", "a path under /mnt, or a device")] },
    fn: (args, _input, ctx) => {
      warnIfRewound(host, ctx);
      const r = vfs.resolve(reqStr(args, 0, "path"));
      const display = vfs.display(r);
      const vol = host.vol;
      switch (r.kind) {
        case "root":
        case "dev":
          return { path: display, type: "dir", size: 0 };
        case "zero":
        case "null":
          return { path: display, type: "char", size: 0 };
        case "raw": {
          const sectorSize = vol.sectorSize();
          const sectors = vol.sectorCount();
          return {
            path: display,
            type: "block",
            size: sectors * sectorSize,
            sectorSize,
            sectors,
            fsType: vol.fsType(),
            state: vol.corruption() ? "corrupt" : "ok",
          };
        }
        case "volume": {
          assertMounted(host, display);
          let info: EntryInfo;
          try {
            info = vol.stat(r.path);
          } catch (e) {
            throw wrapFs(display, e);
          }
          const canon = canonicalize(vol, r.path);
          const g = vol.geometry();
          const fat = vol.fatEntries(0);
          const owners = vol.clusterOwners();
          const first = owners.find((o) => o.path === canon)?.firstCluster ?? 0;
          const chain = buildChain(fat, first);
          const slots = findEntrySlots(vol, g, fat, owners, canon);
          const bps = g.bytesPerSector;
          return {
            path: vfs.toVirtual(canon),
            name: info.name,
            type: info.isDir ? "dir" : "file",
            size: info.size,
            created: fmtDate(info.created),
            modified: fmtDate(info.modified),
            accessed: fmtDate(info.accessed),
            firstCluster: first,
            chain,
            clusters: chain.length,
            entryOffset: slots ? hexAddr(slots.start) : "",
            entrySlots: slots ? `${hexAddr(slots.start)}-${hexAddr(slots.end)}` : "",
            fatEntryOffset: first >= 2 ? hexAddr(g.reservedSectors * bps + first * 2) : "",
            dataOffset: canon === "/" ? hexAddr(g.firstRootDirSector * bps) : first >= 2 ? hexAddr(clusterByteRange(g, first).start) : "",
          };
        }
      }
    },
  };

  const df: CommandDef = {
    spec: { name: "df", summary: "Cluster usage of the mounted volume" },
    fn: (_args, _input, ctx) => {
      warnIfRewound(host, ctx);
      assertMounted(host, "/mnt");
      const vol = host.vol;
      const g = vol.geometry();
      const fat = vol.fatEntries(0);
      let used = 0;
      for (let c = 2; c < g.clusterCount + 2 && c < fat.length; c++) if (fat[c].kind !== "free") used++;
      const free = g.clusterCount - used;
      const clusterSize = g.bytesPerSector * g.sectorsPerCluster;
      const pct = g.clusterCount > 0 ? Math.round((used / g.clusterCount) * 100) : 0;
      return [
        {
          filesystem: "/dev/hda",
          mounted: "/mnt",
          type: vol.fsType(),
          clusterSize,
          clusters: g.clusterCount,
          used,
          free,
          bytesUsed: used * clusterSize,
          bytesFree: free * clusterSize,
          use: `${pct}%`,
        },
      ];
    },
  };

  const mount: CommandDef = {
    spec: { name: "mount", summary: "Mounted volumes" },
    fn: (_args, _input, ctx) => {
      warnIfRewound(host, ctx);
      const vol = host.vol;
      const sectorSize = vol.sectorSize();
      const sectors = vol.sectorCount();
      return [
        {
          device: "/dev/hda",
          mount: "/mnt",
          type: vol.fsType(),
          sectorSize,
          sectors,
          bytes: sectors * sectorSize,
          state: vol.corruption() ? "corrupt" : "ok",
        },
      ];
    },
  };

  const seek: CommandDef = {
    spec: { name: "seek", summary: "Move the hex dump to an address", required: [P("addr", "0x1f, 512, s:65 (sector), or c:3 (cluster)")] },
    fn: (args) => {
      const vol = host.vol;
      const off = parseAddr(reqStr(args, 0, "addr"), vol.geometry());
      const limit = vol.sectorCount() * vol.sectorSize();
      if (off >= limit) throw new ShellError(`${hexAddr(off)} is past the end of the disk (${limit} bytes)`);
      host.jumpTo(off);
    },
  };

  const select: CommandDef = {
    spec: { name: "select", summary: "Highlight a file in the explorer", optional: [P("path", "a path under /mnt; omit to clear")] },
    fn: (args) => {
      const target = posStr(args, 0);
      if (target === undefined) {
        host.select(null);
        return;
      }
      const r = vfs.resolve(target);
      const display = vfs.display(r);
      if (r.kind !== "volume") {
        throw new ShellError(`${display}: only volume paths can be selected`, { help: "select /mnt/<file>, or run select with no argument to clear" });
      }
      try {
        host.vol.stat(r.path);
      } catch (e) {
        throw wrapFs(display, e);
      }
      const canon = canonicalize(host.vol, r.path);
      host.select(canon === "/" ? null : canon);
    },
  };

  const exit: CommandDef = {
    spec: { name: "exit", summary: "Close the terminal drawer" },
    fn: () => {
      host.closeTerminal();
    },
  };

  return [
    { spec: lsSpec, fn: ls },
    { spec: { ...lsSpec, name: "dir", summary: "alias of ls" }, fn: ls },
    cd,
    pwd,
    cat,
    stat,
    df,
    mount,
    seek,
    select,
    exit,
  ];
}

/**
 * Every command the terminal registers. `echo` is a browser-terminal builtin
 * and is not registered here. `vfs` holds the one working directory per page.
 * Task 6 spreads its `writeCommands(host, vfs)` into this array.
 */
export function createCommands(host: ShellHost, vfs: Vfs = new Vfs()): CommandDef[] {
  return [...readCommands(host, vfs)];
}
```

Run the test file again:

```
cd web/ui && pnpm exec vitest run tests/shell/read-commands.test.ts
```

Expected: all 20 tests pass (`Test Files 1 passed`, `Tests 20 passed`).

- [ ] **Step 4: Run the full gate**

```
cd web/ui && pnpm test && pnpm build
```

Expected: every suite passes (the existing suites plus Task 4's `tests/shell/*.test.ts` and this file), then `svelte-check` reports `0 errors, 0 warnings` (both new test files and `commands.ts` are in the tsconfig's `include`), then `vite build` completes. If `svelte-check` reports that `corruption` or `writeRaw` does not exist on `Volume`, the copied wasm package is stale: run `pnpm install` in `web/ui` and rerun.

- [ ] **Step 5: Commit**

```
cd /Users/bsmall/dev/fs-emulator
git add web/ui/src/shell/commands.ts web/ui/tests/shell/helpers.ts web/ui/tests/shell/read-commands.test.ts
git commit -m "feat(ui): add the shell's read commands and command test harness

ls/dir, cd, pwd, cat, stat, df, mount, seek, select and exit over the
ShellHost seam, the rewound-timeline warning, corruption reporting through
Volume.corruption(), and a Volume-backed TestHost with call/callErr for
driving commands under vitest."
```


---

### Task 6: Shell mutating commands, dd, xxd

Spec: `docs/superpowers/specs/2026-09-22-terminal-access-design.md` section 4 (binding). This task
adds the commands that change the disk (`write`, `mkdir`, `rmdir`, `rm`, `touch`, `cp`, `mkfs`),
the `dd` copy runner on top of Task 4's `parseDd`, and `xxd`/`hexdump`. Every mutation goes
through `host.run` so the timeline, diff, ribbon and tree update exactly like a form action;
selections mirror `web/ui/src/components/ActionsPanel.svelte` (`select(path)` after create,
write, mkdir, cp, touch; `select(null)` after rm, rmdir). Errors never carry a command prefix
(browser-terminal adds `` `cmd`: `` itself, `crates/bterm-wasm/src/js_command.rs:409`).

**Files:**
- Modify `web/ui/src/shell/dd.ts` — append `DD_MAX_BYTES` (only if Task 4 did not define it), `runDd`, and its private helpers after the existing `parseDd`.
- Modify `web/ui/src/shell/commands.ts` — add the imports listed in Step 6, the private functions `mutationCommands` and `ddXxdCommands`, and spread both into the array `createCommands` returns.
- Create `web/ui/tests/shell/mutations.test.ts`.
- Modify `web/ui/tests/shell/dd.test.ts` (created by Task 4 for `parseDd`) — append a `describe("dd copies", ...)` block.
- Create `web/ui/tests/shell/xxd-command.test.ts`.

**Interfaces:**

Consumes (exact names from earlier tasks):
- Task 2 (`crates/wasm/pkg/fs_emulator_wasm.d.ts`, after `wasm-pack build crates/wasm --target bundler`): `Volume.writeRaw(offset: number, bytes: Uint8Array): OpRecord`, `Volume.readRaw(offset: number, len: number): Uint8Array`, `Volume.corruption(): string | null`; existing `Volume.stat/readFile/createFile/writeFile/createDir/removeDir/deleteFile/sector/sectorCount/sectorSize/geometry/bootSector/clusterOwners/layout/image`, `FormatOptions`, `EntryInfo`, `OpRecord`.
- Task 4 `web/ui/src/shell/errors.ts`: `class ShellError extends Error { help?: string; code?: string; constructor(message: string, opts?: { help?: string; code?: string }) }`, `wrapFs(display: string, e: unknown): ShellError`.
- Task 4 `web/ui/src/shell/host.ts`: `interface ShellHost { readonly vol: Volume; readonly cursor: number; readonly historyLength: number; run(fn: (v: Volume) => OpRecord): OpRecord; format(options: FormatOptions): void; select(path: string | null): void; jumpTo(offset: number): void; closeTerminal(): void }`, `atLatest(h: ShellHost): boolean`.
- Task 4 `web/ui/src/shell/bytes.ts`: `interface BytesBlob { bytes: string; length: number }`, `toBytes(v: Value): Uint8Array`, `fromBytes(b: Uint8Array): BytesBlob`, `isBlob(v: unknown): v is BytesBlob`.
- Task 4 `web/ui/src/shell/addr.ts`: `parseAddr(v: string | number, g: Geometry): number` (throws `ShellError` with `ADDR_HELP`), `parseSize(v: string | number): number` (throws with `SIZE_HELP`), `ADDR_HELP: string`, `SIZE_HELP: string`.
- Task 4 `web/ui/src/shell/vfs.ts`: `type Resolved = { kind: "root" } | { kind: "dev" } | { kind: "raw" } | { kind: "zero" } | { kind: "null" } | { kind: "volume"; path: string }`, `class Vfs { cwd: string; normalize(input: string): string; resolve(input: string): Resolved; display(r: Resolved): string; toVirtual(volumePath: string): string }`, `canonicalize(vol: Volume, volumePath: string): string`.
- Task 4 `web/ui/src/shell/xxd.ts`: `formatXxd(bytes: Uint8Array, base: number, cols?: number): string`.
- Task 4 `web/ui/src/shell/dd.ts`: `interface DdOpts { if?: string; of?: string; bs: number; count?: number; skip: number; seek: number }`, `parseDd(flags: Record<string, Value>, operands: Value[]): DdOpts`, `DD_MAX_BYTES = 1 << 20`, `planWindow(opts, available): DdWindow`, `formatRecords(len, bs): string`.
- Task 5 `web/ui/src/shell/commands.ts`: `interface CommandDef { spec: CommandSpec; fn: (args: CommandArgs, input: Value, ctx: CommandCtx) => unknown | Promise<unknown> }`, `createCommands(host: ShellHost, vfs?: Vfs): CommandDef[]`, the read commands `ls` and `mount` (used by the corruption round trip), and the rewound-read warning (`showing the latest state, not step N of M; click "Back to now" or run a write command`).
- Task 5 `web/ui/tests/shell/helpers.ts`: `interface TestHost extends ShellHost { history: OpRecord[]; selected: (string | null)[]; jumps: number[]; closed: boolean; rewind(step: number): void }`, `makeHost(vol?: Volume): TestHost` (fresh `Volume.formatFat16(undefined)` by default; `format` replaces `vol` and empties `history`), `interface CallResult { value: unknown; log: string[]; err: string[] }`, `call(defs: CommandDef[], name: string, args?: { positionals?: Value[]; flags?: Record<string, Value> }, input?: Value): Promise<CallResult>`, `callErr(defs, name, args?, input?): Promise<{ message: string; help?: string; code?: string }>`.

Produces:
- `web/ui/src/shell/dd.ts`: `export const DD_MAX_BYTES = 1 << 20` (kept if Task 4 already exports it), `export function runDd(host: ShellHost, vfs: Vfs, opts: DdOpts, input: Value, ctx: CommandCtx): BytesBlob | undefined`.
- `web/ui/src/shell/commands.ts`: `createCommands` now also returns defs named `write`, `mkdir`, `rmdir`, `rm`, `touch`, `cp`, `mkfs`, `dd`, `xxd`, `hexdump`. Task 7 registers them unchanged (`for (const d of createCommands(host)) bt.registerCommand(d.spec, d.fn)`); Task 8's scenario copies their exact command lines.

---

- [ ] **Step 1: Confirm the inputs this task builds on**

Run from `/Users/bsmall/dev/fs-emulator`:

```bash
grep -n "writeRaw\|readRaw\|corruption" crates/wasm/pkg/fs_emulator_wasm.d.ts
grep -n "export" web/ui/src/shell/dd.ts web/ui/src/shell/errors.ts web/ui/src/shell/bytes.ts web/ui/src/shell/addr.ts web/ui/src/shell/vfs.ts web/ui/src/shell/xxd.ts web/ui/src/shell/host.ts
grep -n "export function createCommands\|export type { CommandDef }\|showing the latest state" web/ui/src/shell/commands.ts
grep -n "export" web/ui/tests/shell/helpers.ts
```

Expected: the first command prints the three `Volume` methods (Task 2 built `pkg`); the second prints
`parseDd`, `DdOpts`, `ShellError`, `wrapFs`, `toBytes`, `fromBytes`, `isBlob`, `BytesBlob`, `parseAddr`,
`parseSize`, `ADDR_HELP`, `SIZE_HELP`, `Vfs`, `Resolved`, `canonicalize`, `formatXxd`, `ShellHost`,
`atLatest`; the third prints `createCommands`, `CommandDef` and the rewound warning line; the
fourth prints `makeHost`, `call`, `callErr`. If the first grep prints nothing, run
`wasm-pack build crates/wasm --target bundler` and `cd web/ui && pnpm install` first.

- [ ] **Step 2: Write the failing dd copy tests**

Append the block below to the end of `web/ui/tests/shell/dd.test.ts`. Task 4's file already
imports `describe, expect, it`, `parseDd`, `DD_MAX_BYTES` and `ShellError`; add these five
import lines at the top of the file:

```ts
import { isBlob, toBytes, type BytesBlob } from "../../src/shell/bytes";
import { createCommands } from "../../src/shell/commands";
import { applyChanges } from "../../src/core/patch";
import { Vfs } from "../../src/shell/vfs";
import { call, callErr, makeHost } from "./helpers";
```

Then append:

```ts
const same = (a: Uint8Array, b: Uint8Array) => Buffer.compare(Buffer.from(a), Buffer.from(b)) === 0;
const text = (b: Uint8Array) => new TextDecoder().decode(b);
function setup() {
  const host = makeHost();
  const vfs = new Vfs();
  return { host, vfs, defs: createCommands(host, vfs) };
}

describe("dd copies", () => {
  it("reads sector 0 into a blob and logs the record counts without touching the journal", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "dd", { flags: { if: "/dev/hda", bs: "512", count: "1" } });
    const blob = r.value as BytesBlob;
    expect(isBlob(blob)).toBe(true);
    expect(blob.length).toBe(512);
    expect(same(toBytes(blob), host.vol.sector(0))).toBe(true);
    expect(r.log).toEqual(["1+0 records in", "1+0 records out", "512 bytes copied"]);
    expect(host.history).toEqual([]);
  });

  it("accepts the quoted classic operand form", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "dd", { positionals: ["if=/dev/hda", "bs=256", "skip=2", "count=1"] });
    const blob = r.value as BytesBlob;
    expect(blob.length).toBe(256);
    expect(same(toBytes(blob), host.vol.sector(1).subarray(0, 256))).toBe(true);
  });

  it("copies a file into a new file at --seek, zero-padding the gap", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "alpha");
    const r = await call(defs, "dd", { flags: { if: "/mnt/a.txt", of: "/mnt/b.txt", seek: "2" } });
    expect(r.value).toBeUndefined();
    const b = host.vol.readFile("/b.txt");
    expect(b.length).toBe(1024 + 5);
    expect(b.subarray(0, 1024).every((x) => x === 0)).toBe(true);
    expect(text(b.subarray(1024))).toBe("alpha");
    expect(r.log).toContain("FAT has no partial writes; the whole file was rewritten");
    expect(r.log.slice(-3)).toEqual(["0+1 records in", "0+1 records out", "5 bytes copied"]);
    expect(host.history.map((h) => h.op)).toEqual(["create_file /a.txt", "create_file /b.txt"]);
    expect(host.selected.at(-1)).toBe("/b.txt");
  });

  it("overlays piped bytes onto an existing file at --seek and rewrites it", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "hello world");
    const r = await call(defs, "dd", { flags: { of: "/mnt/a.txt", bs: "1", seek: "6" } }, "XX");
    expect(text(host.vol.readFile("/a.txt"))).toBe("hello XXrld");
    expect(host.history.map((h) => h.op)).toEqual(["create_file /a.txt", "write_file /a.txt"]);
    expect(r.log.slice(-3)).toEqual(["2+0 records in", "2+0 records out", "2 bytes copied"]);
  });

  it("zeroes one sector of /dev/hda and journals exactly that range", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "dd", { flags: { if: "/dev/zero", of: "/dev/hda", seek: "65", count: "1" } });
    expect(r.value).toBeUndefined();
    expect(host.history).toHaveLength(1);
    const rec = host.history[0];
    expect(rec.op).toBe("write_raw 0x8200 +512");
    expect(rec.changes).toHaveLength(1);
    expect(rec.changes[0].offset).toBe(33280);
    expect(rec.changes[0].after.length).toBe(512);
    expect(rec.changes[0].after.every((x) => x === 0)).toBe(true);
    expect(host.vol.readRaw(33280, 512).every((x) => x === 0)).toBe(true);
    expect(r.log).toEqual(["1+0 records in", "1+0 records out", "512 bytes copied"]);
    expect(host.selected).toEqual([]);
  });

  it("refuses a raw write past the end of the disk before journaling anything", async () => {
    const { host, defs } = setup();
    const err = await callErr(defs, "dd", { flags: { if: "/dev/zero", of: "/dev/hda", seek: "32768", count: "1" } });
    expect(err.message).toBe("/dev/hda: Range runs past the end of the disk");
    expect(host.history).toEqual([]);
  });

  it("skipping past the end of the source reads nothing", async () => {
    const { defs } = setup();
    const r = await call(defs, "dd", { flags: { if: "/dev/hda", skip: "40000" } });
    expect((r.value as BytesBlob).length).toBe(0);
    expect(r.log).toEqual(["0+0 records in", "0+0 records out", "0 bytes copied"]);
  });

  it("/dev/zero needs --count", async () => {
    const { defs } = setup();
    const err = await callErr(defs, "dd", { flags: { if: "/dev/zero", of: "/dev/null" } });
    expect(err.message).toBe("/dev/zero is endless; give --count");
  });

  it("caps the read window at DD_MAX_BYTES before any write", async () => {
    const { host, defs } = setup();
    const err = await callErr(defs, "dd", { flags: { if: "/dev/zero", of: "/dev/hda", count: "2049" } });
    expect(err.message).toBe(`refusing to copy ${2049 * 512} bytes in one dd; the limit is ${DD_MAX_BYTES} (1 MiB)`);
    expect(err.help).toContain("--count");
    expect(host.history).toEqual([]);
    const whole = await callErr(defs, "dd", { flags: { if: "/dev/hda" } });
    expect(whole.message).toBe(`refusing to copy ${16 * 1024 * 1024} bytes in one dd; the limit is ${DD_MAX_BYTES} (1 MiB)`);
    const ok = await call(defs, "dd", { flags: { if: "/dev/zero", count: "2048" } });
    expect((ok.value as BytesBlob).length).toBe(DD_MAX_BYTES);
  });

  it("rejects --if together with piped input, and no input at all", async () => {
    const { defs } = setup();
    expect((await callErr(defs, "dd", { flags: { if: "/dev/hda", count: "1" } }, "x")).message).toBe("both --if and piped input given");
    const none = await callErr(defs, "dd", {});
    expect(none.message).toBe("no input");
    expect(none.help).toContain("--if=");
  });

  it("discards into /dev/null and refuses /dev/zero and directories as sinks", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "dd", { flags: { of: "/dev/null" } }, "bye");
    expect(r.value).toBeUndefined();
    expect(r.log).toEqual(["0+1 records in", "0+1 records out", "3 bytes copied"]);
    expect((await callErr(defs, "dd", { flags: { of: "/dev/zero" } }, "x")).message).toBe("/dev/zero: cannot write to /dev/zero");
    expect((await callErr(defs, "dd", { flags: { of: "/mnt" } }, "x")).message).toBe("/mnt: Is a directory");
    expect((await callErr(defs, "dd", { flags: { if: "/mnt" } })).message).toBe("/mnt: Is a directory");
    expect((await callErr(defs, "dd", { flags: { if: "/dev" } })).message).toBe("/dev: Is a directory");
    expect(host.history).toEqual([]);
  });

  it("warns once when reading the disk while rewound", async () => {
    const { host, defs } = setup();
    await call(defs, "dd", { flags: { if: "/dev/zero", of: "/dev/hda", seek: "65", count: "1" } });
    host.rewind(-1);
    const r = await call(defs, "dd", { flags: { if: "/dev/hda", count: "1" } });
    expect(r.err).toHaveLength(1);
    expect(r.err[0]).toContain("showing the latest state");
    expect((r.value as BytesBlob).length).toBe(512);
  });

  it("corruption round trip: wipe sector 0, watch path ops fail, write it back", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "alpha");
    const saved = (await call(defs, "dd", { flags: { if: "/dev/hda", count: "1" } })).value as BytesBlob;
    expect(saved.length).toBe(512);

    await call(defs, "dd", { flags: { if: "/dev/zero", of: "/dev/hda", count: "1" } });
    expect(host.vol.corruption()).toContain("boot sector no longer parses after a raw write");
    const err = await callErr(defs, "ls", { positionals: ["/mnt"] });
    expect(err.message).toContain("boot sector no longer parses after a raw write");
    const corrupt = await call(defs, "mount");
    expect(corrupt.value).toMatchObject([{ state: "corrupt" }]);
    expect(host.vol.layout().length).toBeGreaterThan(0); // unmounted inspection still works

    await call(defs, "dd", { flags: { of: "/dev/hda" } }, saved);
    expect(host.vol.corruption()).toBeNull();
    expect(host.history.map((h) => h.op)).toEqual(["create_file /a.txt", "write_raw 0x0 +512", "write_raw 0x0 +512"]);
    const ls = await call(defs, "ls", { positionals: ["/mnt"] });
    expect(ls.value).toMatchObject([{ name: "a.txt", type: "file", size: 5 }]);
    expect((await call(defs, "mount")).value).toMatchObject([{ state: "ok" }]);

    // The two raw writes rewind byte for byte, the way VolumeStore.seek does it.
    const image = host.vol.image();
    applyChanges(image, host.history[2].changes, "reverse");
    expect(image.subarray(0, 512).every((x) => x === 0)).toBe(true);
    applyChanges(image, host.history[1].changes, "reverse");
    expect(same(image.subarray(0, 512), toBytes(saved))).toBe(true);
    expect(same(image, host.vol.image())).toBe(true);
  });
});
```

- [ ] **Step 3: Write the failing xxd command tests**

Create `web/ui/tests/shell/xxd-command.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { createCommands } from "../../src/shell/commands";
import { Vfs } from "../../src/shell/vfs";
import { formatXxd } from "../../src/shell/xxd";
import type { BytesBlob } from "../../src/shell/bytes";
import { call, callErr, makeHost } from "./helpers";

function setup() {
  const host = makeHost();
  const vfs = new Vfs();
  return { host, vfs, defs: createCommands(host, vfs) };
}
const lines = (v: unknown) => (v as string).split("\n");

describe("xxd command", () => {
  it("dumps one sector of /dev/hda at absolute addresses by default", async () => {
    const { defs } = setup();
    const r = await call(defs, "xxd", { positionals: ["/dev/hda"] });
    const out = lines(r.value);
    expect(out).toHaveLength(32);
    expect(out[0]).toMatch(/^00000000: eb3c 90/);
    expect(out[31]).toMatch(/^000001f0: /);
    expect(out[31]).toContain("55aa");
    expect(out[31].endsWith("U.")).toBe(true);
    expect(r.err).toEqual([]);
  });

  it("--offset and --len pick a window of the disk; addresses stay absolute", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "Hello, FAT16!");
    const g = host.vol.geometry();
    const cluster2 = g.firstDataSector * g.bytesPerSector;
    expect(cluster2).toBe(0xc200);
    const r = await call(defs, "xxd", { positionals: ["/dev/hda"], flags: { offset: "c:2", len: "16" } });
    expect(r.value).toBe(formatXxd(host.vol.readRaw(cluster2, 16), cluster2, 16));
    expect(lines(r.value)).toHaveLength(1);
    expect((r.value as string).startsWith("0000c200: ")).toBe(true);
    expect(r.value).toContain("Hello, FAT16!");
    const root = await call(defs, "xxd", { positionals: ["/dev/hda"], flags: { offset: "s:65", len: "32" } });
    const out = lines(root.value);
    expect(out).toHaveLength(2);
    expect(out[0].startsWith("00008200: ")).toBe(true);
    expect(out[1].startsWith("00008210: ")).toBe(true);
    const tail = await call(defs, "xxd", { positionals: ["/dev/hda"], flags: { offset: "0xfffff0", len: "1k" } });
    expect(lines(tail.value)).toHaveLength(1); // clipped at the end of the disk
  });

  it("dumps a volume file, a piped blob, and a piped string from address 0", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "Hello, FAT16!");
    const expected = formatXxd(host.vol.readFile("/a.txt"), 0, 16);
    expect((await call(defs, "xxd", { positionals: ["/mnt/a.txt"] })).value).toBe(expected);
    const blob = (await call(defs, "dd", { flags: { if: "/mnt/a.txt" } })).value as BytesBlob;
    expect((await call(defs, "xxd", {}, blob)).value).toBe(expected);
    expect((await call(defs, "xxd", {}, "Hello, FAT16!")).value).toBe(expected);
    expect(lines((await call(defs, "xxd", { positionals: ["/mnt/a.txt"], flags: { cols: 8 } })).value)).toHaveLength(2);
    expect((await call(defs, "xxd", { positionals: ["/mnt/a.txt"], flags: { offset: "7", len: "5" } })).value)
      .toBe(formatXxd(new TextEncoder().encode("FAT16"), 7, 16));
  });

  it("/dev/zero dumps zeros for --len; hexdump is the same command", async () => {
    const { defs } = setup();
    const z = await call(defs, "xxd", { positionals: ["/dev/zero"], flags: { len: "4" } });
    expect(z.value).toBe(formatXxd(new Uint8Array(4), 0, 16));
    const a = await call(defs, "xxd", { positionals: ["/dev/hda"] });
    const b = await call(defs, "hexdump", { positionals: ["/dev/hda"] });
    expect(b.value).toBe(a.value);
  });

  it("reports the mistakes it can see", async () => {
    const { defs } = setup();
    const none = await callErr(defs, "xxd", {});
    expect(none.message).toBe("nothing to dump");
    expect(none.help).toContain("| xxd");
    expect((await callErr(defs, "xxd", { positionals: ["/dev/zero"] })).message).toBe("/dev/zero: give --len");
    expect((await callErr(defs, "xxd", { positionals: ["/mnt"] })).message).toBe("/mnt: Is a directory");
    expect((await callErr(defs, "xxd", { positionals: ["/dev"] })).message).toBe("/dev: Is a directory");
    expect((await callErr(defs, "xxd", { positionals: ["/mnt/nope"] })).message).toBe("/mnt/nope: No such file or directory");
    expect((await callErr(defs, "xxd", { positionals: ["/dev/hda"], flags: { cols: 0 } })).message).toBe("--cols must be 1..64");
    expect((await callErr(defs, "xxd", { positionals: ["/dev/hda"], flags: { offset: "0x1000000" } })).message)
      .toBe("0x1000000 is past the end of the disk (16777216 bytes)");
    expect((await callErr(defs, "xxd", { positionals: ["/dev/hda"], flags: { len: "2M" } })).help).toContain("--len");
  });

  it("warns when dumping the disk or a file while rewound", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "x");
    host.rewind(-1);
    expect((await call(defs, "xxd", { positionals: ["/dev/hda"] })).err[0]).toContain("showing the latest state");
    expect((await call(defs, "xxd", { positionals: ["/mnt/a.txt"] })).err[0]).toContain("showing the latest state");
    expect((await call(defs, "xxd", {}, "piped")).err).toEqual([]);
  });
});
```

- [ ] **Step 4: Run the new tests and watch them fail**

```bash
cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm exec vitest run tests/shell/dd.test.ts tests/shell/xxd-command.test.ts
```

Expected: the `parseDd` suite from Task 4 still passes; every case in `dd copies` and `xxd command`
fails. The first failure is either a module error (`"DD_MAX_BYTES" is not exported by
src/shell/dd.ts`, when Task 4 did not define it) or, from the helper, a rejection such as
`no command named "dd" is registered` / `no command named "xxd" is registered` because
`createCommands` does not return those defs yet.

- [ ] **Step 5: Implement the dd copy runner**

Append to `web/ui/src/shell/dd.ts`, after the existing `parseDd` function. Add the import lines
first (at the top of the file, next to Task 4's imports; keep any that already exist):

```ts
import type { CommandCtx } from "./types";
import { fromBytes, toBytes, type BytesBlob } from "./bytes";
import { ShellError, wrapFs } from "./errors";
import type { ShellHost } from "./host";
import { canonicalize, type Vfs } from "./vfs";
```

Then append the runner:

```ts
/** The bytes `dd` will copy: the source window `skip*bs` for `count*bs` (or to the end). */
function readSource(host: ShellHost, vfs: Vfs, opts: DdOpts, input: Value): Uint8Array {
  const slice = (all: Uint8Array): Uint8Array => {
    const { start, len } = planWindow(opts, all.length);
    return all.subarray(Math.min(start, all.length), Math.min(start, all.length) + len);
  };
  if (opts.if === undefined) {
    if (input === null) {
      throw new ShellError("no input", { help: "give --if=<path> or pipe bytes in, e.g. cat --bytes /mnt/a | dd --of=/mnt/b" });
    }
    return slice(toBytes(input));
  }
  if (input !== null) {
    throw new ShellError("both --if and piped input given", { help: "drop --if to copy the piped bytes, or drop the pipe" });
  }
  const src = vfs.resolve(opts.if);
  const display = vfs.display(src);
  switch (src.kind) {
    case "raw": {
      const { start, len } = planWindow(opts, host.vol.sectorCount() * host.vol.sectorSize());
      return len === 0 ? new Uint8Array(0) : host.vol.readRaw(start, len);
    }
    case "zero": {
      if (opts.count === undefined) throw new ShellError("/dev/zero is endless; give --count");
      return new Uint8Array(planWindow(opts, Infinity).len);
    }
    case "null":
      return new Uint8Array(0);
    case "volume": {
      let file: Uint8Array;
      try {
        file = host.vol.readFile(src.path);
      } catch (e) {
        throw wrapFs(display, e);
      }
      return slice(file);
    }
    default:
      throw new ShellError(`${display}: Is a directory`);
  }
}

/** Overlay `data` at `off` on the file at `path` (zero-padded; FAT has no partial writes). */
function writeVolumeFile(host: ShellHost, path: string, display: string, off: number, data: Uint8Array, ctx: CommandCtx): void {
  let existing: Uint8Array | null = null;
  try {
    existing = host.vol.readFile(path);
  } catch (e) {
    if ((e as { code?: string }).code !== "NotFound") throw wrapFs(display, e);
  }
  const out = new Uint8Array(Math.max(existing?.length ?? 0, off + data.length));
  if (existing) out.set(existing, 0);
  out.set(data, off);
  const had = existing !== null;
  try {
    host.run((v) => (had ? v.writeFile(path, out) : v.createFile(path, out)));
  } catch (e) {
    throw wrapFs(display, e);
  }
  if (had || off > 0) ctx.log("FAT has no partial writes; the whole file was rewritten");
  host.select(canonicalize(host.vol, path));
}

/**
 * Run one parsed `dd`. Reads the source window (capped at `DD_MAX_BYTES` before anything is
 * written), copies it to `--of` (`/dev/hda` via `writeRaw`, a volume file via overlay and
 * rewrite, `/dev/null` discards) or returns it as a blob when `--of` is absent, then logs
 * `records in`, `records out` and `bytes copied` like the real tool.
 */
export function runDd(host: ShellHost, vfs: Vfs, opts: DdOpts, input: Value, ctx: CommandCtx): BytesBlob | undefined {
  const data = readSource(host, vfs, opts, input);
  const off = opts.seek * opts.bs;
  let result: BytesBlob | undefined;
  if (opts.of === undefined) {
    result = fromBytes(data);
  } else {
    const dst = vfs.resolve(opts.of);
    const display = vfs.display(dst);
    switch (dst.kind) {
      case "null":
        break;
      case "zero":
        throw new ShellError("/dev/zero: cannot write to /dev/zero");
      case "raw": {
        const disk = host.vol.sectorCount() * host.vol.sectorSize();
        // Checked here as well as in Rust so an offset above 2^32 never reaches the u32 binding.
        if (off + data.length > disk) throw new ShellError("/dev/hda: Range runs past the end of the disk", { code: "OutOfBounds" });
        if (data.length > 0) {
          try {
            host.run((v) => v.writeRaw(off, data));
          } catch (e) {
            throw wrapFs(display, e);
          }
        }
        break;
      }
      case "volume":
        writeVolumeFile(host, dst.path, display, off, data, ctx);
        break;
      default:
        throw new ShellError(`${display}: Is a directory`);
    }
  }
  ctx.log(`${formatRecords(data.length, opts.bs)} in`);
  ctx.log(`${formatRecords(data.length, opts.bs)} out`);
  ctx.log(`${data.length} bytes copied`);
  return result;
}
```

- [ ] **Step 6: Register dd, xxd and hexdump in the command table**

In `web/ui/src/shell/commands.ts`, make sure these imports are present (merge with Task 5's
existing import lines; do not duplicate a binding that is already imported):

```ts
import type { CommandArgs, CommandCtx, CommandSpec, Value } from "./types";
import type { EntryInfo, FormatOptions, Volume } from "../lib/wasm";
import { ADDR_HELP, SIZE_HELP, parseAddr, parseSize } from "./addr";
import { toBytes } from "./bytes";
import { DD_MAX_BYTES, parseDd, runDd } from "./dd";
import { ShellError, wrapFs } from "./errors";
import { atLatest, type ShellHost } from "./host";
import { canonicalize, Vfs, type Resolved } from "./vfs";
import { formatXxd } from "./xxd";
```

Task 5 exports `warnIfRewound(host, ctx)` from this file; the code below calls it as is.

Add this module-level function (below `warnIfRewound`, above `createCommands`):

```ts
/** `dd`, `xxd` and its alias `hexdump`. */
function ddXxdCommands(host: ShellHost, vfs: Vfs): CommandDef[] {
  const dd: CommandDef = {
    spec: {
      name: "dd",
      summary: "Copy blocks between files, /dev/hda and /dev/zero (at most 1 MiB per run)",
      rest: { name: "operand", shape: "str", desc: "quoted classic operands: 'if=/dev/hda' 'bs=512' 'count=1'" },
      flags: [
        { long: "if", shape: "str", desc: "source path (default: the piped input)" },
        { long: "of", shape: "str", desc: "destination path (default: return the bytes as a blob)" },
        { long: "bs", shape: "str", desc: "block size, default 512 (k and M suffixes)" },
        { long: "count", shape: "str", desc: "blocks to copy (default: to the end of the source)" },
        { long: "skip", shape: "str", desc: "blocks to skip at the start of the source" },
        { long: "seek", shape: "str", desc: "blocks to skip at the start of the destination" },
      ],
    },
    fn: (args, input, ctx) => {
      const opts = parseDd(args.flags, args.positionals);
      if (opts.if !== undefined) {
        const src = vfs.resolve(opts.if);
        if (src.kind === "raw" || src.kind === "volume") warnIfRewound(host, ctx);
      }
      return runDd(host, vfs, opts, input, ctx);
    },
  };

  const xxdSpec = (name: string, summary: string): CommandSpec => ({
    name,
    summary,
    optional: [{ name: "path", shape: "str", desc: "/mnt/file, /dev/hda (one sector by default) or /dev/zero --len; omit to dump piped bytes" }],
    flags: [
      { long: "offset", shape: "str", desc: `start address (${ADDR_HELP})` },
      { long: "len", shape: "str", desc: `bytes to show (${SIZE_HELP})` },
      { long: "cols", shape: "int", desc: "bytes per row, default 16" },
    ],
  });

  const xxd: CommandDef["fn"] = (args, input, ctx) => {
    const colsFlag = args.flags.cols;
    const cols = colsFlag === undefined || colsFlag === null ? 16 : Number(colsFlag);
    if (!Number.isInteger(cols) || cols < 1 || cols > 64) throw new ShellError("--cols must be 1..64");
    const offsetFlag = args.flags.offset;
    const offset = offsetFlag === undefined || offsetFlag === null ? undefined : parseAddr(String(offsetFlag), host.vol.geometry());
    const lenFlag = args.flags.len;
    const len = lenFlag === undefined || lenFlag === null ? undefined : parseSize(String(lenFlag));
    const base = offset ?? 0;
    const slice = (all: Uint8Array): Uint8Array => {
      const from = Math.min(base, all.length);
      const to = len === undefined ? all.length : Math.min(all.length, from + len);
      return all.subarray(from, to);
    };
    const tooBig = (n: number) => new ShellError(`refusing to dump ${n} bytes; the limit is ${DD_MAX_BYTES}`, { help: "use a smaller --len" });

    let bytes: Uint8Array;
    if (args.positionals.length === 0) {
      if (input === null) throw new ShellError("nothing to dump", { help: "xxd <path>, or pipe bytes in: dd --if=/dev/hda --count=1 | xxd" });
      bytes = slice(toBytes(input));
    } else {
      const target = vfs.resolve(String(args.positionals[0]));
      const display = vfs.display(target);
      switch (target.kind) {
        case "raw": {
          warnIfRewound(host, ctx);
          const disk = host.vol.sectorCount() * host.vol.sectorSize();
          if (base >= disk) throw new ShellError(`0x${base.toString(16)} is past the end of the disk (${disk} bytes)`);
          const n = Math.min(len ?? host.vol.sectorSize(), disk - base);
          if (n > DD_MAX_BYTES) throw tooBig(n);
          bytes = host.vol.readRaw(base, n);
          break;
        }
        case "zero": {
          if (len === undefined) throw new ShellError("/dev/zero: give --len", { help: SIZE_HELP });
          if (len > DD_MAX_BYTES) throw tooBig(len);
          bytes = new Uint8Array(len);
          break;
        }
        case "null":
          bytes = new Uint8Array(0);
          break;
        case "volume": {
          warnIfRewound(host, ctx);
          let file: Uint8Array;
          try {
            file = host.vol.readFile(target.path);
          } catch (e) {
            throw wrapFs(display, e);
          }
          bytes = slice(file);
          break;
        }
        default:
          throw new ShellError(`${display}: Is a directory`);
      }
    }
    if (bytes.length > DD_MAX_BYTES) throw tooBig(bytes.length);
    return formatXxd(bytes, base, cols);
  };

  return [
    dd,
    { spec: xxdSpec("xxd", "Hex dump a file, /dev/hda, or piped bytes"), fn: xxd },
    { spec: xxdSpec("hexdump", "alias of xxd"), fn: xxd },
  ];
}
```

Then wire it into `createCommands`:

Old:
```ts
  return [...readCommands(host, vfs)];
```

New:
```ts
  return [...readCommands(host, vfs), ...ddXxdCommands(host, vfs)];
```

- [ ] **Step 7: Run the dd and xxd tests and see them pass**

```bash
cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm exec vitest run tests/shell/dd.test.ts tests/shell/xxd-command.test.ts && pnpm exec svelte-check --tsconfig ./tsconfig.json
```

Expected: both files green (the `parseDd` suite plus 13 `dd copies` cases and 6 `xxd command`
cases), `svelte-check` reports 0 errors. If the corruption round trip fails at
`host.vol.corruption()`, Task 2's `pkg` is stale: rebuild with
`wasm-pack build crates/wasm --target bundler` and rerun.

- [ ] **Step 8: Commit the dd and xxd commands**

```bash
cd /Users/bsmall/dev/fs-emulator && git add web/ui/src/shell/dd.ts web/ui/src/shell/commands.ts web/ui/tests/shell/dd.test.ts web/ui/tests/shell/xxd-command.test.ts && git commit -m "feat(shell): dd copy runner and xxd/hexdump commands"
```

- [ ] **Step 9: Write the failing mutation tests**

Create `web/ui/tests/shell/mutations.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { createCommands } from "../../src/shell/commands";
import { Vfs } from "../../src/shell/vfs";
import { call, callErr, makeHost } from "./helpers";

const text = (b: Uint8Array) => new TextDecoder().decode(b);
function setup() {
  const host = makeHost();
  const vfs = new Vfs();
  return { host, vfs, defs: createCommands(host, vfs) };
}
const ops = (host: ReturnType<typeof makeHost>) => host.history.map((r) => r.op);

describe("write", () => {
  it("creates a file from piped text, then rewrites it, selecting the file each time", async () => {
    const { host, defs } = setup();
    const first = await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "hi");
    expect(first.value).toBeUndefined();
    expect(first.log).toEqual(["2 bytes -> /mnt/a.txt"]);
    expect(ops(host)).toEqual(["create_file /a.txt"]);
    expect(text(host.vol.readFile("/a.txt"))).toBe("hi");
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "hello");
    expect(ops(host)).toEqual(["create_file /a.txt", "write_file /a.txt"]);
    expect(text(host.vol.readFile("/a.txt"))).toBe("hello");
    expect(host.selected).toEqual(["/a.txt", "/a.txt"]);
  });

  it("resolves relative paths against the working directory and keeps the on-disk case", async () => {
    const { host, vfs, defs } = setup();
    await call(defs, "mkdir", { positionals: ["/mnt/DOCS"] });
    vfs.cwd = "/mnt/docs";
    await call(defs, "write", { positionals: ["Note.txt"] }, "n");
    expect(ops(host).at(-1)).toBe("create_file /docs/Note.txt");
    expect(host.selected.at(-1)).toBe("/DOCS/Note.txt");
    expect(text(host.vol.readFile("/DOCS/NOTE.TXT"))).toBe("n");
  });

  it("--append reads, concatenates and rewrites the whole file, and says so", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "hi");
    const r = await call(defs, "write", { positionals: ["/mnt/a.txt"], flags: { append: true } }, "hi");
    expect(text(host.vol.readFile("/a.txt"))).toBe("hihi");
    expect(ops(host)).toEqual(["create_file /a.txt", "write_file /a.txt"]);
    expect(r.log).toEqual([
      "appended 2 bytes by rewriting the whole file (FAT has no append; the old chain is freed and reallocated)",
      "4 bytes -> /mnt/a.txt",
    ]);
    const fresh = await call(defs, "write", { positionals: ["/mnt/new.txt"], flags: { append: true } }, "x");
    expect(fresh.log).toEqual(["1 bytes -> /mnt/new.txt"]);
    expect(ops(host).at(-1)).toBe("create_file /new.txt");
  });

  it("accepts echo's forms: a number, and a list of words joined by spaces", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/n.txt"] }, 12345);
    expect(text(host.vol.readFile("/n.txt"))).toBe("12345");
    await call(defs, "write", { positionals: ["/mnt/w.txt"] }, ["hello", "world"]);
    expect(text(host.vol.readFile("/w.txt"))).toBe("hello world");
  });

  it("writes raw bytes to /dev/hda at --at and does not select anything", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "write", { positionals: ["/dev/hda"], flags: { at: "s:65" } }, "RAW");
    expect(ops(host)).toEqual(["write_raw 0x8200 +3"]);
    expect(text(host.vol.readRaw(33280, 3))).toBe("RAW");
    expect(r.log).toEqual(["3 bytes -> /dev/hda at 0x8200"]);
    expect(host.selected).toEqual([]);
    await call(defs, "write", { positionals: ["/dev/hda"], flags: { at: "43" } }, "SHELLDISK  ");
    expect(host.vol.bootSector().volumeLabel.startsWith("SHELLDISK")).toBe(true);
  });

  it("/dev/null discards without journaling", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "write", { positionals: ["/dev/null"] }, "gone");
    expect(r.log).toEqual(["4 bytes -> /dev/null"]);
    expect(host.history).toEqual([]);
  });

  it("rejects the mistakes it can see before touching the disk", async () => {
    const { host, defs } = setup();
    const none = await callErr(defs, "write", { positionals: ["/mnt/a.txt"] });
    expect(none.message).toBe("nothing to write");
    expect(none.help).toContain("echo hi | write");
    expect((await callErr(defs, "write", { positionals: ["/dev/hda"] }, "x")).message).toBe("/dev/hda: give --at <addr>");
    expect((await callErr(defs, "write", { positionals: ["/dev/hda"], flags: { at: "0", append: true } }, "x")).message).toBe("--append is not supported on /dev/hda");
    expect((await callErr(defs, "write", { positionals: ["/dev/hda"], flags: { at: "0" } }, "")).message).toBe("nothing to write");
    expect((await callErr(defs, "write", { positionals: ["/dev/hda"], flags: { at: "0x1000000" } }, "x")).message).toBe("/dev/hda: Range runs past the end of the disk");
    expect((await callErr(defs, "write", { positionals: ["/mnt/a.txt"], flags: { at: "0" } }, "x")).message).toBe("--at only applies to /dev/hda");
    expect((await callErr(defs, "write", { positionals: ["/mnt"] }, "x")).message).toBe("/mnt: Is a directory");
    expect((await callErr(defs, "write", { positionals: ["/"] }, "x")).message).toBe("/: Is a directory");
    expect((await callErr(defs, "write", { positionals: ["/dev/zero"] }, "x")).message).toBe("/dev/zero: cannot write to /dev/zero");
    expect((await callErr(defs, "write", { positionals: ["/mnt/nope/a.txt"] }, "x")).message).toBe("/mnt/nope/a.txt: No such file or directory");
    expect(host.history).toEqual([]);
  });
});

describe("mkdir, rmdir, rm, touch", () => {
  it("mkdir then rmdir go through the journal and mirror the explorer's selection", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    expect(r.value).toBeUndefined();
    expect(ops(host)).toEqual(["create_dir /D"]);
    expect(host.vol.stat("/D").isDir).toBe(true);
    expect((await callErr(defs, "mkdir", { positionals: ["/mnt/D"] })).message).toBe("/mnt/D: File exists");
    await call(defs, "rmdir", { positionals: ["/mnt/D"] });
    expect(ops(host)).toEqual(["create_dir /D", "remove_dir /D"]);
    expect(host.selected).toEqual(["/D", null]);
  });

  it("mkdir resolves relative to the working directory and refuses non-volume targets", async () => {
    const { host, vfs, defs } = setup();
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    vfs.cwd = "/mnt/D";
    await call(defs, "mkdir", { positionals: ["E"] });
    expect(ops(host).at(-1)).toBe("create_dir /D/E");
    expect(host.selected.at(-1)).toBe("/D/E");
    expect((await callErr(defs, "mkdir", { positionals: ["/mnt"] })).message).toBe("/mnt: File exists");
    expect((await callErr(defs, "mkdir", { positionals: ["/dev"] })).message).toBe("/dev: File exists");
    expect((await callErr(defs, "mkdir", { positionals: ["/dev/hda"] })).message).toBe("/dev/hda: cannot create a directory on a device");
    expect((await callErr(defs, "mkdir", { positionals: ["/mnt/nope/X"] })).message).toBe("/mnt/nope/X: No such file or directory");
  });

  it("rmdir refuses a non-empty directory, a file, the mount point and devices", async () => {
    const { host, defs } = setup();
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    await call(defs, "write", { positionals: ["/mnt/D/f.txt"] }, "f");
    expect((await callErr(defs, "rmdir", { positionals: ["/mnt/D"] })).message).toBe("/mnt/D: Directory not empty");
    expect((await callErr(defs, "rmdir", { positionals: ["/mnt/D/f.txt"] })).message).toBe("/mnt/D/f.txt: Not a directory");
    expect((await callErr(defs, "rmdir", { positionals: ["/mnt"] })).message).toBe("/mnt: cannot remove the mount point");
    expect((await callErr(defs, "rmdir", { positionals: ["/dev/hda"] })).message).toBe("/dev/hda: Not a directory");
    expect((await callErr(defs, "rmdir", { positionals: ["/mnt/nope"] })).message).toBe("/mnt/nope: No such file or directory");
    expect(ops(host)).toEqual(["create_dir /D", "create_file /D/f.txt"]);
  });

  it("rm deletes a file, clears the selection, and refuses directories and devices", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "a");
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    const r = await call(defs, "rm", { positionals: ["/mnt/a.txt"] });
    expect(r.value).toBeUndefined();
    expect(ops(host)).toEqual(["create_file /a.txt", "create_dir /D", "delete_file /a.txt"]);
    expect(host.selected).toEqual(["/a.txt", "/D", null]);
    const dir = await callErr(defs, "rm", { positionals: ["/mnt/D"] });
    expect(dir.message).toBe("/mnt/D: Is a directory");
    expect(dir.help).toBe("use rmdir");
    expect((await callErr(defs, "rm", { positionals: ["/mnt"] })).message).toBe("/mnt: Is a directory");
    expect((await callErr(defs, "rm", { positionals: ["/dev/hda"] })).message).toBe("/dev/hda: cannot remove a device");
    expect((await callErr(defs, "rm", { positionals: ["/mnt/nope"] })).message).toBe("/mnt/nope: No such file or directory");
    expect(ops(host)).toHaveLength(3);
  });

  it("touch creates an empty file once and is a logged no-op afterwards", async () => {
    const { host, defs } = setup();
    const first = await call(defs, "touch", { positionals: ["/mnt/e.txt"] });
    expect(first.log).toEqual(["created empty /mnt/e.txt"]);
    expect(ops(host)).toEqual(["create_file /e.txt"]);
    expect(host.vol.stat("/e.txt").size).toBe(0);
    const again = await call(defs, "touch", { positionals: ["/mnt/e.txt"] });
    expect(again.log).toEqual(["/mnt/e.txt exists; FAT explorer has no timestamp-only update, nothing written"]);
    expect(ops(host)).toEqual(["create_file /e.txt"]);
    expect(host.selected).toEqual(["/e.txt", "/e.txt"]);
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    expect((await callErr(defs, "touch", { positionals: ["/mnt/D"] })).message).toBe("/mnt/D: Is a directory");
    expect((await callErr(defs, "touch", { positionals: ["/dev/hda"] })).message).toBe("/dev/hda: cannot touch a device");
  });
});

describe("cp", () => {
  it("copies a file into a new cluster chain", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "alpha");
    const r = await call(defs, "cp", { positionals: ["/mnt/a.txt", "/mnt/b.txt"] });
    expect(r.value).toBeUndefined();
    expect(r.log).toEqual(["5 bytes -> /mnt/b.txt"]);
    expect(text(host.vol.readFile("/b.txt"))).toBe("alpha");
    expect(ops(host)).toEqual(["create_file /a.txt", "create_file /b.txt"]);
    const owners = host.vol.clusterOwners();
    const a = owners.find((o) => o.path === "/a.txt")!;
    const b = owners.find((o) => o.path === "/b.txt")!;
    expect(a.firstCluster).toBe(2);
    expect(b.firstCluster).toBe(3);
    expect(host.selected.at(-1)).toBe("/b.txt");
  });

  it("into an existing directory uses the source's on-disk basename; onto a file overwrites", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/Alpha.txt"] }, "alpha");
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    const r = await call(defs, "cp", { positionals: ["/mnt/alpha.txt", "/mnt/d"] });
    expect(r.log).toEqual(["5 bytes -> /mnt/D/Alpha.txt"]);
    expect(ops(host).at(-1)).toBe("create_file /D/Alpha.txt");
    expect(host.selected.at(-1)).toBe("/D/Alpha.txt");
    await call(defs, "write", { positionals: ["/mnt/Alpha.txt"] }, "beta!");
    await call(defs, "cp", { positionals: ["/mnt/Alpha.txt", "/mnt/D/Alpha.txt"] });
    expect(ops(host).at(-1)).toBe("write_file /D/Alpha.txt");
    expect(text(host.vol.readFile("/D/Alpha.txt"))).toBe("beta!");
  });

  it("refuses devices and directories as the source", async () => {
    const { defs } = setup();
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    expect((await callErr(defs, "cp", { positionals: ["/dev/hda", "/mnt/x"] })).message).toBe("/dev/hda: use dd for devices");
    expect((await callErr(defs, "cp", { positionals: ["/mnt/D", "/mnt/x"] })).message).toBe("/mnt/D: Is a directory");
    expect((await callErr(defs, "cp", { positionals: ["/mnt/nope", "/mnt/x"] })).message).toBe("/mnt/nope: No such file or directory");
    expect((await callErr(defs, "cp", { positionals: ["/mnt/D", "/dev/null"] })).message).toBe("/mnt/D: Is a directory");
  });
});

describe("mkfs", () => {
  it("formats with the given geometry, resets cwd and clears the timeline", async () => {
    const { host, vfs, defs } = setup();
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    vfs.cwd = "/mnt/D";
    const r = await call(defs, "mkfs", { flags: { sectors: 8192, spc: 1, label: "SHELLDISK" } });
    expect(r.value).toBeUndefined();
    expect(r.log).toEqual(["formatted /dev/hda as FAT16; the timeline was cleared"]);
    expect(host.vol.geometry().totalSectors).toBe(8192);
    expect(host.vol.geometry().sectorsPerCluster).toBe(1);
    expect(host.vol.bootSector().volumeLabel.startsWith("SHELLDISK")).toBe(true);
    expect(host.historyLength).toBe(0);
    expect(host.vol.listDir("/")).toEqual([]);
    expect(vfs.cwd).toBe("/mnt");
    expect(host.selected.at(-1)).toBe("/D"); // mkfs selects nothing; the host adapter resets the selection
  });

  it("surfaces the core's geometry error with its code and leaves the volume alone", async () => {
    const { host, defs } = setup();
    const err = await callErr(defs, "mkfs", { flags: { sectors: 5 } });
    expect(err.message).toMatch(/^\/dev\/hda: invalid geometry: /);
    expect(err.code).toBe("InvalidGeometry");
    expect(host.vol.geometry().totalSectors).toBe(32768);
    expect((await callErr(defs, "mkfs", { flags: { spc: -1 } })).message).toBe("--spc must be a non-negative integer");
  });
});
```

- [ ] **Step 10: Run the mutation tests and watch them fail**

```bash
cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm exec vitest run tests/shell/mutations.test.ts
```

Expected: every case fails from the helper with `no command named "write" is registered`,
`no command named "mkdir" is registered` and so on for `rmdir`, `rm`, `touch`, `cp` and `mkfs`
(the dd tests in Step 2 that call `write` were also failing for this reason until now; they go
green in Step 12).

- [ ] **Step 11: Implement the mutating commands**

Add this module-level function to `web/ui/src/shell/commands.ts`, next to `ddXxdCommands`
(the imports from Step 6 already cover everything it uses):

```ts
/** `vol.stat(path)`, or `null` when the path does not exist; other failures become ShellErrors. */
function statIfExists(vol: Volume, path: string, display: string): EntryInfo | null {
  try {
    return vol.stat(path);
  } catch (e) {
    if ((e as { code?: string }).code === "NotFound") return null;
    throw wrapFs(display, e);
  }
}

/** `write`, `mkdir`, `rmdir`, `rm`, `touch`, `cp`, `mkfs`: every disk change goes through `host.run`. */
function mutationCommands(host: ShellHost, vfs: Vfs): CommandDef[] {
  const pathArg = (args: CommandArgs, i: number): string => String(args.positionals[i]);
  const joinPath = (dir: string, name: string): string => (dir === "/" ? `/${name}` : `${dir}/${name}`);
  const baseName = (p: string): string => p.slice(p.lastIndexOf("/") + 1);
  /** The volume path behind `target`, or the ShellError for `/`, `/dev` and the devices. */
  const volumePathOf = (target: Resolved, display: string, deviceMsg: string, dirMsg = "Is a directory"): string => {
    if (target.kind === "volume") return target.path;
    if (target.kind === "root" || target.kind === "dev") throw new ShellError(`${display}: ${dirMsg}`);
    throw new ShellError(`${display}: ${deviceMsg}`);
  };
  const flagGiven = (v: Value | undefined): boolean => v !== undefined && v !== null;

  const write: CommandDef = {
    spec: {
      name: "write",
      summary: "Write piped text or bytes to a file, or to /dev/hda --at <addr>",
      required: [{ name: "path", shape: "str", desc: "/mnt/... or /dev/hda" }],
      flags: [
        { long: "append", desc: "read the file, append the input and rewrite the whole file" },
        { long: "at", shape: "str", desc: `disk address for /dev/hda (${ADDR_HELP})` },
      ],
    },
    fn: (args, input, ctx) => {
      const target = vfs.resolve(pathArg(args, 0));
      const display = vfs.display(target);
      if (input === null) throw new ShellError("nothing to write", { help: "pipe text or bytes in, e.g. echo hi | write /mnt/a.txt" });
      const data = toBytes(input);
      const append = args.flags.append === true;
      const at = args.flags.at;
      if (target.kind === "null") {
        ctx.log(`${data.length} bytes -> /dev/null`);
        return;
      }
      if (target.kind === "zero") throw new ShellError("/dev/zero: cannot write to /dev/zero");
      if (target.kind === "raw") {
        if (append) throw new ShellError("--append is not supported on /dev/hda", { help: "give --at <addr> to place the bytes" });
        if (!flagGiven(at)) throw new ShellError("/dev/hda: give --at <addr>", { help: ADDR_HELP });
        if (data.length === 0) throw new ShellError("nothing to write", { help: "a raw write needs at least one byte" });
        const off = parseAddr(String(at), host.vol.geometry());
        const disk = host.vol.sectorCount() * host.vol.sectorSize();
        if (off + data.length > disk) throw new ShellError("/dev/hda: Range runs past the end of the disk", { code: "OutOfBounds" });
        try {
          host.run((v) => v.writeRaw(off, data));
        } catch (e) {
          throw wrapFs(display, e);
        }
        ctx.log(`${data.length} bytes -> /dev/hda at 0x${off.toString(16)}`);
        return;
      }
      if (target.kind !== "volume") throw new ShellError(`${display}: Is a directory`);
      if (flagGiven(at)) throw new ShellError("--at only applies to /dev/hda");
      const path = target.path;
      const existing = statIfExists(host.vol, path, display);
      if (existing?.isDir) throw new ShellError(`${display}: Is a directory`);
      let out = data;
      if (append && existing) {
        let old: Uint8Array;
        try {
          old = host.vol.readFile(path);
        } catch (e) {
          throw wrapFs(display, e);
        }
        out = new Uint8Array(old.length + data.length);
        out.set(old, 0);
        out.set(data, old.length);
      }
      const had = existing !== null;
      try {
        host.run((v) => (had ? v.writeFile(path, out) : v.createFile(path, out)));
      } catch (e) {
        throw wrapFs(display, e);
      }
      if (append && had) ctx.log(`appended ${data.length} bytes by rewriting the whole file (FAT has no append; the old chain is freed and reallocated)`);
      ctx.log(`${out.length} bytes -> ${display}`);
      host.select(canonicalize(host.vol, path));
    },
  };

  const mkdir: CommandDef = {
    spec: {
      name: "mkdir",
      summary: "Create a directory under /mnt",
      required: [{ name: "path", shape: "str", desc: "/mnt/..." }],
    },
    fn: (args) => {
      const target = vfs.resolve(pathArg(args, 0));
      const display = vfs.display(target);
      const path = volumePathOf(target, display, "cannot create a directory on a device", "File exists");
      if (path === "/") throw new ShellError("/mnt: File exists");
      try {
        host.run((v) => v.createDir(path));
      } catch (e) {
        throw wrapFs(display, e);
      }
      host.select(canonicalize(host.vol, path));
    },
  };

  const rmdir: CommandDef = {
    spec: {
      name: "rmdir",
      summary: "Remove an empty directory",
      required: [{ name: "path", shape: "str", desc: "/mnt/..." }],
    },
    fn: (args) => {
      const target = vfs.resolve(pathArg(args, 0));
      const display = vfs.display(target);
      const path = volumePathOf(target, display, "Not a directory", "cannot remove a virtual directory");
      if (path === "/") throw new ShellError("/mnt: cannot remove the mount point");
      const info = statIfExists(host.vol, path, display);
      if (info === null) throw new ShellError(`${display}: No such file or directory`, { code: "NotFound" });
      if (!info.isDir) throw new ShellError(`${display}: Not a directory`, { code: "NotADirectory" });
      try {
        host.run((v) => v.removeDir(path));
      } catch (e) {
        throw wrapFs(display, e);
      }
      host.select(null);
    },
  };

  const rm: CommandDef = {
    spec: {
      name: "rm",
      summary: "Delete a file",
      required: [{ name: "path", shape: "str", desc: "/mnt/..." }],
    },
    fn: (args) => {
      const target = vfs.resolve(pathArg(args, 0));
      const display = vfs.display(target);
      const path = volumePathOf(target, display, "cannot remove a device");
      if (path === "/") throw new ShellError("/mnt: Is a directory", { help: "use rmdir" });
      const info = statIfExists(host.vol, path, display);
      if (info === null) throw new ShellError(`${display}: No such file or directory`, { code: "NotFound" });
      if (info.isDir) throw new ShellError(`${display}: Is a directory`, { help: "use rmdir", code: "IsADirectory" });
      try {
        host.run((v) => v.deleteFile(path));
      } catch (e) {
        throw wrapFs(display, e);
      }
      host.select(null);
    },
  };

  const touch: CommandDef = {
    spec: {
      name: "touch",
      summary: "Create an empty file (an existing file is left alone)",
      required: [{ name: "path", shape: "str", desc: "/mnt/..." }],
    },
    fn: (args, _input, ctx) => {
      const target = vfs.resolve(pathArg(args, 0));
      const display = vfs.display(target);
      const path = volumePathOf(target, display, "cannot touch a device");
      if (path === "/") throw new ShellError("/mnt: Is a directory");
      const info = statIfExists(host.vol, path, display);
      if (info?.isDir) throw new ShellError(`${display}: Is a directory`);
      if (info) {
        ctx.log(`${display} exists; FAT explorer has no timestamp-only update, nothing written`);
      } else {
        try {
          host.run((v) => v.createFile(path, new Uint8Array(0)));
        } catch (e) {
          throw wrapFs(display, e);
        }
        ctx.log(`created empty ${display}`);
      }
      host.select(canonicalize(host.vol, path));
    },
  };

  const cp: CommandDef = {
    spec: {
      name: "cp",
      summary: "Copy a file (into a directory keeps its name)",
      required: [
        { name: "src", shape: "str", desc: "/mnt/file" },
        { name: "dst", shape: "str", desc: "/mnt/file or /mnt/dir" },
      ],
    },
    fn: (args, _input, ctx) => {
      const src = vfs.resolve(pathArg(args, 0));
      const srcDisplay = vfs.display(src);
      const dst = vfs.resolve(pathArg(args, 1));
      const srcPath = volumePathOf(src, srcDisplay, "use dd for devices");
      let data: Uint8Array;
      try {
        data = host.vol.readFile(srcPath);
      } catch (e) {
        throw wrapFs(srcDisplay, e);
      }
      let dstPath = volumePathOf(dst, vfs.display(dst), "use dd for devices");
      let dstDisplay = vfs.display(dst);
      let info = statIfExists(host.vol, dstPath, dstDisplay);
      if (info?.isDir) {
        dstPath = joinPath(canonicalize(host.vol, dstPath), baseName(canonicalize(host.vol, srcPath)));
        dstDisplay = vfs.toVirtual(dstPath);
        info = statIfExists(host.vol, dstPath, dstDisplay);
        if (info?.isDir) throw new ShellError(`${dstDisplay}: Is a directory`);
      }
      const had = info !== null;
      const path = dstPath;
      try {
        host.run((v) => (had ? v.writeFile(path, data) : v.createFile(path, data)));
      } catch (e) {
        throw wrapFs(dstDisplay, e);
      }
      ctx.log(`${data.length} bytes -> ${dstDisplay}`);
      host.select(canonicalize(host.vol, path));
    },
  };

  const mkfs: CommandDef = {
    spec: {
      name: "mkfs",
      summary: "Format /dev/hda as FAT16 (clears the timeline)",
      flags: [
        { long: "sectors", shape: "int", desc: "total sectors (default 32768 = 16 MB)" },
        { long: "spc", shape: "int", desc: "sectors per cluster (default 4)" },
        { long: "label", shape: "str", desc: "volume label, up to 11 characters" },
        { long: "root-entries", shape: "int", desc: "root directory entries (default 512)" },
        { long: "fats", shape: "int", desc: "FAT copies (default 2)" },
        { long: "reserved", shape: "int", desc: "reserved sectors (default 1)" },
      ],
    },
    fn: (args, _input, ctx) => {
      const int = (name: string): number | undefined => {
        const v = args.flags[name];
        if (!flagGiven(v)) return undefined;
        if (typeof v !== "number" || !Number.isInteger(v) || v < 0) throw new ShellError(`--${name} must be a non-negative integer`);
        return v;
      };
      const options: FormatOptions = {};
      const sectors = int("sectors");
      if (sectors !== undefined) options.totalSectors = sectors;
      const spc = int("spc");
      if (spc !== undefined) options.sectorsPerCluster = spc;
      const rootEntries = int("root-entries");
      if (rootEntries !== undefined) options.rootEntries = rootEntries;
      const fats = int("fats");
      if (fats !== undefined) options.fatCount = fats;
      const reserved = int("reserved");
      if (reserved !== undefined) options.reservedSectors = reserved;
      const label = args.flags.label;
      if (flagGiven(label)) options.volumeLabel = String(label);
      try {
        host.format(options);
      } catch (e) {
        throw wrapFs("/dev/hda", e);
      }
      vfs.cwd = "/mnt";
      ctx.log("formatted /dev/hda as FAT16; the timeline was cleared");
    },
  };

  return [write, mkdir, rmdir, rm, touch, cp, mkfs];
}
```

Then extend the spread added in Step 6 so `createCommands` returns the mutations as well.

Old:
```ts
  return [...readCommands(host, vfs), ...ddXxdCommands(host, vfs)];
```

New:
```ts
  return [...readCommands(host, vfs), ...mutationCommands(host, vfs), ...ddXxdCommands(host, vfs)];
```

- [ ] **Step 12: Run the whole web suite, the type check and the build**

```bash
cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm test && pnpm build
```

Expected: `vitest run` reports every file passing, including `tests/shell/mutations.test.ts`
(17 cases), `tests/shell/dd.test.ts` and `tests/shell/xxd-command.test.ts`, with the earlier
suites (`integration`, `scenarios`, Task 4's and Task 5's shell tests) unchanged. `pnpm build`
runs `svelte-check` (0 errors, 0 warnings from `src/shell`) and `vite build` succeeds. Nothing
under `crates/` changed, so the Rust gates (`cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`) are unaffected; run them anyway if the
branch has uncommitted Rust changes from a sibling task.

- [ ] **Step 13: Commit the mutating commands**

```bash
cd /Users/bsmall/dev/fs-emulator && git add web/ui/src/shell/commands.ts web/ui/tests/shell/mutations.test.ts && git commit -m "feat(shell): write, mkdir, rmdir, rm, touch, cp and mkfs commands"
```


---

### Task 7: UI integration: drawer, keyboard, store host

Spec section 5 of `docs/superpowers/specs/2026-09-22-terminal-access-design.md`. This task puts the terminal on screen: a bottom drawer in `web/ui` that lazily creates a `@benjamin-small/browser-terminal` instance into a host element, registers the shell commands from Task 5/6 over a `ShellHost` wired to the rune stores, and adds the keyboard/button plumbing. Nothing here changes the shell modules or the Rust side.

All paths are under `/Users/bsmall/dev/fs-emulator/`. Run every `pnpm` command from `web/ui`. Vitest runs under node with no DOM, so the two new tests cover the pure helpers only; the drawer itself is verified by `pnpm build` (svelte-check, then vite build) and the manual script in the last step.

**Files:**

Create:
- `web/ui/src/core/keys.ts` — `inTextEntry(e)`
- `web/ui/src/core/terminalHeight.ts` — `clampHeight(px, innerHeight)` and the height constants
- `web/ui/src/state/terminal.svelte.ts` — `TerminalStore` / `terminal`
- `web/ui/src/shell/storeHost.svelte.ts` — `createStoreHost(closeTerminal)`
- `web/ui/src/components/TerminalPanel.svelte` — the drawer
- `web/ui/tests/keys.test.ts`
- `web/ui/tests/terminalHeight.test.ts`

Modify:
- `web/ui/src/App.svelte` — imports (after line 13), `onKeydown` (lines 15–28), `.app` div / topbar (lines 31–33), `<TerminalPanel />` (after line 51)
- `web/ui/src/components/ScenarioPanel.svelte` — import (after line 3), `onKeydown` (lines 58–64)
- `web/ui/src/app.css` — `.app` rule (line 15), the `@media (max-width: 760px)` block (lines 30–33), and a new "Terminal drawer" section appended at the end (after line 153)

**Interfaces:**

Consumes (from earlier tasks; verify each exists before starting, see Step 0):
- Task 4, `web/ui/src/shell/host.ts`:
  - `export interface ShellHost { readonly vol: Volume; readonly cursor: number; readonly historyLength: number; run(fn: (v: Volume) => OpRecord): OpRecord; format(options: FormatOptions): void; select(path: string | null): void; jumpTo(offset: number): void; closeTerminal(): void; }`
  - `export function statusToError(s: { text: string; code?: string } | null): Error & { code?: string }`
- Task 5/6, `web/ui/src/shell/commands.ts`:
  - `export type { CommandDef } from "./types"` (declared in Task 4's `types.ts` as `{ spec: CommandSpec; fn: CommandFn }`)
  - `export function createCommands(host: ShellHost, vfs?: Vfs): CommandDef[]`
- Task 4, `web/ui/package.json`: `"@benjamin-small/browser-terminal": "0.2.0"` and `"@xterm/xterm": "^6.0.0"` in `dependencies`; `web/ui/vite.config.ts` `optimizeDeps.exclude` includes `"@benjamin-small/browser-terminal"`.
- Existing stores: `volume` (`web/ui/src/state/volume.svelte.ts`: `vol`, `cursor`, `history`, `status`, `run(fn): OpRecord | null`, `format(options?)`) and `selection` (`web/ui/src/state/selection.svelte.ts`: `select(path)`, `jumpTo(offset)`).
- `@benjamin-small/browser-terminal@0.2.0`: `BrowserTerminal.create(opts: { mount?: HTMLElement }): Promise<BrowserTerminal>`, `registerCommand(spec: CommandSpec, fn: CommandFn): void`, `dispose(): void`; the xterm helper textarea carries class `xterm-helper-textarea`; xterm's stylesheet is `@xterm/xterm/css/xterm.css` (the package has no `exports` map, so the deep import resolves).

Produces (later tasks — the scenario/docs task — rely on these names):
- `web/ui/src/core/keys.ts`: `export function inTextEntry(e: PathEvent): boolean`, `export interface PathEvent { composedPath(): EventTarget[] }`, `export interface EntryTarget { tagName?: string; isContentEditable?: boolean; closest?: (selector: string) => unknown }`
- `web/ui/src/core/terminalHeight.ts`: `export function clampHeight(px: number, innerHeight: number): number`, `export const TERM_MIN_PX = 120`, `export const TERM_MAX_FRACTION = 0.6`, `export const TERM_DEFAULT_PX = 220`
- `web/ui/src/state/terminal.svelte.ts`: `export type TerminalReady = "idle" | "loading" | "ready" | "error"`, `export class TerminalStore { open: boolean; height: number; focusNonce: number; ready: TerminalReady; error: string | null; toggle(): void; openAndFocus(): void; close(): void; setHeight(px: number): void }`, `export const terminal: TerminalStore`
- `web/ui/src/shell/storeHost.svelte.ts`: `export function createStoreHost(closeTerminal: () => void): ShellHost`
- `web/ui/src/components/TerminalPanel.svelte`: default component, no props; renders `<section id="terminal-drawer" class="terminal-drawer panel">`
- DOM contract: `<button id="terminal-toggle">` in the topbar; the `--term-h` custom property on `.app`.

---

- [ ] **Step 0: Confirm the inputs from Tasks 4–6 are in place**

Run from `web/ui`:

```
grep -n '"@benjamin-small/browser-terminal"\|"@xterm/xterm"' package.json
grep -n 'browser-terminal' vite.config.ts
grep -n 'export interface ShellHost\|export function statusToError' src/shell/host.ts
grep -n 'export function createCommands\|export type { CommandDef }' src/shell/commands.ts
ls node_modules/@xterm/xterm/css/xterm.css node_modules/@benjamin-small/browser-terminal/dist/index.js
```

Expected: every grep prints a match and both files are listed. If `@xterm/xterm` is missing from `package.json`, run `pnpm add "@xterm/xterm@^6.0.0"` and commit `pnpm-lock.yaml` with the message `chore(ui): add @xterm/xterm for the terminal stylesheet` before continuing. If `ShellHost` or `createCommands` is missing, stop: this task depends on Tasks 4–6.

- [ ] **Step 1: Write the failing test for `inTextEntry`**

Create `web/ui/tests/keys.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { inTextEntry } from "../src/core/keys";

// vitest runs under node: no DOM, so events and targets are plain objects shaped like
// the pieces inTextEntry reads (composedPath, tagName, isContentEditable, closest).
const ev = (...path: unknown[]) => ({ composedPath: () => path as EventTarget[] });
const el = (tagName: string, extra: Record<string, unknown> = {}) => ({ tagName, closest: () => null, ...extra });

describe("inTextEntry", () => {
  it("is true for form fields", () => {
    expect(inTextEntry(ev(el("INPUT")))).toBe(true);
    expect(inTextEntry(ev(el("TEXTAREA")))).toBe(true);
    expect(inTextEntry(ev(el("SELECT")))).toBe(true);
  });

  it("is true for contenteditable", () => {
    expect(inTextEntry(ev(el("DIV", { isContentEditable: true })))).toBe(true);
  });

  it("is true anywhere inside the terminal drawer", () => {
    const drawer = el("SECTION");
    const inside = el("DIV", { closest: (s: string) => (s === ".terminal-drawer" ? drawer : null) });
    expect(inTextEntry(ev(inside))).toBe(true);
  });

  it("is false for buttons, the body, and an empty path", () => {
    expect(inTextEntry(ev(el("BUTTON")))).toBe(false);
    expect(inTextEntry(ev(el("BODY")))).toBe(false);
    expect(inTextEntry(ev())).toBe(false);
  });

  it("looks at the original target, not a wrapper later in the path", () => {
    expect(inTextEntry(ev(el("TEXTAREA"), el("DIV"), el("BODY")))).toBe(true);
    expect(inTextEntry(ev(el("BUTTON"), el("TEXTAREA")))).toBe(false);
  });

  it("tolerates a target without closest (a text node or the window)", () => {
    expect(inTextEntry(ev({ tagName: undefined }))).toBe(false);
  });
});
```

- [ ] **Step 2: Run the test and watch it fail**

```
pnpm exec vitest run tests/keys.test.ts
```

Expected: the file fails to load with `Failed to resolve import "../src/core/keys"` (module not found).

- [ ] **Step 3: Implement `inTextEntry`**

Create `web/ui/src/core/keys.ts`:

```ts
/** The pieces of an event target the shortcut guard reads. Structural rather than
 *  `HTMLElement` so vitest (node, no DOM) can pass plain objects. */
export interface EntryTarget {
  tagName?: string;
  isContentEditable?: boolean;
  closest?: (selector: string) => unknown;
}

/** The one method of `Event` the guard needs. */
export interface PathEvent {
  composedPath(): EventTarget[];
}

/**
 * True when a keydown started somewhere that consumes typing: an input, textarea,
 * select, contenteditable, or anywhere inside the terminal drawer. Reads
 * `composedPath()[0]`, the original target, so a `window` listener sees the real
 * element the key was typed into.
 *
 * Why the drawer clause: xterm cancels ordinary keydowns, so typing in the terminal
 * never reaches `window`, but the Ctrl-B prefix chord and the key after it bubble
 * with the xterm helper textarea as target. Without this, Ctrl-B `[` would scrub the
 * timeline and Ctrl-B `n` would advance a scenario.
 */
export function inTextEntry(e: PathEvent): boolean {
  const t = e.composedPath()[0] as EntryTarget | undefined;
  if (!t) return false;
  const tag = t.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
  if (t.isContentEditable === true) return true;
  return typeof t.closest === "function" && t.closest(".terminal-drawer") != null;
}
```

- [ ] **Step 4: Run the test and watch it pass**

```
pnpm exec vitest run tests/keys.test.ts
```

Expected: `Test Files 1 passed`, `Tests 6 passed`.

- [ ] **Step 5: Route the existing global shortcuts through `inTextEntry`**

Modify `web/ui/src/App.svelte`. Replace lines 13–28 (the `volume` import through the end of `onKeydown`):

Old:

```svelte
  import { volume } from "./state/volume.svelte";

  // `[` / `]` scrub the timeline and `/` jumps to the path field, all from anywhere
  // except a text field, so typing a path or file content in the Actions panel isn't
  // hijacked. `n` / `p` (scenario step) are handled by ScenarioPanel itself, since
  // they only apply while a scenario is running.
  function onKeydown(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement | null)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;
    if (e.key === "[") volume.seek(Math.max(0, volume.cursor - 1));
    else if (e.key === "]") volume.seek(volume.cursor + 1);
    else if (e.key === "/") {
      e.preventDefault();
      document.getElementById("action-path")?.focus();
    }
  }
```

New:

```svelte
  import { inTextEntry } from "./core/keys";
  import { volume } from "./state/volume.svelte";

  // `[` / `]` scrub the timeline and `/` jumps to the path field, all from anywhere
  // except text entry, so typing a path or file content in the Actions panel isn't
  // hijacked. `n` / `p` (scenario step) are handled by ScenarioPanel itself, since
  // they only apply while a scenario is running. The terminal drawer counts as text
  // entry too (see core/keys.ts for the Ctrl-B chord case).
  function onKeydown(e: KeyboardEvent) {
    if (inTextEntry(e)) return;
    if (e.key === "[") volume.seek(Math.max(0, volume.cursor - 1));
    else if (e.key === "]") volume.seek(volume.cursor + 1);
    else if (e.key === "/") {
      e.preventDefault();
      document.getElementById("action-path")?.focus();
    }
  }
```

Modify `web/ui/src/components/ScenarioPanel.svelte`. Replace lines 1–3:

Old:

```svelte
<script lang="ts">
  import { all } from "../scenarios";
  import { scenarios } from "../state/scenarios.svelte";
```

New:

```svelte
<script lang="ts">
  import { inTextEntry } from "../core/keys";
  import { all } from "../scenarios";
  import { scenarios } from "../state/scenarios.svelte";
```

And replace lines 58–64 (now 59–65 after the import):

Old:

```svelte
  function onKeydown(e: KeyboardEvent) {
    if (!scenarios.current) return;
    const tag = (e.target as HTMLElement | null)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
    if (e.key === "n") advance();
    else if (e.key === "p") scenarios.prev();
  }
```

New:

```svelte
  function onKeydown(e: KeyboardEvent) {
    if (!scenarios.current || inTextEntry(e)) return;
    if (e.key === "n") advance();
    else if (e.key === "p") scenarios.prev();
  }
```

Run:

```
pnpm test && pnpm build
```

Expected: all test files pass (the existing suites plus `keys.test.ts`); `svelte-check` reports 0 errors; `vite build` writes `dist/`.

- [ ] **Step 6: Commit**

```
git add web/ui/src/core/keys.ts web/ui/tests/keys.test.ts web/ui/src/App.svelte web/ui/src/components/ScenarioPanel.svelte
git commit -m "feat(ui): guard global shortcuts with inTextEntry so the terminal drawer is exempt"
```

- [ ] **Step 7: Write the failing test for `clampHeight`**

Create `web/ui/tests/terminalHeight.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { clampHeight, TERM_DEFAULT_PX, TERM_MAX_FRACTION, TERM_MIN_PX } from "../src/core/terminalHeight";

describe("clampHeight", () => {
  it("passes an in-range height through", () => {
    expect(clampHeight(220, 1000)).toBe(220);
    expect(clampHeight(TERM_MIN_PX, 1000)).toBe(TERM_MIN_PX);
    expect(clampHeight(600, 1000)).toBe(600);
  });

  it("floors at the minimum", () => {
    expect(clampHeight(50, 1000)).toBe(TERM_MIN_PX);
    expect(clampHeight(-10, 1000)).toBe(TERM_MIN_PX);
    expect(clampHeight(0, 1000)).toBe(TERM_MIN_PX);
  });

  it("caps at 60% of the viewport, rounded down", () => {
    expect(clampHeight(900, 1000)).toBe(600);
    expect(clampHeight(900, 999)).toBe(Math.floor(999 * TERM_MAX_FRACTION));
  });

  it("lets the floor win when the viewport is shorter than 200px", () => {
    // 60% of 150 is 90, below the 120px floor; the floor keeps the prompt usable.
    expect(clampHeight(300, 150)).toBe(TERM_MIN_PX);
  });

  it("falls back to the default for a non-finite request (corrupt localStorage)", () => {
    expect(clampHeight(Number.NaN, 1000)).toBe(TERM_DEFAULT_PX);
    expect(clampHeight(Number.POSITIVE_INFINITY, 1000)).toBe(TERM_DEFAULT_PX);
  });

  it("returns an integer", () => {
    expect(clampHeight(200.4, 1000)).toBe(200);
    expect(clampHeight(200.6, 1000)).toBe(201);
  });
});
```

- [ ] **Step 8: Run the test and watch it fail**

```
pnpm exec vitest run tests/terminalHeight.test.ts
```

Expected: `Failed to resolve import "../src/core/terminalHeight"`.

- [ ] **Step 9: Implement `clampHeight`**

Create `web/ui/src/core/terminalHeight.ts`:

```ts
/** Smallest useful drawer: the bar plus about five terminal rows. */
export const TERM_MIN_PX = 120;
/** The drawer never takes more than this share of the viewport. */
export const TERM_MAX_FRACTION = 0.6;
/** Height before the user has dragged the bar. */
export const TERM_DEFAULT_PX = 220;

/**
 * Clamp a requested drawer height to 120px..60% of the viewport, as an integer.
 * The floor wins when the viewport is so short that 60% is under 120px. A
 * non-finite request (a corrupt localStorage value) falls back to the default
 * before clamping, so the store never persists NaN.
 */
export function clampHeight(px: number, innerHeight: number): number {
  const wanted = Number.isFinite(px) ? px : TERM_DEFAULT_PX;
  const max = Math.floor(innerHeight * TERM_MAX_FRACTION);
  return Math.max(TERM_MIN_PX, Math.min(Math.round(wanted), max));
}
```

- [ ] **Step 10: Run the test and watch it pass**

```
pnpm exec vitest run tests/terminalHeight.test.ts
```

Expected: `Tests 6 passed`.

- [ ] **Step 11: Create the terminal store**

Create `web/ui/src/state/terminal.svelte.ts`:

```ts
import { clampHeight, TERM_DEFAULT_PX } from "../core/terminalHeight";

const HEIGHT_KEY = "fs-explorer.terminal.height";

export type TerminalReady = "idle" | "loading" | "ready" | "error";

/**
 * Drawer state. DOM-free on purpose: focus moves and the browser-terminal instance
 * live in TerminalPanel.svelte, so this module stays importable anywhere.
 */
export class TerminalStore {
  open = $state(false);
  /** Drawer height in px, persisted; App.svelte feeds it to `.app` as `--term-h`. */
  height = $state(TERM_DEFAULT_PX);
  /** Bumped by openAndFocus(); TerminalPanel refocuses the shell whenever it changes. */
  focusNonce = $state(0);
  ready = $state<TerminalReady>("idle");
  error = $state<string | null>(null);

  constructor() {
    this.height = clampHeight(readStoredHeight(), viewportHeight());
  }

  toggle() {
    if (this.open) this.close();
    else this.openAndFocus();
  }

  openAndFocus() {
    this.open = true;
    this.focusNonce++;
  }

  close() {
    this.open = false;
  }

  /** Clamp to the current viewport and remember the result across reloads. */
  setHeight(px: number) {
    this.height = clampHeight(px, viewportHeight());
    try {
      localStorage.setItem(HEIGHT_KEY, String(this.height));
    } catch {
      // Private mode or a full quota: the height simply does not persist.
    }
  }
}

function viewportHeight(): number {
  return typeof window === "undefined" ? 800 : window.innerHeight;
}

function readStoredHeight(): number {
  try {
    if (typeof localStorage === "undefined") return TERM_DEFAULT_PX;
    return Number(localStorage.getItem(HEIGHT_KEY) ?? TERM_DEFAULT_PX);
  } catch {
    return TERM_DEFAULT_PX;
  }
}

export const terminal = new TerminalStore();
```

Run:

```
pnpm test && pnpm exec svelte-check --tsconfig ./tsconfig.json
```

Expected: all tests pass; svelte-check reports 0 errors (the store is not imported by any component yet; the check only type-checks it).

- [ ] **Step 12: Commit**

```
git add web/ui/src/core/terminalHeight.ts web/ui/tests/terminalHeight.test.ts web/ui/src/state/terminal.svelte.ts
git commit -m "feat(ui): terminal drawer store with a clamped, persisted height"
```

- [ ] **Step 13: Create the store-backed `ShellHost`**

Create `web/ui/src/shell/storeHost.svelte.ts`. This is the only shell module that touches runes, and it is not imported by any test (vitest has no Svelte plugin, so `*.svelte.ts` cannot load there).

```ts
import type { FormatOptions, OpRecord, Volume } from "../lib/wasm";
import { selection } from "../state/selection.svelte";
import { volume } from "../state/volume.svelte";
import { statusToError, type ShellHost } from "./host";

/**
 * The app's ShellHost. Commands close over the returned object and read live store
 * fields through its getters on every call, so a host created once when the
 * terminal starts never goes stale.
 *
 * `run` and `format` mirror the Actions panel: VolumeStore never throws, it reports
 * failure through `status` (and `run` returns null). The host turns that status back
 * into a thrown error so the shell prints it in red while StatusLine keeps showing
 * the same code, exactly as after a failed form action.
 */
export function createStoreHost(closeTerminal: () => void): ShellHost {
  return {
    get vol(): Volume {
      return volume.vol;
    },
    get cursor(): number {
      return volume.cursor;
    },
    get historyLength(): number {
      return volume.history.length;
    },
    run(fn: (v: Volume) => OpRecord): OpRecord {
      const rec = volume.run(fn);
      if (rec) return rec;
      throw statusToError(volume.status);
    },
    format(options: FormatOptions): void {
      volume.format(options);
      // On failure VolumeStore.format sets status and leaves the disk alone. Read it
      // before select(null), whose side effect is to clear status.
      if (volume.status) throw statusToError(volume.status);
      selection.select(null);
    },
    select(path: string | null): void {
      selection.select(path);
    },
    jumpTo(offset: number): void {
      selection.jumpTo(offset);
    },
    closeTerminal,
  };
}
```

Run:

```
pnpm exec svelte-check --tsconfig ./tsconfig.json
```

Expected: 0 errors. If it reports that the object literal is missing a property or a getter's type mismatches, compare against `src/shell/host.ts` from Task 4 and match that file exactly; the interface in that file is the authority.

- [ ] **Step 14: Commit**

```
git add web/ui/src/shell/storeHost.svelte.ts
git commit -m "feat(ui): store-backed ShellHost that surfaces volume status as shell errors"
```

- [ ] **Step 15: Create `TerminalPanel.svelte`**

Create `web/ui/src/components/TerminalPanel.svelte`:

```svelte
<script lang="ts">
  import { onMount, tick } from "svelte";
  // browser-terminal renders into our element with no shadow DOM and no stylesheet of
  // its own, so the app loads xterm's CSS. (@xterm/xterm has no `exports` map; the
  // deep path resolves.)
  import "@xterm/xterm/css/xterm.css";
  // Type-only: the runtime import is the lazy `import()` in ensureCreated, so the
  // library's wasm loads only when someone opens the drawer.
  import type { BrowserTerminal } from "@benjamin-small/browser-terminal";
  import { createCommands } from "../shell/commands";
  import { createStoreHost } from "../shell/storeHost.svelte";
  import { terminal } from "../state/terminal.svelte";

  let mountEl = $state<HTMLDivElement>();
  let bt: BrowserTerminal | null = null;
  let creating: Promise<void> | null = null;

  /** Close the drawer and hand focus to the topbar button, since the element that had
   *  focus (xterm's helper textarea) is about to be hidden. `exit`, the Close button,
   *  and Escape on the bar all come through here. */
  function close() {
    terminal.close();
    document.getElementById("terminal-toggle")?.focus();
  }

  function focusShell() {
    mountEl?.querySelector<HTMLTextAreaElement>(".xterm-helper-textarea")?.focus();
  }

  /**
   * Create the terminal on first use. Called after the drawer is visible so the
   * library's ResizeObserver fits real dimensions on its first pass. One instance per
   * page is the library's rule; `bt` is the instance, `creating` the in-flight
   * promise, so a second open during the first load reuses it.
   */
  function ensureCreated(): Promise<void> {
    if (bt) return Promise.resolve();
    if (creating) return creating;
    const mount = mountEl;
    if (!mount) return Promise.resolve();
    terminal.ready = "loading";
    terminal.error = null;
    creating = (async () => {
      const { BrowserTerminal } = await import("@benjamin-small/browser-terminal");
      bt = await BrowserTerminal.create({ mount });
      // Commands read live store fields through the host on every call, so registering
      // once is enough (same pattern as browser-terminal's Svelte demo).
      for (const { spec, fn } of createCommands(createStoreHost(close))) bt.registerCommand(spec, fn);
      terminal.ready = "ready";
    })()
      .catch((e: unknown) => {
        terminal.ready = "error";
        terminal.error = e instanceof Error ? e.message : String(e);
      })
      .finally(() => {
        creating = null;
      });
    return creating;
  }

  // Opening (and every openAndFocus while open) creates on first use, then focuses the
  // shell. The library focuses the pane itself on creation; later opens need this.
  $effect(() => {
    void terminal.focusNonce;
    if (!terminal.open) return;
    tick()
      .then(ensureCreated)
      .then(() => requestAnimationFrame(focusShell));
  });

  function disposeTerminal() {
    bt?.dispose();
    bt = null;
  }
  // HMR replaces this module: dispose first or the next create() throws "one instance
  // per page". The unmount cleanup covers the non-HMR teardown. dispose() is idempotent.
  if (import.meta.hot) import.meta.hot.dispose(disposeTerminal);
  onMount(() => disposeTerminal);

  // Drag the bar to resize. The store clamps and persists; the library's
  // ResizeObserver on the mount refits the terminal as the track height changes.
  let drag: { pointerId: number; startY: number; startH: number } | null = null;
  function onBarDown(e: PointerEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    drag = { pointerId: e.pointerId, startY: e.clientY, startH: terminal.height };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    e.preventDefault();
  }
  function onBarMove(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    terminal.setHeight(drag.startH + (drag.startY - e.clientY));
  }
  function onBarUp(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    drag = null;
    (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
  }

  // Escape closes from the bar or the Close button. Inside the terminal xterm cancels
  // Escape before it bubbles, so this never fires while typing a command.
  function onKeydown(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    e.preventDefault();
    close();
  }
</script>

<!-- Re-clamp on viewport changes so a persisted height never exceeds 60% of a smaller window. -->
<svelte:window onresize={() => terminal.setHeight(terminal.height)} />

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
  id="terminal-drawer"
  class="terminal-drawer panel"
  hidden={!terminal.open}
  aria-label="Terminal"
  onkeydown={onKeydown}
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="term-bar" onpointerdown={onBarDown} onpointermove={onBarMove} onpointerup={onBarUp} onpointercancel={onBarUp}>
    <span class="term-title">Terminal <span class="muted">— /mnt is the volume, /dev/hda the raw disk. Type help.</span></span>
    <button type="button" onclick={close} aria-label="Close terminal">Close</button>
  </div>
  {#if terminal.ready === "loading"}<p class="muted term-note">Loading the shell…</p>{/if}
  {#if terminal.ready === "error"}<p class="term-note err">Could not start the terminal: {terminal.error}</p>{/if}
  <div class="terminal-mount" bind:this={mountEl}></div>
</section>
```

Notes the executor should keep in mind (all verified against browser-terminal 0.2.0 and xterm 6.0.0 sources):
- Never call `bt.show()` / `bt.hide()`: with a custom `mount` they write `mount.style.display`, which would fight the drawer's `hidden` attribute. The section's `hidden` is the only visibility switch.
- `terminal.ready`/`terminal.error` are written only inside the async chain, never synchronously inside the `$effect`, so the effect depends on `open` and `focusNonce` alone.
- `BrowserTerminal.create` throws if an instance is live; after HMR `bt` is null in the new module instance, but the old module's `import.meta.hot.dispose` and the old component's unmount cleanup have already disposed it.

- [ ] **Step 16: Wire the panel, the button, and `--term-h` into `App.svelte`**

Modify `web/ui/src/App.svelte`.

Add two imports. After the line `  import Timeline from "./components/Timeline.svelte";` and before `  import { inTextEntry } from "./core/keys";`:

Old:

```svelte
  import Timeline from "./components/Timeline.svelte";
  import { inTextEntry } from "./core/keys";
  import { volume } from "./state/volume.svelte";
```

New:

```svelte
  import TerminalPanel from "./components/TerminalPanel.svelte";
  import Timeline from "./components/Timeline.svelte";
  import { inTextEntry } from "./core/keys";
  import { terminal } from "./state/terminal.svelte";
  import { volume } from "./state/volume.svelte";
```

Add the backtick toggle to `onKeydown`:

Old:

```svelte
  function onKeydown(e: KeyboardEvent) {
    if (inTextEntry(e)) return;
    if (e.key === "[") volume.seek(Math.max(0, volume.cursor - 1));
```

New:

```svelte
  function onKeydown(e: KeyboardEvent) {
    if (inTextEntry(e)) return;
    if (e.key === "`") {
      // Toggle the terminal from anywhere outside text entry. Inside the terminal the
      // key is typed (xterm cancels it), so closing is exit / Close / Escape on the bar.
      e.preventDefault();
      terminal.toggle();
    } else if (e.key === "[") volume.seek(Math.max(0, volume.cursor - 1));
```

Set `--term-h` on `.app` (custom properties inherit downward only, and `.app`'s `grid-auto-rows` consumes it, so it must be set here, not on the drawer) and add the topbar button after the title:

Old:

```svelte
<div class="app">
  <header class="topbar">
    <h1>FAT explorer</h1>
    <div id="scenario-slot"><ScenarioPanel /></div>
```

New:

```svelte
<div class="app" style:--term-h="{terminal.height}px">
  <header class="topbar">
    <h1>FAT explorer</h1>
    <button
      id="terminal-toggle"
      type="button"
      aria-pressed={terminal.open}
      aria-controls="terminal-drawer"
      title="Toggle the terminal (`)"
      onclick={() => terminal.toggle()}
    >Terminal</button>
    <div id="scenario-slot"><ScenarioPanel /></div>
```

Mount the drawer as the grid's implicit fifth row, after the footer:

Old:

```svelte
  <footer id="timeline-slot"><Timeline /></footer>
</div>
```

New:

```svelte
  <footer id="timeline-slot"><Timeline /></footer>
  <TerminalPanel />
</div>
```

- [ ] **Step 17: Drawer layout and xterm overrides in `app.css`**

Modify `web/ui/src/app.css`.

Line 15, the `.app` rule:

Old:

```css
.app { display: grid; grid-template-rows: auto auto minmax(0, 1fr) auto; height: 100vh; gap: 8px; padding: 8px; }
```

New:

```css
/* The terminal drawer is the implicit fifth row: `grid-auto-rows` sizes it from `--term-h`
   (set on .app by App.svelte), and a hidden drawer generates no grid item, so it adds
   neither a track nor a gap. */
.app { display: grid; grid-template-rows: auto auto minmax(0, 1fr) auto; grid-auto-rows: var(--term-h, 220px); height: 100vh; gap: 8px; padding: 8px; }
```

Lines 30–33, the narrow-screen block:

Old:

```css
@media (max-width: 760px) {
  .grid { grid-template-columns: minmax(0, 1fr); }
  .left, .right { max-height: 40vh; }
}
```

New:

```css
@media (max-width: 760px) {
  .grid { grid-template-columns: minmax(0, 1fr); }
  .left, .right { max-height: 40vh; }
  .app { grid-auto-rows: min(var(--term-h, 220px), 40vh); }
}
```

Append at the end of the file (after the `.actions .using-bytes` line):

```css

/* Terminal drawer */
/* browser-terminal renders into .terminal-mount with no shadow DOM and no stylesheet of
   its own (TerminalPanel imports xterm's), and it hard-codes a dark theme: an inline
   background on each pane div, xterm's injected `.xterm-dom-renderer-owner-N` rules for
   text color and font, and an inline background on the scrollable viewport. The
   token-driven `!important` rules below win over all of those, and because the tokens
   swap under `prefers-color-scheme: dark`, the terminal follows the OS theme with no JS.
   Font: xterm measures the cell width on an offscreen canvas in its own option font and
   then letter-spaces `.xterm-rows` so every glyph fills exactly one cell, so overriding
   the rendered font (and the `.xterm-char-measure-element` it uses for glyph widths)
   keeps text, cursor, selection, and mouse position aligned. */
.terminal-drawer { display: flex; flex-direction: column; min-height: 0; padding: 0; overflow: hidden; }
.terminal-drawer[hidden] { display: none; }
.terminal-drawer:focus-within { border-color: var(--focus); }
.term-bar { display: flex; align-items: center; gap: 8px; padding: 4px 10px; border-bottom: 1px solid var(--hairline); cursor: row-resize; user-select: none; touch-action: none; font: 500 12px var(--font-display); }
.term-bar button { margin-left: auto; cursor: pointer; }
.term-note { margin: 4px 10px; font-size: 12px; }
.terminal-drawer .err { color: var(--diff-ink); }
.terminal-mount { flex: 1; min-height: 0; position: relative; }
.terminal-mount > div { background: var(--panel) !important; }
.terminal-mount .xterm,
.terminal-mount .xterm .xterm-viewport,
.terminal-mount .xterm .xterm-scrollable-element,
.terminal-mount .xterm .xterm-screen { background-color: var(--panel) !important; }
.terminal-mount .xterm .xterm-rows { color: var(--ink) !important; }
/* Cursor: 5 classes outrank xterm's injected 4-class rule without `!important`, which
   would also freeze the blink animation (important declarations beat keyframes). */
.terminal-mount .xterm .xterm-rows .xterm-cursor.xterm-cursor-block { background-color: var(--focus); color: var(--panel); }
.terminal-mount .xterm .xterm-rows .xterm-cursor.xterm-cursor-outline { outline-color: var(--focus); }
.terminal-mount .xterm .xterm-rows .xterm-cursor.xterm-cursor-bar { box-shadow: 1px 0 0 var(--focus) inset; }
.terminal-mount .xterm .xterm-rows .xterm-cursor.xterm-cursor-underline { border-bottom-color: var(--focus); }
.terminal-mount .xterm, .terminal-mount .xterm * { font-family: var(--font-mono) !important; }
.terminal-mount .xterm .xterm-rows, .terminal-mount .xterm .xterm-char-measure-element { font-size: 12px !important; }
/* app.css gives every focused textarea/[tabindex] an outline; xterm's helper textarea
   sits off-screen and the .xterm box would otherwise get a ring inside the drawer. */
.terminal-mount .xterm:focus-visible, .terminal-mount .xterm-helper-textarea:focus-visible { outline: none; }
```

- [ ] **Step 18: Type-check and build**

```
pnpm test && pnpm build
```

Expected: every test file passes (the existing suites, the shell suites from Tasks 4–6, `keys.test.ts`, `terminalHeight.test.ts`); `svelte-check` prints `0 errors`; `vite build` emits `dist/` with a separate chunk for `@benjamin-small/browser-terminal` and a `bterm_wasm_bg-*.wasm` asset (look for both names in the build output). Warnings from svelte-check about a11y on the drawer are acceptable only if a `svelte-ignore` comment above was mistyped; fix the comment rather than the markup.

If svelte-check complains about `import.meta.hot`, confirm `tsconfig.json` still lists `"vite/client"` in `types` (it does on `main`).

- [ ] **Step 19: Commit**

```
git add web/ui/src/components/TerminalPanel.svelte web/ui/src/App.svelte web/ui/src/app.css
git commit -m "feat(ui): terminal drawer with lazy browser-terminal creation, toggle button, and backtick"
```

- [ ] **Step 20: Manual verification in the built-in browser**

Start the dev server with the `ui` launch configuration (`.claude/launch.json`, `pnpm --dir web/ui dev --port 5174`) and open `http://localhost:5174/`. Work through every item; each is a pass/fail:

1. Load. The drawer is hidden and the layout is unchanged (four rows, no extra gap under the timeline). Press backtick: the drawer opens at 220px with "Loading the shell…", then a prompt appears and the caret is focused (typing shows in the terminal).
2. In the terminal type `[`, `]`, `/`, `n`, `p`: nothing happens in the app (timeline cursor unchanged, path field unfocused). Press Ctrl-B then `[`: still nothing in the app. Click the hex dump, press backtick: the drawer closes; press backtick again: it opens and the shell is focused. Click the Actions Path field and press backtick: a backtick is typed and the drawer is untouched.
3. `echo hi | write /mnt/A.TXT`: the timeline gains a `create_file /A.TXT` step, the dump jumps to amber diff bytes, the Files tree shows `A.TXT`, the ribbon adds a hue, StatusLine is clear. `cat /mnt/A.TXT` prints `hi`. `stat /mnt/A.TXT` shows an entry offset and data offset that match the Inspector when you click the file in the tree.
4. Click Prev on the timeline, then `ls /mnt`: a red warning line (`showing the latest state, not step N of M; …`) precedes the listing. Run `echo again | write /mnt/B.TXT`: the "Viewing step" banner disappears and a new step appends at the end.
5. `dd --if=/dev/hda --bs=512 --count=1 | xxd` prints sector 0 (the `EB 3C 90` jump at `00000000`). `echo 'SHELLDISK  ' | dd --of=/dev/hda --bs=1 --seek=43`: the timeline gains `write_raw 0x2b +11` and the Inspector's boot-sector annotation at offset 43 shows the new label.
6. Corruption round trip, using only raw reads and writes (they are not gated, unlike file reads, and the shell has no user variables to park a blob in). `stat /dev/hda` and note the sector count `S`. Stash sector 0 in the last sector: `dd --if=/dev/hda --bs=512 --count=1 --of=/dev/hda --seek=<S-1>` (a `write_raw` step appears). Wipe: `dd --if=/dev/zero --of=/dev/hda --count=1`. Now `ls /mnt` fails with the corruption message, `mount` reports state `corrupt`, and the hex dump still renders sector 0 as zeros. Click Prev on the timeline: the boot bytes come back in the dump while `mount` (with the rewound warning line) still says `corrupt`, because the volume is always the latest state. Click Back to now, then restore: `dd --if=/dev/hda --bs=512 --skip=<S-1> --count=1 --of=/dev/hda`. `ls /mnt` lists the files again and `mount` says `ok`.
7. Run the "Fill the disk" scenario to completion, then `echo more | write /mnt/X.TXT`: the terminal prints a red error whose text ends in `No space left on device`, and StatusLine shows the `DiskFull` code.
8. Drag the bar up and down: the terminal reflows with no clipped rows. Resize the browser window: same. Reload: the drawer remembers its height (open it to check). Toggle the OS dark mode: the terminal background/text follow the tokens with no reload. Type `exit`: the drawer closes and focus lands on the Terminal button (press Space: the drawer reopens). Edit a comment in `TerminalPanel.svelte` and save with the drawer open: HMR replaces the component, the drawer re-creates the shell, and the console shows no "one instance per page" error.
9. Selection alignment: drag-select a word in the terminal; the highlight covers exactly the glyphs (confirms the font override left the cell grid consistent). Click in the middle of a command line: the caret lands on the clicked character.
10. Base path: stop the dev server, run `VITE_BASE=/fs-emulator/ pnpm build && pnpm exec vite preview`, open `http://localhost:4173/fs-emulator/`, press backtick, and confirm in the network tab that the terminal's wasm loads from `/fs-emulator/assets/bterm_wasm_bg-*.wasm` with status 200.

Record any failing item and fix it before this task is considered done; the CSS overrides (item 8 dark mode, item 9 alignment) are the likeliest to need a selector tweak against the installed xterm build.


---

### Task 8: Shell scenario and documentation

Spec section 6 of `docs/superpowers/specs/2026-09-22-terminal-access-design.md`. Adds the eighth guided scenario, "Work from the shell", whose step text shows the terminal command and whose `action` performs the equivalent `Volume` operation (the scenario engine is unchanged), and brings every README and the roadmap in line with the terminal drawer, raw writes, and the browser-terminal follow-ups.

**Files:**

- Create: `web/ui/src/scenarios/shell.ts`
- Modify: `web/ui/src/scenarios/index.ts` (whole file: one import and one array entry)
- Test: `web/ui/tests/scenarios.test.ts` (append one `describe` block after the `for (const s of all)` loop, before the closing `});` on the last line)
- Modify: `web/ui/README.md` (line 6 "seven"; line 26 test comment; lines 71–74 scenario list; new "Terminal" section inserted between line 80 and line 82; keyboard table lines 97–106; paragraph lines 108–110)
- Modify: `README.md` (root; line 21 architecture diagram; line 28 fs-core line; line 46 status row; lines 50–58 crate bullets; lines 77–82 Web section; lines 112–116 documentation list)
- Modify: `crates/wasm/README.md` (lines 16–17 error codes; lines 21–25 generic methods; lines 27–32 FAT-only methods) — only if Task 2 left it untouched; step 7 says how to check
- Modify: `docs/ROADMAP.md` (lines 14–17 journal rule; lines 68–71 core API additions; lines 83–87 deferred `web/ui`; new "browser-terminal follow-ups" subsection after line 87)

**Interfaces:**

Consumes (all exist by the time this task runs):

- `Volume.writeRaw(offset: number, bytes: Uint8Array): OpRecord` (Task 2, `crates/wasm/pkg/fs_emulator_wasm.d.ts`; op string `write_raw 0x{offset:x} +{len}`; re-parses the boot sector when `offset < 512`).
- `Volume.corruption(): string | null` (Task 2; `null` while the boot sector parses).
- `Volume.readFile(path: string): Uint8Array`, `createFile`, `createDir`, `deleteFile`, `listDir`, `stat`, `bootSector`, `clusterOwners`, `geometry` (existing).
- `Scenario`, `Step`, `StepFocus` from `web/ui/src/state/scenarios.svelte.ts` (existing, unchanged):
  ```ts
  export interface StepFocus { offset?: number; sector?: number; cluster?: number; path?: string | null; showRemnants?: boolean; strings?: boolean; }
  export interface Step { title: string; text: string; action?: (v: Volume) => OpRecord; format?: FormatOptions; focus?: StepFocus | ((v: Volume) => StepFocus); }
  export interface Scenario { id: string; title: string; summary: string; steps: Step[]; }
  ```
- Command names and flag spellings from Tasks 5 and 6 (`ls`, `dir`, `cd`, `pwd`, `cat --bytes`, `write --append --at`, `dd --if= --of= --bs= --count= --skip= --seek=`, `xxd`/`hexdump`, `mkdir`, `rmdir`, `rm`, `touch`, `cp`, `stat`, `df`, `mount`, `seek`, `select`, `mkfs`, `exit`), `DD_MAX_BYTES = 1 << 20`, and the blob convention `{ bytes: "<hex>", length }` from Task 4. The docs quote these; nothing here imports them.
- The backtick shortcut, the `Terminal` topbar button, the drawer bar copy, Escape-on-bar, and the persisted height from Task 7. The docs describe these; nothing here imports them.

Produces:

- `web/ui/src/scenarios/shell.ts`: `export const scenario: Scenario` with `id: "shell"`, `title: "Work from the shell"`, eight steps.
- `web/ui/src/scenarios/index.ts`: `all` grows to eight entries with `shell` last.
- No new runtime API; later tasks (none) rely on nothing from here.

The scenario test drives every registered scenario against a fresh `Volume`, so the new scenario is covered by the existing loop as soon as it is registered; the added `describe` pins the label patch and the file/directory outcomes.

---

- [ ] **Step 1: Write the failing scenario test**

Append the following block to `/Users/bsmall/dev/fs-emulator/web/ui/tests/scenarios.test.ts`. Insert it immediately before the final line of the file (the closing `});` of `describe("scenario scripts", ...)`), i.e. after the `for (const s of all) { ... }` loop's closing `}`. Also add the import on line 5.

Old text (lines 1–5):

```ts
import { describe, expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";
import { ScenarioCursor } from "../src/core/scenarioCursor";
import { all } from "../src/scenarios";
```

New text:

```ts
import { describe, expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";
import { ScenarioCursor } from "../src/core/scenarioCursor";
import { all } from "../src/scenarios";
import { scenario as shell } from "../src/scenarios/shell";
```

Old text (the end of the file):

```ts
        } else {
          expect(run).not.toThrow();
        }
      }
    });
  }
});
```

New text:

```ts
        } else {
          expect(run).not.toThrow();
        }
      }
    });
  }

  // The shell scenario's step text shows a terminal command and its action performs the
  // equivalent Volume call. Pin the outcomes the commands would leave behind.
  describe("work from the shell", () => {
    it("is the eighth scenario and registered last", () => {
      expect(all.length).toBe(8);
      expect(all[all.length - 1]).toBe(shell);
      expect(shell.id).toBe("shell");
      expect(shell.steps.length).toBe(8);
      // Every step's text names the command it stands for.
      for (const step of shell.steps) expect(step.text).toMatch(/`[a-z]+[^`]*`/);
    });

    it("leaves the volume the way the equivalent commands would", () => {
      const vol = Volume.formatFat16(undefined);
      const run = (i: number) => {
        const step = shell.steps[i];
        expect(step.action, `step ${i} "${step.title}" has no action`).toBeDefined();
        return step.action!(vol);
      };
      const text = (b: Uint8Array) => new TextDecoder().decode(b);

      // echo 'Hello from the shell' | write /mnt/HELLO.TXT
      expect(run(1).op).toBe("create_file /HELLO.TXT");
      expect(text(vol.readFile("/HELLO.TXT"))).toBe("Hello from the shell");
      expect(vol.clusterOwners().find((o) => o.path === "/HELLO.TXT")?.firstCluster).toBe(2);

      // mkdir /mnt/DOCS
      expect(run(3).op).toBe("create_dir /DOCS");
      expect(vol.stat("/DOCS").isDir).toBe(true);

      // cp /mnt/HELLO.TXT /mnt/DOCS/COPY.TXT
      run(4);
      expect(vol.readFile("/DOCS/COPY.TXT")).toEqual(vol.readFile("/HELLO.TXT"));
      expect(vol.clusterOwners().find((o) => o.path === "/DOCS/COPY.TXT")?.firstCluster).toBe(4);

      // echo 'SHELLDISK  ' | dd --of=/dev/hda --bs=1 --seek=43
      // The BootSector DTO trims trailing spaces from the 11-byte label (dto.rs `text`).
      const patch = run(6);
      expect(patch.op).toBe("write_raw 0x2b +11");
      expect(patch.changes).toHaveLength(1);
      expect(patch.changes[0].offset).toBe(43);
      expect(patch.changes[0].after).toEqual(new TextEncoder().encode("SHELLDISK  "));
      expect(vol.bootSector().volumeLabel).toBe("SHELLDISK");
      expect(vol.corruption()).toBeNull();

      // rm /mnt/HELLO.TXT: the entry is marked deleted, DOCS (slot 1) is all that lists.
      expect(run(7).op).toBe("delete_file /HELLO.TXT");
      expect(vol.listDir("/").map((e) => e.name)).toEqual(["DOCS"]);
      expect(text(vol.readFile("/DOCS/COPY.TXT"))).toBe("Hello from the shell");
    });
  });
});
```

Step indices are zero-based: steps 0, 2 and 5 have no `action` (they only focus), so the test runs 1, 3, 4, 6, 7.

- [ ] **Step 2: Run the test and confirm it fails**

```
cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm test tests/scenarios.test.ts
```

Expected: the file fails to load with `Error: Failed to load url ../src/scenarios/shell` (Vite's "Cannot find module" resolution error for the missing `src/scenarios/shell.ts`); no test in the file runs.

- [ ] **Step 3: Write the scenario**

Create `/Users/bsmall/dev/fs-emulator/web/ui/src/scenarios/shell.ts`:

```ts
import type { Scenario } from "../state/scenarios.svelte";

const FILE = "/HELLO.TXT";
const DIR = "/DOCS";
const COPY = "/DOCS/COPY.TXT";
const GREETING = "Hello from the shell";

// The volume label lives at boot-sector offset 43, 11 bytes, space padded.
const LABEL_OFFSET = 43;
const LABEL = "SHELLDISK  ";

const enc = (s: string) => new TextEncoder().encode(s);

export const scenario: Scenario = {
  id: "shell",
  title: "Work from the shell",
  summary: "Do the same operations from the terminal drawer: /mnt is the volume, /dev/hda the raw disk.",
  steps: [
    {
      title: "Open the terminal",
      text: "Press ` (backtick) or click Terminal to open the drawer. Type `ls /mnt`: a fresh disk has an empty root. `pwd` prints /mnt, and `mount` and `df` describe the volume.",
      focus: { path: null },
    },
    {
      title: "Write a file from a pipe",
      text: "Type `echo 'Hello from the shell' | write /mnt/HELLO.TXT`. There is no `>` yet, so the pipe carries the text into write. The same three places change as with Add file: a directory entry, a FAT entry, and a data cluster.",
      action: (v) => v.createFile(FILE, enc(GREETING)),
      focus: { path: FILE },
    },
    {
      title: "Read it back",
      text: "`cat /mnt/HELLO.TXT` prints the text. `stat /mnt/HELLO.TXT` shows the entry offset, first cluster, chain, and data offset, and `seek c:2` moves the dump to that cluster.",
      focus: (v) => ({ cluster: v.clusterOwners().find((o) => o.path === FILE)?.firstCluster }),
    },
    {
      title: "Make a directory",
      text: "`mkdir /mnt/DOCS` allocates a cluster for the directory and writes its `.` and `..` entries there. `cd /mnt/DOCS` then `ls` shows an empty listing (the dot entries are hidden, as on a real shell); `xxd /dev/hda --offset c:3 --len 64` shows the two entries on disk.",
      action: (v) => v.createDir(DIR),
      focus: { path: DIR },
    },
    {
      title: "Copy a file",
      text: "`cp /mnt/HELLO.TXT /mnt/DOCS/COPY.TXT` reads the bytes and creates the copy inside DOCS. A new cluster is allocated for it, right after the directory's own.",
      action: (v) => v.createFile(COPY, v.readFile(FILE)),
      focus: { path: COPY },
    },
    {
      title: "Read the raw boot sector",
      text: "`dd --if=/dev/hda --bs=512 --count=1 | xxd` dumps sector 0 straight from the disk, the way a real tool would. (`dd 'if=/dev/hda' 'count=1'` works too; the = must be quoted.)",
      focus: { sector: 0 },
    },
    {
      title: "Patch the disk directly",
      text: "`echo 'SHELLDISK  ' | dd --of=/dev/hda --bs=1 --seek=43` overwrites the 11-byte volume label at boot offset 43. The raw write is journaled like any other step, so it rewinds, and the inspector's boot annotation shows the new label.",
      action: (v) => v.writeRaw(LABEL_OFFSET, enc(LABEL)),
      focus: { offset: LABEL_OFFSET },
    },
    {
      title: "Delete and look at what remains",
      text: "`rm /mnt/HELLO.TXT` marks the entry deleted and frees its chain; the bytes stay on disk. `ls -l /mnt` no longer lists it, and the copy in DOCS is untouched.",
      action: (v) => v.deleteFile(FILE),
      focus: (v) => ({ offset: v.geometry().firstRootDirSector * v.geometry().bytesPerSector, path: null, showRemnants: true }),
    },
  ],
};
```

- [ ] **Step 4: Register the scenario last**

Replace the whole of `/Users/bsmall/dev/fs-emulator/web/ui/src/scenarios/index.ts`.

Old text:

```ts
import type { Scenario } from "../state/scenarios.svelte";
import { scenario as directory } from "./directory";
import { scenario as deleteRemnants } from "./deleteRemnants";
import { scenario as fillDisk } from "./fillDisk";
import { scenario as format } from "./format";
import { scenario as longName } from "./longName";
import { scenario as overwriteGrows } from "./overwriteGrows";
import { scenario as smallFile } from "./smallFile";

export const all: Scenario[] = [format, smallFile, longName, overwriteGrows, deleteRemnants, fillDisk, directory];
```

New text:

```ts
import type { Scenario } from "../state/scenarios.svelte";
import { scenario as directory } from "./directory";
import { scenario as deleteRemnants } from "./deleteRemnants";
import { scenario as fillDisk } from "./fillDisk";
import { scenario as format } from "./format";
import { scenario as longName } from "./longName";
import { scenario as overwriteGrows } from "./overwriteGrows";
import { scenario as shell } from "./shell";
import { scenario as smallFile } from "./smallFile";

export const all: Scenario[] = [format, smallFile, longName, overwriteGrows, deleteRemnants, fillDisk, directory, shell];
```

- [ ] **Step 5: Run the tests and the build; confirm they pass**

```
cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm test && pnpm build
```

Expected: `tests/scenarios.test.ts` reports the existing suites plus `runs "Work from the shell" end to end against a fresh Volume` and the two `work from the shell` cases, all passing; every other test file passes unchanged; `svelte-check` reports 0 errors and `vite build` completes.

If `pnpm test` fails with `vol.corruption is not a function` or `vol.writeRaw is not a function`, the wasm package predates Task 2: run `wasm-pack build /Users/bsmall/dev/fs-emulator/crates/wasm --target bundler` and `cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm install`, then rerun.

- [ ] **Step 6: Commit the scenario**

```
cd /Users/bsmall/dev/fs-emulator && git add web/ui/src/scenarios/shell.ts web/ui/src/scenarios/index.ts web/ui/tests/scenarios.test.ts && git commit -m 'feat(ui): add the "Work from the shell" scenario'
```

- [ ] **Step 7: Document the terminal in `web/ui/README.md`**

Six edits, in file order.

Edit 7a, line 6. Old text:

```
byte-diff replay, and seven guided scenarios. It runs entirely in the
```

New text:

```
byte-diff replay, eight guided scenarios, and a terminal drawer that mounts
the volume at `/mnt` and the raw disk at `/dev/hda`. It runs entirely in the
```

Edit 7b, line 26. Old text:

```
pnpm test      # vitest over src/core
```

New text:

```
pnpm test      # vitest over src/core and src/shell (loads the real wasm package)
```

Edit 7c, lines 71–74 (the start of the Scenarios bullet). Old text:

```
- **Scenarios** (top bar) — guided walkthroughs: format an empty disk, add
  a small file, add a long-named file (LFN entries), overwrite with a
  larger file (chain grows), delete and see what remains, fill the disk,
  and make a directory. Starting one formats a fresh disk. Each step runs
```

New text:

```
- **Scenarios** (top bar) — guided walkthroughs: format an empty disk, add
  a small file, add a long-named file (LFN entries), overwrite with a
  larger file (chain grows), delete and see what remains, fill the disk,
  make a directory, and work from the shell (the same operations typed as
  commands, plus a raw-sector read and a raw patch of the volume label).
  Starting one formats a fresh disk. Each step runs
```

Edit 7d: insert a new section between the end of the Scenarios bullet (line 80, `  **Prev** stops there rather than rewinding past it.`) and the heading `## What is FAT-specific` (line 82). The blank line after line 80 stays; the new section is followed by one blank line before the heading. New text to insert:

````
## Terminal

The **Terminal** button in the top bar (or the backtick key, from anywhere
that is not a text field) opens a drawer along the bottom running a shell
from `@benjamin-small/browser-terminal`, pinned at 0.2.0. The volume is
mounted at `/mnt` and the raw disk is `/dev/hda`; `/dev/zero` and `/dev/null`
exist too. Every write is an ordinary journaled operation, so it lands in
the timeline, the dump, the ribbon, and the tree exactly like a form action,
and it rewinds the same way. The drawer bar can be dragged to resize
(120px to 60% of the window; the height persists), `Escape` on the bar,
the Close button, or `exit` close it, and `help` or `<command> --help`
describe every command.

| Command | Does |
|---|---|
| `ls [path] [-l]` / `dir` | List a directory as a table of name, type, size (`-l` adds the modified time); entries keep their on-disk order. `ls /` shows `dev` and `mnt`; `ls /dev` shows `hda`, `zero`, `null` |
| `cd [path]`, `pwd` | Change or print the working directory; `.` and `..` resolve client-side, `cd` alone returns to `/mnt`, and the stored path takes the on-disk case |
| `cat <path> [--bytes]` | Print a file as UTF-8 text, or as a blob for pipes; refuses text over 1 MiB and warns on binary content |
| `write <path> [--append] [--at <addr>]` | Write the piped input, creating or overwriting the file (`echo hi \| write /mnt/A.TXT`). `--append` reads, concatenates, and rewrites; `write /dev/hda --at <addr>` patches the disk |
| `dd --if=<src> --of=<dst> --bs=N --count=N --skip=N --seek=N` | Copy bytes between files and the raw disk; quoted `'if=/dev/hda'` operands also work; at most 1 MiB per invocation; `/dev/zero` needs `--count` |
| `xxd [path] [--offset --len --cols]` / `hexdump` | Hex dump of a path, a piped blob, or piped text; on `/dev/hda` one sector at absolute addresses that match the dump |
| `mkdir`, `rmdir`, `rm`, `touch`, `cp` | The usual; `cp` into an existing directory keeps the source name |
| `stat <path>` | Name, type, size, timestamps, first cluster, chain, entry offset, FAT entry offset, data offset; `stat /dev/hda` reports the sector size and count |
| `df`, `mount` | Cluster usage; device, mount point, type, and `ok` or `corrupt` |
| `seek <addr>` | Move the hex dump (`0x200`, `512`, `s:1`, `c:2`) |
| `select [path]` | Select a file in every pane, or clear the selection |
| `mkfs [--sectors --spc --label --root-entries --fats --reserved]` | Format a fresh disk; the timeline is cleared |
| `exit` | Close the drawer |

The "Work from the shell" scenario walks through these commands:

```
ls /mnt
echo 'Hello from the shell' | write /mnt/HELLO.TXT
cat /mnt/HELLO.TXT
stat /mnt/HELLO.TXT
seek c:2
mkdir /mnt/DOCS
cd /mnt/DOCS
cp /mnt/HELLO.TXT /mnt/DOCS/COPY.TXT
dd --if=/dev/hda --bs=512 --count=1 | xxd
echo 'SHELLDISK  ' | dd --of=/dev/hda --bs=1 --seek=43
rm /mnt/HELLO.TXT
```

And the one it leaves out, which corrupts the disk on purpose:

```
dd --if=/dev/zero --of=/dev/hda --count=1   # wipe sector 0
ls /mnt                                      # fails: boot sector no longer parses after a raw write
mount                                        # state: corrupt
xxd /dev/hda                                 # still works: the dump reads the disk, not the filesystem
```

Once sector 0 is gone, every path operation (`ls`, `cat`, `stat`, `write`,
...) fails with `CorruptImage` while `layout`, the dump, the ribbon, `xxd
/dev/hda`, and `dd` keep working. The wipe is one journaled step, so **Prev**
on the timeline shows the disk as it was; writing a parsable boot sector back
to `/dev/hda` (or `mkfs`) clears the corruption.

Things to know:

- There is no `>`, `>>`, or `<` yet: the shell reserves them. Pipe into
  `write` or `dd --of=` instead.
- `=` is not a bareword character, so write `dd --if=/dev/hda` (the
  documented form) or quote the classic spelling: `dd 'if=/dev/hda'`.
- Strings cross pipes as UTF-8 text. Binary data crosses as a blob record
  `{ bytes: "<hex>", length }`; `cat --bytes`, `dd`, `xxd`, and `write` all
  speak it. `echo a b` produces a list, which `write` joins with one space.
- While the timeline is rewound, reads show the latest state and the shell
  warns on each one; any write snaps the timeline back to now first.
- `dd` and `cat` refuse more than 1 MiB per invocation: every byte is
  journaled twice in Rust and again in the UI's history.
- The working directory is one value per page, not per shell session.
- `echo` is the shell's own builtin and behaves as in browser-terminal.
- The terminal and its wasm load on first open, so the initial page load is
  unchanged.
````

Edit 7e, keyboard table (lines 97–106 before the insert). Old text:

```
| `g` | Jump to an offset, sector, or cluster (dump focused) |
| `s` | Toggle string highlighting (dump focused) |
```

New text:

```
| `g` | Jump to an offset, sector, or cluster (dump focused) |
| `s` | Toggle string highlighting (dump focused) |
| `` ` `` | Toggle the terminal drawer |
```

Edit 7f, the paragraph after the table (lines 108–110 before the insert). Old text:

```
All shortcuts except the dump's own (which need the dump focused) work from
anywhere that isn't a text field, so typing a path or file content in
Actions is never hijacked.
```

New text:

```
All shortcuts except the dump's own (which need the dump focused) work from
anywhere that isn't a text field, so typing a path or file content in
Actions is never hijacked. The terminal counts as a text field: keys typed
into it, including `[`, `]`, `/`, `n`, `p`, and the backtick, never reach the
app's shortcuts, so close the drawer with `exit`, the Close button, or
`Escape` on its bar.
```

- [ ] **Step 8: Update the root `README.md`**

Six edits, in file order.

Edit 8a, line 21. Old text:

```
web/ui  (Svelte)         the explorer: hex dump, ribbon, timeline, scenarios
```

New text:

```
web/ui  (Svelte)         the explorer: hex dump, ribbon, timeline, scenarios, terminal
```

Edit 8b, line 28. Old text:

```
crates/fs-core           Disk, byte journal, regions, annotations, FileSystem trait
```

New text:

```
crates/fs-core           Disk, byte journal (incl. raw writes), regions, annotations, FileSystem trait
```

Edit 8c, line 46 (the `web/ui` status row). Old text:

```
| `web/ui` | Complete for FAT16; the dump, ribbon, timeline, and strings are region-driven and carry over |
```

New text:

```
| `web/ui` | Complete for FAT16; the dump, ribbon, timeline, strings, and the terminal drawer (`/mnt`, `/dev/hda`) are region-driven and carry over |
```

Edit 8d, lines 50–53 (the fs-core crate bullet). Old text:

```
- `fs-core`: filesystem-agnostic core. The `Disk`, the change journal
  (`OpRecord`, `ByteChange`, `Event`), `Region` and `Annotation`, shared types,
  and the `FileSystem` trait. Zero external dependencies, no I/O or clock, so
  it compiles unchanged for `wasm32-unknown-unknown`.
```

New text:

```
- `fs-core`: filesystem-agnostic core. The `Disk`, the change journal
  (`OpRecord`, `ByteChange`, `Event`), `Region` and `Annotation`, shared types,
  and the `FileSystem` trait, including `write_raw` for journaled writes to
  arbitrary disk offsets. Zero external dependencies, no I/O or clock, so
  it compiles unchanged for `wasm32-unknown-unknown`.
```

Edit 8e, lines 77–82 (the Web section's first paragraph). Old text:

```
`web/ui` is the explorer: a whole-disk hex dump with an ASCII gutter and
strings overlay, a disk ribbon and FAT cluster map showing where files land, an
operation timeline that rewinds the disk byte for byte, and guided scenarios.
The build from `main` is published to GitHub Pages by
`.github/workflows/pages.yml`. See `web/ui/README.md` for panes, shortcuts,
and what in it is FAT-specific.
```

New text:

```
`web/ui` is the explorer: a whole-disk hex dump with an ASCII gutter and
strings overlay, a disk ribbon and FAT cluster map showing where files land, an
operation timeline that rewinds the disk byte for byte, guided scenarios, and a
terminal drawer (the user's `@benjamin-small/browser-terminal`) with the volume
at `/mnt` and the raw disk at `/dev/hda`, so `ls`, `cat`, `write`, `dd`, and
`xxd` drive the same journal as the forms. The build from `main` is published
to GitHub Pages by `.github/workflows/pages.yml`. See `web/ui/README.md` for
panes, the command table, shortcuts, and what in it is FAT-specific.
```

Edit 8f, lines 112–116 (the specs list). Old text:

```
- `docs/superpowers/specs/`: the design documents, one per feature. They are
  the binding description of how each piece works:
  `2026-09-21-fat16-emulator-design.md` (core and FAT16, including the FAT32
  seams), `2026-09-21-wasm-wrapper-design.md`, and
  `2026-09-21-fat-explorer-ui-design.md`.
```

New text:

```
- `docs/superpowers/specs/`: the design documents, one per feature. They are
  the binding description of how each piece works:
  `2026-09-21-fat16-emulator-design.md` (core and FAT16, including the FAT32
  seams), `2026-09-21-wasm-wrapper-design.md`,
  `2026-09-21-fat-explorer-ui-design.md`, and
  `2026-09-22-terminal-access-design.md` (raw writes and the terminal drawer).
```

- [ ] **Step 9: Update `crates/wasm/README.md` if Task 2 did not**

Check first:

```
grep -n "writeRaw\|readRaw\|corruption" /Users/bsmall/dev/fs-emulator/crates/wasm/README.md
```

If all three names are already present, skip to Step 10. Otherwise make these three edits.

Edit 9a, lines 16–17. Old text:

```
Errors are `Error` objects with a `code` property (`NotFound`, `DiskFull`,
`CorruptImage`, ..., plus `NotFat` and `BadArgument`).
```

New text:

```
Errors are `Error` objects with a `code` property (`NotFound`, `DiskFull`,
`CorruptImage`, `OutOfBounds`, ..., plus `NotFat` and `BadArgument`).
```

Edit 9b, lines 21–25. Old text:

```
`Volume` wraps the `FileSystem` trait from `fs-core`, so `createFile`,
`writeFile`, `readFile`, `deleteFile`, `createDir`, `removeDir`, `listDir`,
`stat`, `setNow`, `layout`, `annotateSector`, `historyLength`, `historyAt`,
`sectorSize`, `sectorCount`, `sector`, and `image` work on every filesystem
the crate will ever hold. `fsType()` names the one inside (`"FAT16"` today).
```

New text:

```
`Volume` wraps the `FileSystem` trait from `fs-core`, so `createFile`,
`writeFile`, `readFile`, `deleteFile`, `createDir`, `removeDir`, `listDir`,
`stat`, `setNow`, `layout`, `annotateSector`, `historyLength`, `historyAt`,
`sectorSize`, `sectorCount`, `sector`, `image`, `readRaw(offset, len)`, and
`writeRaw(offset, bytes)` work on every filesystem the crate will ever hold.
`fsType()` names the one inside (`"FAT16"` today).

`writeRaw` is a journaled operation like any other: it returns an `OpRecord`
with one `ByteChange` and one `raw_write` event, rewinds through the history,
and throws `OutOfBounds` (writing nothing) when the range runs past the disk.
`readRaw` throws `BadArgument` past the end. A raw write that touches the
first 512 bytes makes the FAT volume re-parse its boot sector: a valid sector
that fits the disk is adopted (so `layout()` and `geometry()` follow it), and
an invalid one leaves the last good geometry in place while every path-based
method (`listDir`, `stat`, `readFile`, `createFile`, ...) throws
`CorruptImage` until the sector parses again. `layout`, `annotateSector`,
`readRaw`, `writeRaw`, and the history keep working throughout.
```

Edit 9c, lines 27–28 of the original. Old text:

```
`bootSector`, `geometry`, `fatEntries`, `clusterChain`, `rawDirEntries`,
`clusterOwners`, and `annotateSectorWith` are FAT-only and throw an error
```

New text:

```
`bootSector`, `geometry`, `fatEntries`, `clusterChain`, `rawDirEntries`,
`clusterOwners`, `annotateSectorWith`, and `corruption()` (the `CorruptImage`
message while the boot sector does not parse, else `null`) are FAT-only and throw an error
```

- [ ] **Step 10: Update `docs/ROADMAP.md`**

Four edits, in file order.

Edit 10a, lines 14–17 (the journal rule). Old text:

```
- **Every operation journals its bytes.** An `OpRecord` carries each changed
  byte's offset, before value, and after value, plus plain-language events.
  The UI's timeline, diff replay, and rewind are built only on that journal,
  so a new filesystem gets them for free.
```

New text:

```
- **Every operation journals its bytes.** An `OpRecord` carries each changed
  byte's offset, before value, and after value, plus plain-language events.
  This includes raw writes (`FileSystem::write_raw`), which journal like any
  other operation and are never capped in Rust; a filesystem re-parses its
  on-disk metadata when a raw write touches it and reports `CorruptImage`
  from path operations until that metadata parses again. The UI's timeline,
  diff replay, rewind, and terminal are built only on that journal, so a new
  filesystem gets them for free.
```

Edit 10b, lines 68–71 (Core API additions). Old text:

```
## Core API additions

Out of scope so far, in rough order: rename, append, truncate to size, and
undo derived from `ByteChange.before` (the journal already stores it).
```

New text:

```
## Core API additions

Out of scope so far, in rough order: rename (the shell's `mv` waits on it),
append and truncate to size (the shell's `write --append` rewrites the whole
file and says so), and undo derived from `ByteChange.before` (the journal
already stores it).
```

Edit 10c, lines 83–87 (the deferred `web/ui` paragraph). Old text:

```
**`web/ui`**: the right column should become tabs under 1100px; history
memory is uncapped; the FAT map chain has no arrowheads; `[` and `]` are not
scenario-aware; canvas captions are not live regions; the `prompt()` used for
jump-to-offset should be guarded in browsers that block it; the ribbon has no
minimum region width, so tiny regions can vanish at narrow widths.
```

New text:

```
**`web/ui`**: the right column should become tabs under 1100px; history
memory is uncapped; the FAT map chain has no arrowheads; `[` and `]` are not
scenario-aware; canvas captions are not live regions; the `prompt()` used for
jump-to-offset should be guarded in browsers that block it; the ribbon has no
minimum region width, so tiny regions can vanish at narrow widths.

**`web/ui` terminal** (decided 2026-09-22): no `>`, `>>`, or `<` redirection,
only pipes into `write` and `dd --of=`; the working directory is one value per
page, not per shell session (commands cannot learn their session from `ctx`);
reads while the timeline is rewound show the latest state and warn (an
`--at-step` flag reading the cached image is the follow-up); `dd` and `cat`
are capped at 1 MiB per invocation in the shell, not in Rust; no `mv`,
true append, or truncate until the core has them; the terminal's colors and
font are applied through `!important` overrides on xterm's DOM because
`CreateOptions` has no theme option; the library and its wasm load lazily on
first open; `Escape` closes the drawer only from its bar, since xterm cancels
the key inside the terminal; no tab completion of `/mnt` paths.
```

Edit 10d: insert a new subsection immediately after the paragraph added in 10c and before the heading `## Adding a filesystem: checklist`. New text (one blank line before and after):

```
### browser-terminal follow-ups

Changes in `@benjamin-small/browser-terminal` that would let the explorer's
shell drop its workarounds, in order of value:

1. `>`, `>>`, and `<` redirection with a host-pluggable file hook (the parser
   already lexes them and rejects them as reserved); the explorer would
   register its `/mnt` and `/dev` resolver and the same commands gain real
   redirection.
2. `key=value` barewords, so `dd if=/dev/hda count=1` lexes without quotes.
3. A bytes `Value`, replacing the `{ bytes: "<hex>", length }` blob record.
4. A session or pane id on `ctx`, so each shell can keep its own working
   directory.
5. `CreateOptions.terminal` (theme, font family, font size), replacing the
   `!important` CSS overrides.
6. A public `focus()`, replacing the `.xterm-helper-textarea` query.

Until then the explorer pins the package exactly (0.2.0) so none of these
workarounds break on a minor release.
```

- [ ] **Step 11: Check the docs render and nothing else changed**

```
cd /Users/bsmall/dev/fs-emulator && git status --short && git diff --stat && grep -n "seven" web/ui/README.md README.md docs/ROADMAP.md; echo "exit=$?"
```

Expected: only `README.md`, `web/ui/README.md`, `docs/ROADMAP.md`, and (if Step 9 applied) `crates/wasm/README.md` are modified; the `grep` prints nothing and `exit=1`. Then confirm the web build still passes (the READMEs are not compiled, but this is the last gate before the commit):

```
cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm test && pnpm build
```

Expected: all tests pass; `svelte-check` reports 0 errors.

- [ ] **Step 12: Commit the documentation**

```
cd /Users/bsmall/dev/fs-emulator && git add README.md web/ui/README.md docs/ROADMAP.md crates/wasm/README.md && git commit -m "docs: describe the terminal drawer, raw writes, and the browser-terminal follow-ups"
```

(`git add crates/wasm/README.md` is a no-op if Step 9 was skipped.)


---

