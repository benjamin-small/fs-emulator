# ext3 Journal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a JBD2 version-2 journal on inode 8 to `crates/ext`, so a volume can be formatted as ext3 in ordered or full-data mode, every path mutation runs as one journal transaction whose byte record shows the real write order, a crash can be armed at a chosen phase of the next transaction, and a `recover` operation replays or discards what the journal holds exactly as `e2fsck -fy` does; with the wasm `Volume` exposing `formatExt3`, the crash and recovery methods, and the journal inspection DTOs the explorer will use in slice 4.

**Architecture:** The journal lives in `crates/ext/src/journal/` (`mod.rs` primitives, `state.rs` format and load, `txn.rs` the post-pass transaction builder, `recovery.rs` scan and replay, `inspect.rs` classification and annotations). `ExtFs` gains `journal: Option<JournalState>`; on an ext3 volume `run_mutation` runs the unchanged ext2 body under a draft operation, classifies the changed blocks (file data versus everything else) from `block_owners`, snapshots after-images, restores the disk, and re-emits the honest sequence as the recorded operation: needs-recovery flag, data home (ordered), journal superblock, descriptor, copies, commit, checkpoint, journal emptied, flag cleared. A crash is that emission stopped at the armed phase, recorded as a successful truncated operation; `recover` is jbd2's scan and replay. Every rest state is a cleanly unmounted volume, which keeps e2fsck clean after every operation, and the recovery oracle is byte-equality with e2fsck's own replay. ext2 volumes and every slice-2 test are unchanged; the explorer keeps refusing ext volumes until slice 4.

**Tech Stack:** Rust 1.87 workspace (fs-core, fat, ext, wasm via wasm-bindgen + serde-wasm-bindgen), e2fsprogs 1.47.4 (`e2fsck`, `dumpe2fs`, `debugfs`) in tests locally and in CI, wasm-pack, the existing web/ui gates unchanged.

**Spec:** `docs/superpowers/specs/2026-09-24-ext3-journal-design.md` (binding), with `docs/superpowers/specs/2026-09-23-ext2-design.md` binding for everything it covers. The shared interface contract the tasks were written against is reproduced in each task's Interfaces block.

## Global Constraints

1. `crates/fs-core`, `crates/fat`, and `crates/ext` keep zero external dependencies and no I/O, clock, or threads in library code; tests may write temp files and spawn e2fsprogs. Everything builds for `wasm32-unknown-unknown`.
2. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass at every commit, including `items_after_test_module` and `dead_code` (no stub or helper may be left unused at a commit; public re-exports count as use).
3. FAT behaviour and ext2 behaviour are unchanged: every existing FAT, ext2, wasm, and web test passes with no expectation edited. ext2 images stay byte-identical (the new superblock fields encode as zero).
4. Names, signatures, file paths, constants, event kinds, Display strings, error messages, and op strings are the spec's and the contract's, verbatim. `crates/ext/src/journal/` is a directory module; each submodule belongs to the task the plan names.
5. On-disk bytes follow spec section 2 exactly (JBD2 offsets, big-endian, 122 tags per descriptor, the uuid after the first tag, `ESCAPE` rules, the commit header, the ext superblock journal fields in every copy, inode 8's shape). Between operations the image is a cleanly unmounted volume: journal `s_start == 0`, `needs_recovery` clear.
6. The transaction sequence of spec section 5.2 is emitted in that order with the events named there; a crash is a successful operation with a truncated record and a name suffix, never an error; `recover` follows spec section 6 including the `expected + 1` sequence rule.
7. e2fsprogs tests skip with a printed notice where the tools are missing and fail when `CI` is set; the recovery oracle compares against `e2fsck -fy`'s replay byte for byte outside the superblock copies, and any other difference is a finding, not an exclusion.
8. Gates by task: Tasks 1 to 6 keep the Rust gates green (fmt, clippy, `cargo test --workspace`, wasm32 build); Task 7 adds `wasm-pack test --node crates/wasm`, `wasm-pack build crates/wasm --target bundler`, and the unchanged `web/ui` gates (`pnpm install --frozen-lockfile && pnpm test && pnpm build`).
9. One conventional commit per task, no attribution lines. Specs are binding: where task text and spec differ, the spec wins.
10. Reading the tasks: line ranges in a task's **Files** block refer to that task's own commit on its writer's branch; later tasks shift them, so use the anchor text, not the numbers. Test counts and "filtered out" figures in **Expected** lines were observed on the task's base branch; when Tasks 5 and 6 run in sequence the second sees the first's tests too, so totals are larger than printed and only the named tests' results matter. Task 7's counts were observed after the two fix rounds that followed the critique of Tasks 3 to 6.

---

---

### Task 1: fs-core seams and superblock fields

Spec: sections 1 (the `fs-core` additions and the seven `Superblock` fields), 2 (the ext superblock fields paragraph), 9 (nothing in `web/ui` yet), 12.

**Spec:** sections 1 (the fs-core additions and the `Superblock` field list), 2 (the "ext superblock fields for ext3" paragraph: the offsets this task decodes and encodes; the values are Task 3's), 9 (the web layer sees only the new `"journal"` kind and `"NeedsRecovery"` code; the TypeScript unions are Task 7's), and 12 (`BlockRole` gains `Journal`; `Superblock` gains the section-1 fields and ext2 images are unchanged).

No journal behaviour. This task adds the error, the region kind, the block role, and the seven superblock fields; for ext2 all of them are zero, so every existing image and test stays byte-identical. Each new enum variant breaks one exhaustive `match` somewhere in the workspace, and this task adds the one arm each needs to keep compiling: `code_of` in `crates/wasm/src/error.rs`, `region_kind_name` in `crates/wasm/src/dto.rs`, and `annotate_owned_block` in `crates/ext/src/fs.rs` (where `Journal` returns `None` like `Data`; Task 6 gives journal blocks their annotations through `annotate_sector`). A workspace grep for `RegionKind::`, `Error::`, and `BlockRole::` finds no other exhaustive match: `crates/fat` only constructs or `matches!` single variants, and the `web/ui` code lists are Task 7's. Everything below was compiled and run on `feat/ext3-journal` at a928aa9 (fmt, workspace clippy, `cargo test --workspace` including the e2fsprogs oracle tests, the wasm32 build, and `wasm-pack test --node crates/wasm` all green), so copy the code verbatim.

**Files:**
- Modify: `crates/fs-core/src/error.rs` (variant at lines 24-27, Display arm at line 56, test assert at line 83)
- Modify: `crates/fs-core/src/layout.rs` (variant at lines 13-14)
- Modify: `crates/wasm/src/error.rs` (`code_of` arm at line 23; test lists at lines 71 and 107, assert at line 77)
- Modify: `crates/wasm/src/dto.rs` (`region_kind_name` arm at line 153, test assert at line 652)
- Modify: `crates/ext/src/fs.rs` (`BlockRole::Journal` at lines 39-40; `annotate_owned_block` arm at line 1615)
- Modify: `crates/ext/src/superblock.rs` (fields at lines 149-165, `decode` at lines 195-197 and 232-238, `encode` doc at lines 242-245 and body at lines 282-290, `new` at lines 335-341)
- Test: `crates/ext/src/superblock.rs` (`new_fills_the_default_disk_fields` asserts at lines 508-511, the comment in `encode_writes_the_spec_offsets_and_decode_round_trips` at lines 536-537, the new `journal_fields_encode_at_their_offsets_and_round_trip` at lines 544-573); `crates/fs-core/src/error.rs` line 83; `crates/wasm/src/error.rs` lines 71, 77, 107; `crates/wasm/src/dto.rs` line 652

**Interfaces:**
- Consumes (existing): `fs_core::Error` (`crates/fs-core/src/error.rs`), `fs_core::RegionKind` (`crates/fs-core/src/layout.rs`), `fs_emulator_wasm`'s `code_of(&fs_core::Error) -> &'static str` and `region_kind_name(RegionKind) -> &'static str`, `ext::BlockRole`, `ext::Superblock { decode, encode, new }` with the private helpers `u32_at(b: &[u8], i: usize) -> u32` and `put_u32(b: &mut [u8], i: usize, v: u32)` in `superblock.rs`.
- Produces (the contract's names, verbatim):
  - `fs_core::Error::NeedsRecovery`, Display `"needs recovery"`.
  - `fs_core::RegionKind::Journal`.
  - wasm: `code_of(&Error::NeedsRecovery) == "NeedsRecovery"`; `region_kind_name(RegionKind::Journal) == "journal"`.
  - `ext::BlockRole::Journal` (the variant only; Task 3 puts it into `block_owners`).
  - `ext::Superblock` gains `pub journal_uuid: [u8; 16]` (0xD0), `pub journal_inum: u32` (0xE0), `pub journal_dev: u32` (0xE4), `pub last_orphan: u32` (0xE8), `pub default_mount_opts: u32` (0x100), `pub jnl_backup_type: u8` (0xFD), `pub jnl_blocks: [u32; 17]` (0x10C..0x150, little-endian like every ext2 field), decoded and encoded at those offsets; `Superblock::new` sets all of them to zero. `encode` still zeroes every other byte of 136..1024.

Gate commands used throughout (run from the repository root):

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p fs-emulator-wasm --target wasm32-unknown-unknown
```

---

- [ ] **Step 1: Write the failing test for `Error::NeedsRecovery`**

In `crates/fs-core/src/error.rs`, in `mod tests`, `fn display_is_human_readable`, add one line after the `OutOfBounds` assertion so the end of the test reads:

```rust
            "out of bounds: 2 bytes at offset 3 run past the end of the 4-byte disk"
        );
        assert_eq!(Error::NeedsRecovery.to_string(), "needs recovery");
    }
```

Run: `cargo test -p fs-core --lib error`
Expected: the build fails with ``error[E0599]: no variant or associated item named `NeedsRecovery` found for enum `Error` in the current scope``.

- [ ] **Step 2: Add the variant and its Display text**

In `crates/fs-core/src/error.rs`, the enum ends with `OutOfBounds`. Replace:

```rust
        disk_len: u64,
    },
}
```

with:

```rust
        disk_len: u64,
    },
    /// An ext3 volume whose journal holds transactions not yet replayed:
    /// every mutation is refused until `recover` runs.
    NeedsRecovery,
}
```

and add the Display arm after the `OutOfBounds` arm, so the end of the `match` reads:

```rust
            } => write!(
                f,
                "out of bounds: {len} bytes at offset {offset} run past the end of the {disk_len}-byte disk"
            ),
            Error::NeedsRecovery => write!(f, "needs recovery"),
        }
```

Run: `cargo test -p fs-core --lib`
Expected: `test result: ok. 34 passed; 0 failed`.

Run: `cargo check -p fs-emulator-wasm`
Expected: the build fails with ``error[E0004]: non-exhaustive patterns: `&fs_core::Error::NeedsRecovery` not covered`` in `crates/wasm/src/error.rs` (fixed in Step 3).

- [ ] **Step 3: Give `NeedsRecovery` its wasm error code, with tests**

In `crates/wasm/src/error.rs`, `mod tests`: both `every_variant_has_its_own_code` and `wrapper_codes_are_distinct_from_every_core_code` build an array of every core variant ending in `OutOfBounds { .. }`. In both arrays, append `Error::NeedsRecovery` so each ends:

```rust
            Error::OutOfBounds {
                offset: 1,
                len: 2,
                disk_len: 3,
            },
            Error::NeedsRecovery,
        ];
```

and in `every_variant_has_its_own_code`, after `assert_eq!(codes[13], "OutOfBounds");`, add:

```rust
        assert_eq!(codes[14], "NeedsRecovery");
```

Then in `code_of`, add the arm after `OutOfBounds`, so the `match` ends:

```rust
        OutOfBounds { .. } => "OutOfBounds",
        NeedsRecovery => "NeedsRecovery",
    }
```

Run: `cargo test -p fs-emulator-wasm --lib error`
Expected: `test result: ok. 3 passed; 0 failed` (the filter matches the two `error::tests` and one other test whose name contains `error`; `every_variant_has_its_own_code` now checks 15 distinct codes).

- [ ] **Step 4: Write the failing test for `RegionKind::Journal`**

In `crates/wasm/src/dto.rs`, `mod tests`, after `assert_eq!(region_kind_name(RegionKind::Other), "other");`, add:

```rust
        assert_eq!(region_kind_name(RegionKind::Journal), "journal");
```

Run: `cargo test -p fs-emulator-wasm --lib dto`
Expected: the build fails with ``error[E0599]: no variant or associated item named `Journal` found for enum `RegionKind` in the current scope``.

- [ ] **Step 5: Add `RegionKind::Journal` and its wasm name**

In `crates/fs-core/src/layout.rs`, replace:

```rust
    Data,
    Reserved,
```

with:

```rust
    Data,
    /// An ext3 journal's blocks.
    Journal,
    Reserved,
```

In `crates/wasm/src/dto.rs`, `region_kind_name`, add the arm after `Data`:

```rust
        RegionKind::Data => "data",
        RegionKind::Journal => "journal",
```

Run: `cargo test -p fs-emulator-wasm --lib`
Expected: `test result: ok. 20 passed; 0 failed`.

- [ ] **Step 6: Add `BlockRole::Journal`**

In `crates/ext/src/fs.rs`, replace:

```rust
pub enum BlockRole {
    Data,
    Directory,
    Indirect,
}
```

with:

```rust
pub enum BlockRole {
    Data,
    Directory,
    Indirect,
    /// A data block of the ext3 journal (inode 8).
    Journal,
}
```

Run: `cargo check -p ext`
Expected: the build fails with ``error[E0004]: non-exhaustive patterns: `BlockRole::Journal` not covered`` in `annotate_owned_block`.

In `annotate_owned_block`, replace:

```rust
            BlockRole::Data => None,
```

with:

```rust
            BlockRole::Data | BlockRole::Journal => None,
```

so the `match` reads:

```rust
        match owner.role {
            BlockRole::Data | BlockRole::Journal => None,
            BlockRole::Directory => Some(annotate_dir_block(self.disk.read(start, BS))),
            BlockRole::Indirect => Some(annotate_pointer_block(&blockmap::read_pointers(
                &self.disk, block,
            ))),
        }
```

Run: `cargo check -p ext`
Expected: `Finished` with no errors or warnings. (`BlockRole` is re-exported from `lib.rs`, so the unused variant raises no `dead_code` warning.)

- [ ] **Step 7: Write the failing superblock tests**

In `crates/ext/src/superblock.rs`, `mod tests`:

(a) In `new_fills_the_default_disk_fields`, after `assert_eq!(sb.label(), "");`, add:

```rust
        assert_eq!(sb.journal_uuid, [0; 16]);
        assert_eq!((sb.journal_inum, sb.journal_dev, sb.last_orphan), (0, 0, 0));
        assert_eq!((sb.default_mount_opts, sb.jnl_backup_type), (0, 0));
        assert_eq!(sb.jnl_blocks, [0; 17]);
```

(b) In `encode_writes_the_spec_offsets_and_decode_round_trips`, the existing all-zero check now also pins the journal fields of the ext2 superblock. Replace:

```rust
        assert_eq!(&bytes[120..125], b"hello");
        assert!(bytes[136..].iter().all(|&b| b == 0));
```

with:

```rust
        assert_eq!(&bytes[120..125], b"hello");
        // ext2 has no journal: every byte past the volume name is zero,
        // the journal fields at 0xD0..0x150 included.
        assert!(bytes[136..].iter().all(|&b| b == 0));
```

(c) Add a new test directly after `encode_writes_the_spec_offsets_and_decode_round_trips` (before `validate_reports_each_corrupt_image_reason`):

```rust
    #[test]
    fn journal_fields_encode_at_their_offsets_and_round_trip() {
        let mut sb = default_superblock();
        sb.journal_uuid = std::array::from_fn(|i| 0x10 + i as u8);
        sb.journal_inum = 8;
        sb.journal_dev = 0x0102_0304;
        sb.last_orphan = 0x0506_0708;
        sb.default_mount_opts = 0x40;
        sb.jnl_backup_type = 1;
        sb.jnl_blocks = std::array::from_fn(|i| 0x100 + i as u32);
        sb.jnl_blocks[16] = 0x10_0000;
        let mut bytes = [0xAAu8; 1024];
        sb.encode(&mut bytes);
        assert_eq!(&bytes[0xD0..0xE0], &sb.journal_uuid);
        assert_eq!((bytes[0xD0], bytes[0xDF]), (0x10, 0x1F));
        assert_eq!(&bytes[0xE0..0xE4], &8u32.to_le_bytes());
        assert_eq!(&bytes[0xE4..0xE8], &0x0102_0304u32.to_le_bytes());
        assert_eq!(&bytes[0xE8..0xEC], &0x0506_0708u32.to_le_bytes());
        assert_eq!(bytes[0xFD], 1);
        assert_eq!(&bytes[0x100..0x104], &0x40u32.to_le_bytes());
        assert_eq!(&bytes[0x10C..0x110], &0x100u32.to_le_bytes());
        assert_eq!(&bytes[0x148..0x14C], &0x10Fu32.to_le_bytes());
        assert_eq!(&bytes[0x14C..0x150], &0x10_0000u32.to_le_bytes());
        // Every other byte past the volume name stays zero.
        let journal = [0xD0..0xEC, 0xFD..0xFE, 0x100..0x104, 0x10C..0x150];
        assert!((136..1024)
            .filter(|i| !journal.iter().any(|r| r.contains(i)))
            .all(|i| bytes[i] == 0));
        assert_eq!(Superblock::decode(&bytes), sb);
    }
```

Run: `cargo test -p ext --lib superblock`
Expected: the build fails with ``error[E0609]: no field `journal_uuid` on type `superblock::Superblock` `` and the same for `journal_inum`, `journal_dev`, `last_orphan`, `default_mount_opts`, `jnl_backup_type`, `jnl_blocks` (16 errors).

- [ ] **Step 8: Add the seven fields to `Superblock`**

In `crates/ext/src/superblock.rs`:

(a) The struct ends with `volume_name`. Replace:

```rust
    pub uuid: [u8; 16],
    pub volume_name: [u8; 16],
}
```

with:

```rust
    pub uuid: [u8; 16],
    pub volume_name: [u8; 16],
    /// `s_journal_uuid` (0xD0): an external journal's uuid; zero for an
    /// internal journal and for ext2.
    pub journal_uuid: [u8; 16],
    /// `s_journal_inum` (0xE0): the journal inode, 8 on ext3, 0 on ext2.
    pub journal_inum: u32,
    /// `s_journal_dev` (0xE4): an external journal's device; always 0 here.
    pub journal_dev: u32,
    /// `s_last_orphan` (0xE8): head of the orphan list; always 0 here.
    pub last_orphan: u32,
    /// `s_default_mount_opts` (0x100): the journal mode bits on ext3.
    pub default_mount_opts: u32,
    /// `s_jnl_backup_type` (0xFD): 1 when `jnl_blocks` holds a backup of
    /// the journal inode's block map.
    pub jnl_backup_type: u8,
    /// `s_jnl_blocks` (0x10C..0x150): the journal inode's `i_block[0..15]`,
    /// then `i_size_high`, then `i_size`.
    pub jnl_blocks: [u32; 17],
}
```

(b) In `decode`, replace:

```rust
        let mut volume_name = [0u8; 16];
        volume_name.copy_from_slice(&b[120..136]);
        Superblock {
```

with:

```rust
        let mut volume_name = [0u8; 16];
        volume_name.copy_from_slice(&b[120..136]);
        let mut journal_uuid = [0u8; 16];
        journal_uuid.copy_from_slice(&b[0xD0..0xE0]);
        let jnl_blocks = std::array::from_fn(|i| u32_at(b, 0x10C + 4 * i));
        Superblock {
```

and at the end of the struct literal replace:

```rust
            uuid,
            volume_name,
        }
    }
```

with:

```rust
            uuid,
            volume_name,
            journal_uuid,
            journal_inum: u32_at(b, 0xE0),
            journal_dev: u32_at(b, 0xE4),
            last_orphan: u32_at(b, 0xE8),
            default_mount_opts: u32_at(b, 0x100),
            jnl_backup_type: b[0xFD],
            jnl_blocks,
        }
    }
```

(c) Replace the doc comment of `encode`:

```rust
    /// Write every field into the first `LEN` bytes of `out`; the fields
    /// this crate does not model (`s_last_mounted` onwards, bytes 136..1024)
    /// are zeroed. Panics if `out` is shorter than `LEN`.
```

with:

```rust
    /// Write every field into the first `LEN` bytes of `out`; the bytes of
    /// fields this crate does not model (`s_last_mounted` and the rest of
    /// 136..1024 outside the journal fields) are zeroed. Panics if `out` is
    /// shorter than `LEN`.
```

and at the end of its body replace:

```rust
        b[120..136].copy_from_slice(&self.volume_name);
    }
```

with:

```rust
        b[120..136].copy_from_slice(&self.volume_name);
        b[0xD0..0xE0].copy_from_slice(&self.journal_uuid);
        put_u32(b, 0xE0, self.journal_inum);
        put_u32(b, 0xE4, self.journal_dev);
        put_u32(b, 0xE8, self.last_orphan);
        b[0xFD] = self.jnl_backup_type;
        put_u32(b, 0x100, self.default_mount_opts);
        for (i, &v) in self.jnl_blocks.iter().enumerate() {
            put_u32(b, 0x10C + 4 * i, v);
        }
    }
```

(d) In `new`, at the end of the struct literal replace:

```rust
            uuid: options.uuid,
            volume_name: options.label,
        }
```

with:

```rust
            uuid: options.uuid,
            volume_name: options.label,
            journal_uuid: [0; 16],
            journal_inum: 0,
            journal_dev: 0,
            last_orphan: 0,
            default_mount_opts: 0,
            jnl_backup_type: 0,
            jnl_blocks: [0; 17],
        }
```

Run: `cargo test -p ext --lib superblock`
Expected: `test result: ok. 12 passed; 0 failed` (the filter also matches one `group` test), including `superblock::tests::journal_fields_encode_at_their_offsets_and_round_trip ... ok`.

No other `Superblock { .. }` literal needs a change: every other construction in the workspace (`crates/ext/src/group.rs` tests, `crates/ext/tests/ext2.rs`, the `validate` tests) uses `..` struct update syntax.

- [ ] **Step 9: Run every gate**

Run: `cargo fmt --all -- --check`
Expected: no output, exit 0.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: `Finished`, no warnings.

Run: `cargo test --workspace`
Expected: every suite green; in particular `ext` lib `55 passed`, `tests/e2fsprogs.rs` `9 passed` (the ext2 oracle tests still pass: ext2 images are byte-identical), `tests/ext2.rs` `52 passed`, `fs_core` lib `34 passed`, `fs_emulator_wasm` lib `20 passed`, `fat` lib `82 passed`, `tests/fat16.rs` `16 passed`.

Run: `cargo build -p fs-emulator-wasm --target wasm32-unknown-unknown`
Expected: `Finished`.

Optional (needs Node on `PATH`): `wasm-pack test --node crates/wasm`
Expected: `test result: ok. 15 passed; 0 failed`.

- [ ] **Step 10: Commit**

```
git add crates/fs-core/src/error.rs crates/fs-core/src/layout.rs crates/wasm/src/error.rs crates/wasm/src/dto.rs crates/ext/src/fs.rs crates/ext/src/superblock.rs
git commit -m "feat(core): needs-recovery error, journal region kind, and the ext3 superblock fields"
```

---

### Task 2: JBD2 journal primitives

Spec: sections 1 (the module and its public types), 2 (the on-disk format: journal superblock table, constants, descriptor, commit, escape), 3 (the journal size table), 4 items 3, 4, 5, 6 (the journal superblock checks, the feature names, the mode bits), 5.2 (the ring rule `wrap`), 6 (the tag parsing rule and the `CrashPhase` strings), 10.1 (the unit tests).

This task adds pure codecs over byte slices and small types, in one file, with no caller yet: `ExtFs` does not use them until Task 3. The module is `pub` and every public item is re-exported from `lib.rs`, so nothing is dead code and no `#[allow(dead_code)]` is needed. It touches no other file: not `fs.rs`, `superblock.rs`, or `events.rs` (Tasks 1 and 3 own those).

**Files**

- Create: `crates/ext/src/journal/mod.rs` (1,109 lines): constants, `JournalMode`, `JournalOptions`, `CrashPhase`, `default_journal_blocks`, `wrap`, `descriptor_blocks_needed` (lines 1-168); the big-endian helpers, `JournalSuperblock`, the feature-name tables, `journal_feature_names` (lines 170-397); `BlockHeader`, `read_header`, `write_header`, `Tag`, `encode_descriptor`, `decode_descriptor`, `encode_commit`, `decode_commit`, `needs_escape`, `escape`, `unescape` (lines 399-526).
- Modify: `crates/ext/src/lib.rs`: `pub mod journal;` (line 13) and the `pub use journal::{..}` re-export block (lines 21-31).
- Test: `crates/ext/src/journal/mod.rs` `#[cfg(test)] mod tests` (lines 528-1109): modes, phases, size table, ring, descriptor count (lines 532-622); the journal superblock and `validate` (lines 624-915); headers, descriptors, escape, commit (lines 917-1108).

**Interfaces**

Consumes: `fs_core::{Error, Result}` (`Error::Unsupported(String)`, `Error::CorruptImage(String)`). Nothing from other tasks.

Produces (all in `crates/ext/src/journal/mod.rs`, re-exported at the crate root as `ext::<name>` and reachable as `ext::journal::<name>` / `crate::journal::<name>`):

```rust
pub const JOURNAL_INO: u32 = 8;
pub const JBD2_MAGIC: u32 = 0xC03B_3998;
pub const BLOCKTYPE_DESCRIPTOR: u32 = 1;
pub const BLOCKTYPE_COMMIT: u32 = 2;
pub const BLOCKTYPE_SUPERBLOCK_V1: u32 = 3;
pub const BLOCKTYPE_SUPERBLOCK_V2: u32 = 4;
pub const BLOCKTYPE_REVOKE: u32 = 5;
pub const TAG_ESCAPE: u16 = 0x1;
pub const TAG_SAME_UUID: u16 = 0x2;
pub const TAG_DELETED: u16 = 0x4;
pub const TAG_LAST: u16 = 0x8;
pub const HEADER_LEN: usize = 12;
pub const TAG_BYTES: usize = 8;
pub const TAGS_PER_DESCRIPTOR: usize = 122;
pub const MIN_JOURNAL_BLOCKS: u32 = 1024;
pub const MIN_VOLUME_BLOCKS_FOR_JOURNAL: u32 = 2048;
pub const FEATURE_COMPAT_HAS_JOURNAL: u32 = 0x0004;
pub const FEATURE_INCOMPAT_RECOVER: u32 = 0x0004;
pub const DEFM_JMODE_MASK: u32 = 0x0060;
pub const DEFM_JMODE_DATA: u32 = 0x0020;
pub const DEFM_JMODE_ORDERED: u32 = 0x0040;
pub const DEFM_JMODE_WBACK: u32 = 0x0060;
pub const COMMIT_SEC_OFFSET: usize = 48;
pub const COMMIT_NSEC_OFFSET: usize = 56;

pub enum JournalMode { Ordered /* default */, Data }   // Debug, Clone, Copy, PartialEq, Eq, Default
impl JournalMode {
    pub fn mount_opts(self) -> u32;                     // Ordered -> 0x0040, Data -> 0x0020
    pub fn from_mount_opts(opts: u32) -> Result<Self>;  // masks 0x0060: 0 | 0x0040 -> Ordered, 0x0020 -> Data, 0x0060 -> Err(Unsupported("writeback journaling"))
    pub fn as_str(self) -> &'static str;                // "ordered" | "data"
    pub fn parse(s: &str) -> Option<Self>;              // inverse of as_str, exact spelling only
}
pub struct JournalOptions { pub blocks: Option<u32>, pub mode: JournalMode }   // Debug, Clone, Copy, PartialEq, Eq, Default
pub enum CrashPhase { BeforeCommit, AfterCommit, DuringCheckpoint }            // Debug, Clone, Copy, PartialEq, Eq
impl CrashPhase {
    pub fn as_str(self) -> &'static str;   // "before_commit" | "after_commit" | "during_checkpoint"
    pub fn parse(s: &str) -> Option<Self>; // inverse of as_str
}
impl fmt::Display for CrashPhase;          // "before commit" | "after commit" | "during checkpoint"

pub fn default_journal_blocks(total_blocks: u32) -> Option<u32>; // None < 2048; 1024 < 32,768; 4096 < 262,144; else 8192
pub fn wrap(index: u32, first: u32, maxlen: u32) -> u32;         // index < maxlen ? index : index - (maxlen - first)
pub fn descriptor_blocks_needed(tagged: usize) -> usize;         // tagged.div_ceil(122)

pub struct JournalSuperblock {                                   // Debug, Clone, PartialEq, Eq
    pub blocktype: u32, pub sequence_hdr: u32, pub blocksize: u32, pub maxlen: u32, pub first: u32,
    pub sequence: u32, pub start: u32, pub errno: i32, pub feature_compat: u32, pub feature_incompat: u32,
    pub feature_ro_compat: u32, pub uuid: [u8; 16], pub nr_users: u32, pub dynsuper: u32,
    pub max_transaction: u32, pub max_trans_data: u32, pub checksum_type: u8, pub num_fc_blks: u32,
    pub head: u32, pub checksum: u32, pub users: [u8; 768],
}
impl JournalSuperblock {
    pub const LEN: usize = 1024;
    pub const SEQUENCE_OFFSET: usize = 0x18;
    pub const START_OFFSET: usize = 0x1C;
    pub fn new(maxlen: u32, uuid: [u8; 16]) -> Self;        // spec section 2 format-time values
    pub fn decode(bytes: &[u8]) -> Result<Self>;             // panics if bytes.len() < 1024
    pub fn encode(&self, out: &mut [u8]);                    // all 1024 bytes; panics if out.len() < 1024
    pub fn validate(&self, data_blocks: u32) -> Result<()>;  // spec section 4 items 3 (after the magic), 4, 6
}
pub fn journal_feature_names(compat: u32, incompat: u32, ro_compat: u32) -> Vec<String>;

pub struct BlockHeader { pub blocktype: u32, pub sequence: u32 }   // Debug, Clone, Copy, PartialEq, Eq
pub fn read_header(block: &[u8]) -> Option<BlockHeader>;   // None if shorter than 12 bytes or without the magic
pub fn write_header(block: &mut [u8], blocktype: u32, sequence: u32);
pub struct Tag { pub block: u32, pub flags: u16 }                  // Debug, Clone, Copy, PartialEq, Eq
pub fn encode_descriptor(sequence: u32, uuid: &[u8; 16], tags: &[Tag]) -> [u8; 1024];
pub fn decode_descriptor(block: &[u8]) -> Result<Vec<Tag>>;
pub fn encode_commit(sequence: u32, commit_sec: u64) -> [u8; 1024];
pub fn decode_commit(block: &[u8]) -> Option<u64>;
pub fn needs_escape(block: &[u8]) -> bool;
pub fn escape(block: &mut [u8]);
pub fn unescape(block: &mut [u8]);
```

Behaviour later tasks rely on (pinned by this task's tests):

- `JournalSuperblock::decode` checks only the magic: `Err(CorruptImage("journal superblock magic is 0x{:08X}"))`, e.g. `"journal superblock magic is 0x00000000"`. `encode` always writes the magic, zeroes the padding 0x5C..0xFC and 0x51..0x54, and writes `users` at 0x100..0x400.
- `JournalSuperblock::validate(data_blocks)` returns the first failure in this order, with these exact messages:
  1. `blocktype != 4`: `Unsupported("journal superblock version {blocktype}")`.
  2. `blocksize != 1024`: `CorruptImage("journal superblock s_blocksize is {n}, not 1024")`.
  3. `maxlen != data_blocks`: `CorruptImage("journal superblock s_maxlen is {maxlen} but the journal inode maps {data_blocks} blocks")`.
  4. `first != 1`: `CorruptImage("journal superblock s_first is {n}, not 1")`.
  5. any feature bit: `Unsupported("journal feature {name}")`, `name` = `journal_feature_names(..)[0]`.
  6. `start != 0` and not in `first..maxlen` (half-open): `CorruptImage("journal superblock s_start is {start}, outside the log {first}..{maxlen}")`.
  Task 3's `JournalState::open` calls `decode` then `validate(data block count of inode 8)` for spec items 3, 4, 6, and `JournalMode::from_mount_opts(sb.default_mount_opts)` for item 5.
- `journal_feature_names` lists every set bit, compat then incompat then ro_compat, lowest bit first. Named bits: compat 0x1 `journal_checksum`; incompat 0x1 `journal_incompat_revoke`, 0x2 `journal_64bit`, 0x4 `journal_async_commit`, 0x8 `journal_checksum_v2`, 0x10 `journal_checksum_v3`, 0x20 `fast_commit`. Any other bit prints as `"{word} 0x{bit:08x}"` with `word` in `compat`, `incompat`, `ro_compat` (e.g. `"ro_compat 0x00000001"`).
- `encode_descriptor` masks `SAME_UUID` and `LAST` off the caller's flags and sets them itself (first tag: no `SAME_UUID`, followed by the 16-byte uuid; later tags: `SAME_UUID`; last tag: `LAST`), so `encode_descriptor(seq, uuid, &decode_descriptor(&block)?)` reproduces `block`. `t_checksum` is zero. It panics with `"a descriptor holds 1..=122 tags, not {n}"` on 0 or more than 122 tags: the caller splits a transaction into `descriptor_blocks_needed(n)` chunks of at most `TAGS_PER_DESCRIPTOR`.
- `decode_descriptor` does not look at the header (the caller has already checked it with `read_header`); it returns the tags with their on-disk flags, stepping over a uuid after every tag without `SAME_UUID`, and stops at the first `LAST`. With no `LAST` before the block ends it returns `Err(CorruptImage("journal descriptor tags run past the end of the block without a last tag ({k} read)"))`.
- `encode_commit(seq, sec)`: header type 2, bytes 12..48 zero (`h_chksum_type`, `h_chksum_size`, padding, `h_chksum[8]`), be64 `sec` at 48, be32 0 at 56, the rest zero. The caller passes the emulator clock as Unix seconds, floored at 1. `decode_commit` returns `h_commit_sec` for a type-2 block with the magic, else `None`.
- `needs_escape(block)` is true exactly when the first four bytes are `C0 3B 39 98`; `escape` zeroes them; `unescape` writes the magic back.

**Steps**

- [ ] **Step 1: Register the module.** In `crates/ext/src/lib.rs`, add `pub mod journal;` between `pub mod inode;` and `pub mod superblock;`, so the module list reads:

```rust
mod alloc;
pub mod bitmap;
pub mod blockmap;
pub mod dir;
pub mod events;
pub mod fs;
pub mod group;
pub mod inode;
pub mod journal;
pub mod superblock;
```

The re-exports come in Step 9, once the items exist.

- [ ] **Step 2: Write the first tests (modes, phases, size table, ring, descriptor count).** Create `crates/ext/src/journal/mod.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journal_mode_maps_to_the_default_mount_options() {
        assert_eq!(JournalMode::default(), JournalMode::Ordered);
        assert_eq!(JournalMode::Ordered.mount_opts(), 0x0040);
        assert_eq!(JournalMode::Data.mount_opts(), 0x0020);
        assert_eq!(JournalMode::from_mount_opts(0), Ok(JournalMode::Ordered));
        assert_eq!(
            JournalMode::from_mount_opts(0x0040),
            Ok(JournalMode::Ordered)
        );
        assert_eq!(JournalMode::from_mount_opts(0x0020), Ok(JournalMode::Data));
        // Bits outside the mode mask do not matter.
        assert_eq!(
            JournalMode::from_mount_opts(0x0020 | 0x0004 | 0x0008),
            Ok(JournalMode::Data)
        );
        assert_eq!(
            JournalMode::from_mount_opts(0x0060),
            Err(Error::Unsupported("writeback journaling".into()))
        );
        assert_eq!(
            JournalOptions::default(),
            JournalOptions {
                blocks: None,
                mode: JournalMode::Ordered
            }
        );
    }

    #[test]
    fn journal_mode_and_crash_phase_strings_round_trip() {
        for mode in [JournalMode::Ordered, JournalMode::Data] {
            assert_eq!(JournalMode::parse(mode.as_str()), Some(mode));
        }
        assert_eq!(JournalMode::Ordered.as_str(), "ordered");
        assert_eq!(JournalMode::Data.as_str(), "data");
        assert_eq!(JournalMode::parse("writeback"), None);
        assert_eq!(JournalMode::parse("Ordered"), None);

        let phases = [
            (CrashPhase::BeforeCommit, "before_commit", "before commit"),
            (CrashPhase::AfterCommit, "after_commit", "after commit"),
            (
                CrashPhase::DuringCheckpoint,
                "during_checkpoint",
                "during checkpoint",
            ),
        ];
        for (phase, dto, text) in phases {
            assert_eq!(phase.as_str(), dto);
            assert_eq!(CrashPhase::parse(dto), Some(phase));
            assert_eq!(phase.to_string(), text);
        }
        assert_eq!(CrashPhase::parse("before commit"), None);
        assert_eq!(CrashPhase::parse(""), None);
    }

    #[test]
    fn default_journal_blocks_follows_the_mke2fs_table() {
        assert_eq!(default_journal_blocks(0), None);
        assert_eq!(default_journal_blocks(2047), None);
        assert_eq!(default_journal_blocks(2048), Some(1024));
        assert_eq!(default_journal_blocks(16_384), Some(1024));
        assert_eq!(default_journal_blocks(32_767), Some(1024));
        assert_eq!(default_journal_blocks(32_768), Some(4096));
        assert_eq!(default_journal_blocks(262_143), Some(4096));
        assert_eq!(default_journal_blocks(262_144), Some(8192));
        assert_eq!(
            default_journal_blocks(MIN_VOLUME_BLOCKS_FOR_JOURNAL),
            Some(MIN_JOURNAL_BLOCKS)
        );
    }

    #[test]
    fn wrap_folds_indexes_past_maxlen_back_to_first() {
        assert_eq!(wrap(1, 1, 1024), 1);
        assert_eq!(wrap(1023, 1, 1024), 1023);
        assert_eq!(wrap(1024, 1, 1024), 1);
        assert_eq!(wrap(1025, 1, 1024), 2);
        assert_eq!(wrap(1030, 1, 1024), 7);
    }

    #[test]
    fn descriptor_blocks_needed_is_ceil_of_122() {
        assert_eq!(descriptor_blocks_needed(0), 0);
        assert_eq!(descriptor_blocks_needed(1), 1);
        assert_eq!(descriptor_blocks_needed(122), 1);
        assert_eq!(descriptor_blocks_needed(123), 2);
        assert_eq!(descriptor_blocks_needed(244), 2);
        assert_eq!(descriptor_blocks_needed(245), 3);
    }
}
```

- [ ] **Step 3: Run them and watch them fail to compile.**

Run: `cargo test -p ext --lib journal::`

Expected: exit code 101 with ``error[E0433]: cannot find type `JournalMode` in this scope`` (and the same for `CrashPhase`, `Error`, `JournalOptions`, `default_journal_blocks`, `wrap`, `descriptor_blocks_needed`, `MIN_JOURNAL_BLOCKS`, `MIN_VOLUME_BLOCKS_FOR_JOURNAL`), ending ``error: could not compile `ext` (lib test) due to 52 previous errors; 1 warning emitted``.

- [ ] **Step 4: Implement the constants, modes, phases, size table, and ring.** Insert at the top of `crates/ext/src/journal/mod.rs`, above `#[cfg(test)]`, followed by one blank line:

```rust
//! The ext3 journal (JBD2 on the reserved inode 8): the on-disk constants,
//! the journal superblock, descriptor, and commit codecs, the escape rule,
//! the ring arithmetic, and the small types the rest of the journal code and
//! the wasm boundary share. Everything here is a pure function over byte
//! slices; every JBD2 field is big-endian.

use fs_core::{Error, Result};
use std::fmt;

/// The reserved inode that holds the journal.
pub const JOURNAL_INO: u32 = 8;
pub const JBD2_MAGIC: u32 = 0xC03B_3998;
pub const BLOCKTYPE_DESCRIPTOR: u32 = 1;
pub const BLOCKTYPE_COMMIT: u32 = 2;
pub const BLOCKTYPE_SUPERBLOCK_V1: u32 = 3;
pub const BLOCKTYPE_SUPERBLOCK_V2: u32 = 4;
pub const BLOCKTYPE_REVOKE: u32 = 5;
/// The copy's first four bytes were the magic and are zeroed in the journal.
pub const TAG_ESCAPE: u16 = 0x1;
/// The tag shares the uuid of the tag before it; no uuid follows it.
pub const TAG_SAME_UUID: u16 = 0x2;
pub const TAG_DELETED: u16 = 0x4;
/// The last tag of its descriptor block.
pub const TAG_LAST: u16 = 0x8;
/// Magic, block type, and sequence: the header every journal block but a
/// copy starts with.
pub const HEADER_LEN: usize = 12;
/// `t_blocknr` (be32), `t_checksum` (be16), `t_flags` (be16).
pub const TAG_BYTES: usize = 8;
/// The kernel starts a descriptor with 1012 bytes of space, spends 8 per tag
/// plus 16 for the uuid after the first, and closes the block once fewer
/// than 24 bytes remain: after the 122nd tag.
pub const TAGS_PER_DESCRIPTOR: usize = 122;
pub const MIN_JOURNAL_BLOCKS: u32 = 1024;
pub const MIN_VOLUME_BLOCKS_FOR_JOURNAL: u32 = 2048;
/// `s_feature_compat` bit of the ext superblock.
pub const FEATURE_COMPAT_HAS_JOURNAL: u32 = 0x0004;
/// `s_feature_incompat` bit of the ext superblock.
pub const FEATURE_INCOMPAT_RECOVER: u32 = 0x0004;
/// The journal mode bits of the ext superblock's `s_default_mount_opts`.
pub const DEFM_JMODE_MASK: u32 = 0x0060;
pub const DEFM_JMODE_DATA: u32 = 0x0020;
pub const DEFM_JMODE_ORDERED: u32 = 0x0040;
pub const DEFM_JMODE_WBACK: u32 = 0x0060;
/// `h_commit_sec` (be64) in the commit block.
pub const COMMIT_SEC_OFFSET: usize = 48;
/// `h_commit_nsec` (be32) in the commit block.
pub const COMMIT_NSEC_OFFSET: usize = 56;

/// What the journal carries: metadata only (file data written home first),
/// or metadata and file data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JournalMode {
    #[default]
    Ordered,
    Data,
}

impl JournalMode {
    /// The journal mode bits of `s_default_mount_opts`.
    pub fn mount_opts(self) -> u32 {
        match self {
            JournalMode::Ordered => DEFM_JMODE_ORDERED,
            JournalMode::Data => DEFM_JMODE_DATA,
        }
    }

    /// Read the journal mode bits of `s_default_mount_opts`; the other bits
    /// are ignored. No mode bits means ordered, the kernel's default.
    pub fn from_mount_opts(opts: u32) -> Result<Self> {
        match opts & DEFM_JMODE_MASK {
            DEFM_JMODE_DATA => Ok(JournalMode::Data),
            DEFM_JMODE_WBACK => Err(Error::Unsupported("writeback journaling".into())),
            _ => Ok(JournalMode::Ordered),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            JournalMode::Ordered => "ordered",
            JournalMode::Data => "data",
        }
    }

    /// The inverse of `as_str`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ordered" => Some(JournalMode::Ordered),
            "data" => Some(JournalMode::Data),
            _ => None,
        }
    }
}

/// The journal part of the format options. `blocks: None` takes mke2fs's
/// size for the volume (`default_journal_blocks`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JournalOptions {
    pub blocks: Option<u32>,
    pub mode: JournalMode,
}

/// Where an armed crash stops the next transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashPhase {
    /// After the last copy; the commit block is never written.
    BeforeCommit,
    /// After the commit block, before any checkpoint write.
    AfterCommit,
    /// After the first checkpoint write.
    DuringCheckpoint,
}

impl CrashPhase {
    /// The DTO string.
    pub fn as_str(self) -> &'static str {
        match self {
            CrashPhase::BeforeCommit => "before_commit",
            CrashPhase::AfterCommit => "after_commit",
            CrashPhase::DuringCheckpoint => "during_checkpoint",
        }
    }

    /// The inverse of `as_str`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "before_commit" => Some(CrashPhase::BeforeCommit),
            "after_commit" => Some(CrashPhase::AfterCommit),
            "during_checkpoint" => Some(CrashPhase::DuringCheckpoint),
            _ => None,
        }
    }
}

impl fmt::Display for CrashPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CrashPhase::BeforeCommit => "before commit",
            CrashPhase::AfterCommit => "after commit",
            CrashPhase::DuringCheckpoint => "during checkpoint",
        })
    }
}

/// mke2fs's table: None below 2048 blocks; 1024 below 32,768; 4096 below 262,144; else 8192.
pub fn default_journal_blocks(total_blocks: u32) -> Option<u32> {
    match total_blocks {
        0..2048 => None,
        2048..32_768 => Some(1024),
        32_768..262_144 => Some(4096),
        _ => Some(8192),
    }
}

/// The ring: `x` if `x < maxlen`, else `x - (maxlen - first)`.
pub fn wrap(index: u32, first: u32, maxlen: u32) -> u32 {
    if index < maxlen {
        index
    } else {
        index - (maxlen - first)
    }
}

/// The descriptor blocks a transaction of `tagged` blocks needs:
/// `ceil(tagged / 122)`, 0 for 0.
pub fn descriptor_blocks_needed(tagged: usize) -> usize {
    tagged.div_ceil(TAGS_PER_DESCRIPTOR)
}
```

Run: `cargo test -p ext --lib journal::`

Expected: `running 5 tests` and `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 55 filtered out`.

- [ ] **Step 5: Write the journal superblock tests.** Append inside `mod tests`, after `descriptor_blocks_needed_is_ceil_of_122` and before the module's closing `}`, separated by one blank line:

```rust
    const UUID: [u8; 16] = [
        0xe2, 0xf5, 0xee, 0x00, 0x20, 0x26, 0x49, 0x23, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x01,
    ];

    fn unsupported(sb: &JournalSuperblock, data_blocks: u32) -> String {
        match sb.validate(data_blocks) {
            Err(Error::Unsupported(msg)) => msg,
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    fn corrupt(sb: &JournalSuperblock, data_blocks: u32) -> String {
        match sb.validate(data_blocks) {
            Err(Error::CorruptImage(msg)) => msg,
            other => panic!("expected CorruptImage, got {other:?}"),
        }
    }

    #[test]
    fn new_encodes_the_mke2fs_format_time_bytes() {
        // mke2fs 1.47.4, `-t ext3 -b 1024 -J size=1`: journal index 0.
        let mut expected = [0u8; 0x60];
        let words: [u32; 8] = [
            0xc03b_3998,
            0x0000_0004,
            0x0000_0000,
            0x0000_0400,
            0x0000_0400,
            0x0000_0001,
            0x0000_0001,
            0x0000_0000,
        ];
        for (i, word) in words.iter().enumerate() {
            expected[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        expected[0x30..0x40].copy_from_slice(&UUID);
        expected[0x40..0x44].copy_from_slice(&[0, 0, 0, 1]);

        let mut bytes = [0xAAu8; 1024];
        JournalSuperblock::new(1024, UUID).encode(&mut bytes);
        assert_eq!(&bytes[..0x60], &expected[..]);
        assert!(bytes[0x60..0x400].iter().all(|&b| b == 0));
        assert_eq!(
            &bytes[JournalSuperblock::SEQUENCE_OFFSET..JournalSuperblock::SEQUENCE_OFFSET + 4],
            &[0, 0, 0, 1]
        );
        assert_eq!(
            &bytes[JournalSuperblock::START_OFFSET..JournalSuperblock::START_OFFSET + 4],
            &[0, 0, 0, 0]
        );
    }

    #[test]
    fn superblock_decode_round_trips_encode() {
        let mut users = [0u8; 768];
        users[..16].copy_from_slice(&UUID);
        users[767] = 0x5A;
        let sb = JournalSuperblock {
            blocktype: 3,
            sequence_hdr: 0x0102_0304,
            blocksize: 4096,
            maxlen: 8192,
            first: 2,
            sequence: 0xDEAD_BEEF,
            start: 17,
            errno: -5,
            feature_compat: 0x1,
            feature_incompat: 0x12,
            feature_ro_compat: 0x8000_0000,
            uuid: UUID,
            nr_users: 2,
            dynsuper: 3,
            max_transaction: 256,
            max_trans_data: 128,
            checksum_type: 4,
            num_fc_blks: 64,
            head: 99,
            checksum: 0xCAFE_F00D,
            users,
        };
        let mut bytes = [0xAAu8; 1024];
        sb.encode(&mut bytes);
        assert_eq!(&bytes[0x20..0x24], &(-5i32).to_be_bytes());
        assert_eq!(bytes[0x50], 4);
        assert_eq!(&bytes[0x51..0x54], &[0, 0, 0]);
        assert!(bytes[0x5C..0xFC].iter().all(|&b| b == 0));
        assert_eq!(&bytes[0xFC..0x100], &[0xCA, 0xFE, 0xF0, 0x0D]);
        assert_eq!(JournalSuperblock::decode(&bytes), Ok(sb));

        let fresh = JournalSuperblock::new(1024, UUID);
        fresh.encode(&mut bytes);
        assert_eq!(JournalSuperblock::decode(&bytes), Ok(fresh));
    }

    #[test]
    fn decode_rejects_a_block_without_the_magic() {
        let bytes = [0u8; 1024];
        assert_eq!(
            JournalSuperblock::decode(&bytes),
            Err(Error::CorruptImage(
                "journal superblock magic is 0x00000000".into()
            ))
        );
        let mut bytes = [0u8; 1024];
        JournalSuperblock::new(1024, UUID).encode(&mut bytes);
        bytes[3] = 0x99;
        assert_eq!(
            JournalSuperblock::decode(&bytes),
            Err(Error::CorruptImage(
                "journal superblock magic is 0xC03B3999".into()
            ))
        );
    }

    #[test]
    fn validate_accepts_the_format_time_superblock_and_any_start_in_the_log() {
        let good = JournalSuperblock::new(1024, UUID);
        assert_eq!(good.validate(1024), Ok(()));
        for start in [1, 500, 1023] {
            let sb = JournalSuperblock {
                start,
                ..good.clone()
            };
            assert_eq!(sb.validate(1024), Ok(()));
        }
    }

    #[test]
    fn validate_reports_the_version_then_each_field() {
        let good = JournalSuperblock::new(1024, UUID);
        assert_eq!(
            unsupported(
                &JournalSuperblock {
                    blocktype: BLOCKTYPE_SUPERBLOCK_V1,
                    ..good.clone()
                },
                1024
            ),
            "journal superblock version 3"
        );
        // The version comes before the fields.
        assert_eq!(
            unsupported(
                &JournalSuperblock {
                    blocktype: BLOCKTYPE_SUPERBLOCK_V1,
                    blocksize: 4096,
                    ..good.clone()
                },
                1024
            ),
            "journal superblock version 3"
        );
        let msg = corrupt(
            &JournalSuperblock {
                blocksize: 4096,
                ..good.clone()
            },
            1024,
        );
        assert_eq!(msg, "journal superblock s_blocksize is 4096, not 1024");
        let msg = corrupt(&good, 2048);
        assert_eq!(
            msg,
            "journal superblock s_maxlen is 1024 but the journal inode maps 2048 blocks"
        );
        let msg = corrupt(
            &JournalSuperblock {
                first: 2,
                ..good.clone()
            },
            1024,
        );
        assert_eq!(msg, "journal superblock s_first is 2, not 1");
        // The fields come before the features.
        let msg = corrupt(
            &JournalSuperblock {
                first: 0,
                feature_incompat: 0x2,
                ..good
            },
            1024,
        );
        assert!(msg.contains("s_first"), "{msg}");
    }

    #[test]
    fn validate_names_the_first_journal_feature() {
        let good = JournalSuperblock::new(1024, UUID);
        let with = |compat, incompat, ro_compat| JournalSuperblock {
            feature_compat: compat,
            feature_incompat: incompat,
            feature_ro_compat: ro_compat,
            ..good.clone()
        };
        assert_eq!(
            unsupported(&with(0x1, 0, 0), 1024),
            "journal feature journal_checksum"
        );
        assert_eq!(
            unsupported(&with(0, 0x2, 0), 1024),
            "journal feature journal_64bit"
        );
        assert_eq!(
            unsupported(&with(0, 0x10 | 0x8, 0), 1024),
            "journal feature journal_checksum_v2"
        );
        assert_eq!(
            unsupported(&with(0, 0, 0x1), 1024),
            "journal feature ro_compat 0x00000001"
        );
        // compat before incompat before ro_compat.
        assert_eq!(
            unsupported(&with(0x1, 0x1, 0x1), 1024),
            "journal feature journal_checksum"
        );
        assert_eq!(
            unsupported(&with(0, 0x20, 0x1), 1024),
            "journal feature fast_commit"
        );
        // The features come before the start.
        let msg = unsupported(
            &JournalSuperblock {
                start: 5000,
                ..with(0, 0x1, 0)
            },
            1024,
        );
        assert_eq!(msg, "journal feature journal_incompat_revoke");
    }

    #[test]
    fn validate_rejects_a_start_outside_the_log() {
        let good = JournalSuperblock::new(1024, UUID);
        let msg = corrupt(
            &JournalSuperblock {
                start: 1024,
                ..good.clone()
            },
            1024,
        );
        assert_eq!(
            msg,
            "journal superblock s_start is 1024, outside the log 1..1024"
        );
        let msg = corrupt(
            &JournalSuperblock {
                start: 70_000,
                ..good
            },
            1024,
        );
        assert!(msg.contains("s_start is 70000"), "{msg}");
    }

    #[test]
    fn journal_feature_names_uses_the_e2fsprogs_spelling() {
        assert!(journal_feature_names(0, 0, 0).is_empty());
        assert_eq!(journal_feature_names(0x1, 0, 0), vec!["journal_checksum"]);
        let incompat = [
            (0x01, "journal_incompat_revoke"),
            (0x02, "journal_64bit"),
            (0x04, "journal_async_commit"),
            (0x08, "journal_checksum_v2"),
            (0x10, "journal_checksum_v3"),
            (0x20, "fast_commit"),
        ];
        for (bit, name) in incompat {
            assert_eq!(journal_feature_names(0, bit, 0), vec![name]);
        }
        assert_eq!(
            journal_feature_names(0x2, 0x40, 0x8000_0001),
            vec![
                "compat 0x00000002",
                "incompat 0x00000040",
                "ro_compat 0x00000001",
                "ro_compat 0x80000000",
            ]
        );
        assert_eq!(
            journal_feature_names(0x1, 0x3f, 0),
            vec![
                "journal_checksum",
                "journal_incompat_revoke",
                "journal_64bit",
                "journal_async_commit",
                "journal_checksum_v2",
                "journal_checksum_v3",
                "fast_commit",
            ]
        );
    }
```

Run: `cargo test -p ext --lib journal::`

Expected: exit code 101 with ``error[E0433]: cannot find type `JournalSuperblock` in this scope`` and ``error[E0425]: cannot find function `journal_feature_names` in this scope``, ending ``error: could not compile `ext` (lib test) due to 33 previous errors``.

The format-time bytes in `new_encodes_the_mke2fs_format_time_bytes` are journal index 0 of a real image: `mke2fs -q -F -t ext3 -b 1024 -I 128 -O has_journal,filetype,sparse_super,^resize_inode,^dir_index,^ext_attr -J size=1 -U e2f5ee00-2026-4923-8000-000000000001` (e2fsprogs 1.47.4) on a 16 MiB file, then `debugfs -R "bmap <8> 0"` gives the physical block; its 1024 bytes are exactly the eight words at 0x00..0x20, the uuid at 0x30, `00000001` at 0x40, and zero everywhere else.

- [ ] **Step 6: Implement the journal superblock and the feature names.** Insert directly above `#[cfg(test)]` (after `descriptor_blocks_needed`), with one blank line on each side:

```rust
fn be32_at(b: &[u8], i: usize) -> u32 {
    u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

fn put_be32(b: &mut [u8], i: usize, v: u32) {
    b[i..i + 4].copy_from_slice(&v.to_be_bytes());
}

/// The journal superblock (journal index 0), version 2. The padding at
/// 0x5C..0xFC is not stored: `encode` writes it as zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalSuperblock {
    pub blocktype: u32,
    pub sequence_hdr: u32,
    pub blocksize: u32,
    pub maxlen: u32,
    pub first: u32,
    pub sequence: u32,
    pub start: u32,
    pub errno: i32,
    pub feature_compat: u32,
    pub feature_incompat: u32,
    pub feature_ro_compat: u32,
    pub uuid: [u8; 16],
    pub nr_users: u32,
    pub dynsuper: u32,
    pub max_transaction: u32,
    pub max_trans_data: u32,
    pub checksum_type: u8,
    pub num_fc_blks: u32,
    pub head: u32,
    pub checksum: u32,
    pub users: [u8; 768],
}

impl JournalSuperblock {
    pub const LEN: usize = 1024;
    /// `s_sequence`, the tid the next transaction takes.
    pub const SEQUENCE_OFFSET: usize = 0x18;
    /// `s_start`, the journal index of the oldest live transaction; 0 when
    /// the journal is empty.
    pub const START_OFFSET: usize = 0x1C;

    /// The values mke2fs writes at format: version 2, 1 KiB blocks, the log
    /// from index 1, sequence 1, empty, one user counted but its slot left
    /// zero.
    pub fn new(maxlen: u32, uuid: [u8; 16]) -> Self {
        JournalSuperblock {
            blocktype: BLOCKTYPE_SUPERBLOCK_V2,
            sequence_hdr: 0,
            blocksize: 1024,
            maxlen,
            first: 1,
            sequence: 1,
            start: 0,
            errno: 0,
            feature_compat: 0,
            feature_incompat: 0,
            feature_ro_compat: 0,
            uuid,
            nr_users: 1,
            dynsuper: 0,
            max_transaction: 0,
            max_trans_data: 0,
            checksum_type: 0,
            num_fc_blks: 0,
            head: 0,
            checksum: 0,
            users: [0; 768],
        }
    }

    /// Read every field of the first `LEN` bytes. Only the magic is checked
    /// here; `validate` checks the rest. Panics if `bytes` is shorter than
    /// `LEN`.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let b = &bytes[..Self::LEN];
        let magic = be32_at(b, 0x00);
        if magic != JBD2_MAGIC {
            return Err(Error::CorruptImage(format!(
                "journal superblock magic is 0x{magic:08X}"
            )));
        }
        let mut uuid = [0u8; 16];
        uuid.copy_from_slice(&b[0x30..0x40]);
        let mut users = [0u8; 768];
        users.copy_from_slice(&b[0x100..0x400]);
        Ok(JournalSuperblock {
            blocktype: be32_at(b, 0x04),
            sequence_hdr: be32_at(b, 0x08),
            blocksize: be32_at(b, 0x0C),
            maxlen: be32_at(b, 0x10),
            first: be32_at(b, 0x14),
            sequence: be32_at(b, Self::SEQUENCE_OFFSET),
            start: be32_at(b, Self::START_OFFSET),
            errno: be32_at(b, 0x20) as i32,
            feature_compat: be32_at(b, 0x24),
            feature_incompat: be32_at(b, 0x28),
            feature_ro_compat: be32_at(b, 0x2C),
            uuid,
            nr_users: be32_at(b, 0x40),
            dynsuper: be32_at(b, 0x44),
            max_transaction: be32_at(b, 0x48),
            max_trans_data: be32_at(b, 0x4C),
            checksum_type: b[0x50],
            num_fc_blks: be32_at(b, 0x54),
            head: be32_at(b, 0x58),
            checksum: be32_at(b, 0xFC),
            users,
        })
    }

    /// Write all `LEN` bytes of `out`, the magic included and the padding
    /// zeroed. Panics if `out` is shorter than `LEN`.
    pub fn encode(&self, out: &mut [u8]) {
        let b = &mut out[..Self::LEN];
        b.fill(0);
        put_be32(b, 0x00, JBD2_MAGIC);
        put_be32(b, 0x04, self.blocktype);
        put_be32(b, 0x08, self.sequence_hdr);
        put_be32(b, 0x0C, self.blocksize);
        put_be32(b, 0x10, self.maxlen);
        put_be32(b, 0x14, self.first);
        put_be32(b, Self::SEQUENCE_OFFSET, self.sequence);
        put_be32(b, Self::START_OFFSET, self.start);
        put_be32(b, 0x20, self.errno as u32);
        put_be32(b, 0x24, self.feature_compat);
        put_be32(b, 0x28, self.feature_incompat);
        put_be32(b, 0x2C, self.feature_ro_compat);
        b[0x30..0x40].copy_from_slice(&self.uuid);
        put_be32(b, 0x40, self.nr_users);
        put_be32(b, 0x44, self.dynsuper);
        put_be32(b, 0x48, self.max_transaction);
        put_be32(b, 0x4C, self.max_trans_data);
        b[0x50] = self.checksum_type;
        put_be32(b, 0x54, self.num_fc_blks);
        put_be32(b, 0x58, self.head);
        put_be32(b, 0xFC, self.checksum);
        b[0x100..0x400].copy_from_slice(&self.users);
    }

    /// Check what this crate can replay and write, for a journal inode that
    /// maps `data_blocks` blocks, and return the first failure: the version
    /// (`Unsupported`), then block size, `s_maxlen`, and `s_first`
    /// (`CorruptImage`), then the feature words (`Unsupported`, the first set
    /// bit by name), then `s_start` (`CorruptImage`).
    pub fn validate(&self, data_blocks: u32) -> Result<()> {
        if self.blocktype != BLOCKTYPE_SUPERBLOCK_V2 {
            return Err(Error::Unsupported(format!(
                "journal superblock version {}",
                self.blocktype
            )));
        }
        if self.blocksize != 1024 {
            return Err(Error::CorruptImage(format!(
                "journal superblock s_blocksize is {}, not 1024",
                self.blocksize
            )));
        }
        if self.maxlen != data_blocks {
            return Err(Error::CorruptImage(format!(
                "journal superblock s_maxlen is {} but the journal inode maps {data_blocks} blocks",
                self.maxlen
            )));
        }
        if self.first != 1 {
            return Err(Error::CorruptImage(format!(
                "journal superblock s_first is {}, not 1",
                self.first
            )));
        }
        if let Some(name) = journal_feature_names(
            self.feature_compat,
            self.feature_incompat,
            self.feature_ro_compat,
        )
        .into_iter()
        .next()
        {
            return Err(Error::Unsupported(format!("journal feature {name}")));
        }
        if self.start != 0 && !(self.first..self.maxlen).contains(&self.start) {
            return Err(Error::CorruptImage(format!(
                "journal superblock s_start is {}, outside the log {}..{}",
                self.start, self.first, self.maxlen
            )));
        }
        Ok(())
    }
}

/// A feature word's bits and their e2fsprogs names.
type FeatureTable = &'static [(u32, &'static str)];

/// e2fsprogs' names of the journal compat feature bits.
const JOURNAL_COMPAT_NAMES: [(u32, &str); 1] = [(0x0001, "journal_checksum")];

/// e2fsprogs' names of the journal incompat feature bits.
const JOURNAL_INCOMPAT_NAMES: [(u32, &str); 6] = [
    (0x0001, "journal_incompat_revoke"),
    (0x0002, "journal_64bit"),
    (0x0004, "journal_async_commit"),
    (0x0008, "journal_checksum_v2"),
    (0x0010, "journal_checksum_v3"),
    (0x0020, "fast_commit"),
];

/// Names for the journal feature words, e2fsprogs spelling, for `validate`
/// and tests: every set bit, compat then incompat then ro_compat, lowest bit
/// first. A bit without a name prints as its word and the bit in hex
/// (`ro_compat 0x00000001`).
pub fn journal_feature_names(compat: u32, incompat: u32, ro_compat: u32) -> Vec<String> {
    let words: [(&str, u32, FeatureTable); 3] = [
        ("compat", compat, &JOURNAL_COMPAT_NAMES),
        ("incompat", incompat, &JOURNAL_INCOMPAT_NAMES),
        ("ro_compat", ro_compat, &[]),
    ];
    let mut names = Vec::new();
    for (word, bits, table) in words {
        for bit in (0..32).map(|i| 1u32 << i).filter(|bit| bits & bit != 0) {
            names.push(match table.iter().find(|(b, _)| *b == bit) {
                Some((_, name)) => (*name).to_string(),
                None => format!("{word} 0x{bit:08x}"),
            });
        }
    }
    names
}
```

Run: `cargo test -p ext --lib journal::`

Expected: `running 13 tests` and `test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 55 filtered out`.

- [ ] **Step 7: Write the header, descriptor, escape, and commit tests.** Append inside `mod tests`, after `journal_feature_names_uses_the_e2fsprogs_spelling` and before the module's closing `}`, separated by one blank line:

```rust
    fn tags(blocks: std::ops::Range<u32>) -> Vec<Tag> {
        blocks.map(|block| Tag { block, flags: 0 }).collect()
    }

    fn be16_at(b: &[u8], i: usize) -> u16 {
        u16::from_be_bytes([b[i], b[i + 1]])
    }

    #[test]
    fn header_round_trips_and_requires_the_magic() {
        let mut block = [0u8; 1024];
        assert_eq!(read_header(&block), None);
        write_header(&mut block, BLOCKTYPE_REVOKE, 42);
        assert_eq!(
            &block[..HEADER_LEN],
            &[0xC0, 0x3B, 0x39, 0x98, 0, 0, 0, 5, 0, 0, 0, 42]
        );
        assert_eq!(
            read_header(&block),
            Some(BlockHeader {
                blocktype: BLOCKTYPE_REVOKE,
                sequence: 42
            })
        );
        assert_eq!(read_header(&block[..HEADER_LEN - 1]), None);
        block[0] = 0;
        assert_eq!(read_header(&block), None);
    }

    #[test]
    fn descriptor_packs_the_uuid_after_the_first_tag() {
        let block = encode_descriptor(
            7,
            &UUID,
            &[
                Tag { block: 5, flags: 0 },
                Tag {
                    block: 69,
                    flags: TAG_ESCAPE,
                },
                Tag {
                    block: 1111,
                    flags: 0,
                },
            ],
        );
        assert_eq!(
            read_header(&block),
            Some(BlockHeader {
                blocktype: BLOCKTYPE_DESCRIPTOR,
                sequence: 7
            })
        );
        // Tag 0 at 12, the uuid at 20, tag 1 at 36, tag 2 at 44.
        assert_eq!(be32_at(&block, 12), 5);
        assert_eq!(be16_at(&block, 16), 0);
        assert_eq!(be16_at(&block, 18), 0);
        assert_eq!(&block[20..36], &UUID);
        assert_eq!(be32_at(&block, 36), 69);
        assert_eq!(be16_at(&block, 40), 0);
        assert_eq!(be16_at(&block, 42), TAG_ESCAPE | TAG_SAME_UUID);
        assert_eq!(be32_at(&block, 44), 1111);
        assert_eq!(be16_at(&block, 50), TAG_SAME_UUID | TAG_LAST);
        assert!(block[52..].iter().all(|&b| b == 0));

        // A single tag is both first and last.
        let one = encode_descriptor(1, &UUID, &tags(82..83));
        assert_eq!(be16_at(&one, 18), TAG_LAST);
        assert_eq!(&one[20..36], &UUID);
        assert!(one[36..].iter().all(|&b| b == 0));
    }

    #[test]
    fn exactly_122_tags_fit_in_a_descriptor() {
        let block = encode_descriptor(3, &UUID, &tags(100..222));
        // 12 header + 8 first tag + 16 uuid + 121 more tags = 1004 bytes.
        let last = HEADER_LEN + TAG_BYTES + 16 + 120 * TAG_BYTES;
        assert_eq!(last + TAG_BYTES, 1004);
        assert_eq!(be32_at(&block, last), 221);
        assert_eq!(be16_at(&block, last + 6), TAG_SAME_UUID | TAG_LAST);
        assert!(block[1004..].iter().all(|&b| b == 0));
        assert_eq!(
            decode_descriptor(&block).unwrap().len(),
            TAGS_PER_DESCRIPTOR
        );
    }

    #[test]
    #[should_panic(expected = "a descriptor holds 1..=122 tags, not 123")]
    fn encode_descriptor_refuses_a_123rd_tag() {
        encode_descriptor(3, &UUID, &tags(0..123));
    }

    #[test]
    fn decode_descriptor_round_trips_encode_descriptor() {
        let input = [
            Tag { block: 5, flags: 0 },
            Tag {
                block: 69,
                flags: TAG_ESCAPE,
            },
            Tag {
                block: 1111,
                flags: 0,
            },
        ];
        let block = encode_descriptor(9, &UUID, &input);
        let decoded = decode_descriptor(&block).unwrap();
        assert_eq!(
            decoded,
            vec![
                Tag { block: 5, flags: 0 },
                Tag {
                    block: 69,
                    flags: TAG_ESCAPE | TAG_SAME_UUID
                },
                Tag {
                    block: 1111,
                    flags: TAG_SAME_UUID | TAG_LAST
                },
            ]
        );
        // The on-disk flags re-encode to the same block.
        assert_eq!(encode_descriptor(9, &UUID, &decoded), block);

        let full = encode_descriptor(4, &UUID, &tags(0..122));
        let decoded = decode_descriptor(&full).unwrap();
        assert_eq!(
            decoded.iter().map(|t| t.block).collect::<Vec<_>>(),
            (0..122).collect::<Vec<_>>()
        );
        assert_eq!(encode_descriptor(4, &UUID, &decoded), full);
    }

    #[test]
    fn decode_descriptor_rejects_a_block_without_a_last_tag() {
        let mut block = encode_descriptor(2, &UUID, &tags(10..12));
        // Clear LAST on the second tag (t_flags at 12 + 8 + 16 + 6 = 42).
        block[43] &= !(TAG_LAST as u8);
        let err = decode_descriptor(&block).unwrap_err();
        assert!(
            matches!(&err, Error::CorruptImage(msg) if msg.contains("without a last tag")),
            "{err:?}"
        );
        // A zeroed body reads as tags that each carry a uuid and never end.
        let mut empty = [0u8; 1024];
        write_header(&mut empty, BLOCKTYPE_DESCRIPTOR, 2);
        assert!(matches!(
            decode_descriptor(&empty),
            Err(Error::CorruptImage(_))
        ));
    }

    #[test]
    fn escape_zeroes_the_magic_and_unescape_restores_it() {
        let mut block = [0x11u8; 1024];
        assert!(!needs_escape(&block));
        block[..4].copy_from_slice(&[0xC0, 0x3B, 0x39, 0x98]);
        assert!(needs_escape(&block));
        let home = block;
        escape(&mut block);
        assert_eq!(&block[..4], &[0, 0, 0, 0]);
        assert_eq!(&block[4..], &home[4..]);
        assert!(!needs_escape(&block));
        unescape(&mut block);
        assert_eq!(block, home);
        // Only the full magic in the first four bytes counts.
        assert!(!needs_escape(&[0xC0, 0x3B, 0x39]));
        assert!(!needs_escape(&[0x00, 0xC0, 0x3B, 0x39, 0x98]));
    }

    #[test]
    fn commit_block_carries_the_commit_time() {
        let block = encode_commit(12, 1_758_672_000);
        assert_eq!(
            &block[..HEADER_LEN],
            &[0xC0, 0x3B, 0x39, 0x98, 0, 0, 0, 2, 0, 0, 0, 12]
        );
        // h_chksum_type, h_chksum_size, padding, and h_chksum[8] are zero.
        assert!(block[HEADER_LEN..COMMIT_SEC_OFFSET].iter().all(|&b| b == 0));
        assert_eq!(
            &block[COMMIT_SEC_OFFSET..COMMIT_SEC_OFFSET + 8],
            &[0, 0, 0, 0, 0x68, 0xD3, 0x34, 0x80]
        );
        assert_eq!(&block[COMMIT_NSEC_OFFSET..COMMIT_NSEC_OFFSET + 4], &[0; 4]);
        assert!(block[COMMIT_NSEC_OFFSET + 4..].iter().all(|&b| b == 0));
        assert_eq!(decode_commit(&block), Some(1_758_672_000));

        assert_eq!(decode_commit(&[0u8; 1024]), None);
        let descriptor = encode_descriptor(12, &UUID, &tags(5..6));
        assert_eq!(decode_commit(&descriptor), None);
    }
```

Run: `cargo test -p ext --lib journal::`

Expected: exit code 101 with ``cannot find function `encode_descriptor` in this scope``, ``cannot find struct, variant or union type `Tag` in this scope``, ``cannot find struct, variant or union type `BlockHeader` in this scope`` (and the same for `read_header`, `write_header`, `decode_descriptor`, `encode_commit`, `decode_commit`, `needs_escape`, `escape`, `unescape`), ending ``error: could not compile `ext` (lib test) due to 46 previous errors``.

- [ ] **Step 8: Implement headers, descriptors, commit blocks, and the escape rule.** Insert directly above `#[cfg(test)]` (after `journal_feature_names`), with one blank line on each side:

```rust
/// The header of a descriptor, commit, or revoke block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockHeader {
    pub blocktype: u32,
    pub sequence: u32,
}

/// The header of `block`; None unless the magic is present.
pub fn read_header(block: &[u8]) -> Option<BlockHeader> {
    let b = block.get(..HEADER_LEN)?;
    (be32_at(b, 0) == JBD2_MAGIC).then(|| BlockHeader {
        blocktype: be32_at(b, 4),
        sequence: be32_at(b, 8),
    })
}

/// Write the magic, `blocktype`, and `sequence` into the first 12 bytes.
pub fn write_header(block: &mut [u8], blocktype: u32, sequence: u32) {
    put_be32(block, 0, JBD2_MAGIC);
    put_be32(block, 4, blocktype);
    put_be32(block, 8, sequence);
}

/// One descriptor tag: the home block and the flags as on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tag {
    pub block: u32,
    pub flags: u16,
}

/// A descriptor block for `sequence` listing `tags` in order. The caller
/// passes only TAG_ESCAPE in flags; SAME_UUID and LAST are set here: the
/// first tag is followed by `uuid`, every later tag sets SAME_UUID, the last
/// sets LAST. Any SAME_UUID or LAST the caller passes is replaced, so
/// re-encoding decoded tags gives the same block. Panics unless
/// `1 <= tags.len() <= TAGS_PER_DESCRIPTOR`.
pub fn encode_descriptor(sequence: u32, uuid: &[u8; 16], tags: &[Tag]) -> [u8; 1024] {
    assert!(
        !tags.is_empty() && tags.len() <= TAGS_PER_DESCRIPTOR,
        "a descriptor holds 1..={TAGS_PER_DESCRIPTOR} tags, not {}",
        tags.len()
    );
    let mut block = [0u8; 1024];
    write_header(&mut block, BLOCKTYPE_DESCRIPTOR, sequence);
    let mut pos = HEADER_LEN;
    for (i, tag) in tags.iter().enumerate() {
        let mut flags = tag.flags & !(TAG_SAME_UUID | TAG_LAST);
        if i > 0 {
            flags |= TAG_SAME_UUID;
        }
        if i + 1 == tags.len() {
            flags |= TAG_LAST;
        }
        put_be32(&mut block, pos, tag.block);
        // t_checksum (be16) at pos + 4 stays zero.
        block[pos + 6..pos + 8].copy_from_slice(&flags.to_be_bytes());
        pos += TAG_BYTES;
        if i == 0 {
            block[pos..pos + 16].copy_from_slice(uuid);
            pos += 16;
        }
    }
    block
}

/// The tags of a descriptor block, stopping at TAG_LAST, with their flags as
/// on disk; a uuid follows every tag without SAME_UUID. The header is not
/// checked. CorruptImage if the tags run past the block without one carrying
/// TAG_LAST.
pub fn decode_descriptor(block: &[u8]) -> Result<Vec<Tag>> {
    let mut tags = Vec::new();
    let mut pos = HEADER_LEN;
    while pos + TAG_BYTES <= block.len() {
        let tag = Tag {
            block: be32_at(block, pos),
            flags: u16::from_be_bytes([block[pos + 6], block[pos + 7]]),
        };
        tags.push(tag);
        if tag.flags & TAG_LAST != 0 {
            return Ok(tags);
        }
        pos += TAG_BYTES;
        if tag.flags & TAG_SAME_UUID == 0 {
            pos += 16;
        }
    }
    Err(Error::CorruptImage(format!(
        "journal descriptor tags run past the end of the block without a last tag ({} read)",
        tags.len()
    )))
}

/// A commit block for `sequence` stamped `commit_sec` (Unix seconds): no
/// checksum, `h_commit_nsec` 0, the rest zero.
pub fn encode_commit(sequence: u32, commit_sec: u64) -> [u8; 1024] {
    let mut block = [0u8; 1024];
    write_header(&mut block, BLOCKTYPE_COMMIT, sequence);
    block[COMMIT_SEC_OFFSET..COMMIT_SEC_OFFSET + 8].copy_from_slice(&commit_sec.to_be_bytes());
    put_be32(&mut block, COMMIT_NSEC_OFFSET, 0);
    block
}

/// The `h_commit_sec` of a commit block; None without the magic or type 2.
pub fn decode_commit(block: &[u8]) -> Option<u64> {
    let header = read_header(block)?;
    if header.blocktype != BLOCKTYPE_COMMIT || block.len() < COMMIT_SEC_OFFSET + 8 {
        return None;
    }
    let mut sec = [0u8; 8];
    sec.copy_from_slice(&block[COMMIT_SEC_OFFSET..COMMIT_SEC_OFFSET + 8]);
    Some(u64::from_be_bytes(sec))
}

/// Whether a copy of `block` must be escaped: its first four bytes are the
/// journal magic, which recovery would take for a journal block.
pub fn needs_escape(block: &[u8]) -> bool {
    block.len() >= 4 && be32_at(block, 0) == JBD2_MAGIC
}

/// Zero the first four bytes.
pub fn escape(block: &mut [u8]) {
    block[..4].fill(0);
}

/// Restore the magic in the first four bytes.
pub fn unescape(block: &mut [u8]) {
    put_be32(block, 0, JBD2_MAGIC);
}
```

Run: `cargo test -p ext --lib journal::`

Expected: `running 21 tests` and `test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 55 filtered out`.

- [ ] **Step 9: Re-export the public items.** In `crates/ext/src/lib.rs`, insert between `pub use inode::Inode;` and `pub use superblock::{`:

```rust
pub use journal::{
    decode_commit, decode_descriptor, default_journal_blocks, descriptor_blocks_needed,
    encode_commit, encode_descriptor, escape, journal_feature_names, needs_escape, read_header,
    unescape, wrap, write_header, BlockHeader, CrashPhase, JournalMode, JournalOptions,
    JournalSuperblock, Tag, BLOCKTYPE_COMMIT, BLOCKTYPE_DESCRIPTOR, BLOCKTYPE_REVOKE,
    BLOCKTYPE_SUPERBLOCK_V1, BLOCKTYPE_SUPERBLOCK_V2, COMMIT_NSEC_OFFSET, COMMIT_SEC_OFFSET,
    DEFM_JMODE_DATA, DEFM_JMODE_MASK, DEFM_JMODE_ORDERED, DEFM_JMODE_WBACK,
    FEATURE_COMPAT_HAS_JOURNAL, FEATURE_INCOMPAT_RECOVER, HEADER_LEN, JBD2_MAGIC, JOURNAL_INO,
    MIN_JOURNAL_BLOCKS, MIN_VOLUME_BLOCKS_FOR_JOURNAL, TAGS_PER_DESCRIPTOR, TAG_BYTES, TAG_DELETED,
    TAG_ESCAPE, TAG_LAST, TAG_SAME_UUID,
};
```

Run: `cargo build -p ext`

Expected: `Finished` with no warnings.

- [ ] **Step 10: Run the gates.**

Run: `cargo fmt --all -- --check`
Expected: no output, exit code 0.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: `Finished` with no warnings or errors.

Run: `cargo test --workspace`
Expected: every `test result:` line reads `ok` with `0 failed`; the `ext` lib unit tests read `test result: ok. 76 passed; 0 failed` (55 from slice 2 and Task 1 plus the 21 here). The e2fsprogs, ext2, fat16, and fs-core results are unchanged.

Run: `cargo build -p fs-emulator-wasm --target wasm32-unknown-unknown`
Expected: `Finished`.

- [ ] **Step 11: Commit.**

```bash
git add crates/ext/src/journal/mod.rs crates/ext/src/lib.rs
git commit -m "feat(ext): JBD2 journal primitives"
```

Expected: one commit touching exactly those two files (`2 files changed, 1121 insertions(+)`).

---

### Task 3: ext3 format and load

**Spec:** sections 1 (`ExtFs.journal`, `JournalState`, `fs_type`), 2 (the journal superblock at format, the ext superblock fields for ext3, inode 8), 3 (format: options, size rules and their messages, placement, counters, the fresh `JournalState`), 4 (`Superblock::validate` variants, `JournalState::open` items 1 to 6 in order, `needs_recovery`, never replaying, the extended corruption gate), 7 (the `journal_info`, `layout`, and `block_owners` bullets only), 10.2 (the Format and Validation bullets, and the ext2 bullet's `journal_info() == None`), 10.3 (the first bullet), 12 (the amendments to `BlockRole`, `Superblock::validate`, `layout`, and the gate).

This task makes the journal real on disk and in memory, with no transactions yet: `ExtFs::format` with `ExtFormatOptions { journal: Some(..) }` lays down inode 8, its 1024 (or mke2fs-table) data blocks and pointer blocks, the journal superblock at journal index 0, and the ext3 fields in every superblock copy; `ExtFs::from_image` opens the journal of a `has_journal` image with the checks of spec section 4 and never replays it; `fs_type()` says `"ext3"`; `journal_info()`, `journal_mode()`, and `needs_recovery()` report it; `layout()` carves a `journal` region out of the data region; `block_owners()` lists inode 8's blocks as `<journal>`; and a raw write to journal index 0, inode 8's slot, or a journal pointer block re-runs the whole parse. ext2 is byte-identical to before (a test pins the FNV-1a hash of the default ext2 image, taken on the base branch before this task).

Everything below was compiled and run on a scratch branch holding Tasks 1 and 2 plus this task, and again after the plan critique's fix round: fmt, workspace clippy, `cargo test --workspace` including the e2fsprogs oracle (also with `CI=1`, nothing skipped), the wasm32 build, and `wasm-pack test --node crates/wasm`. Each round was then replayed from the base branch (Round 1 files, then Round 2, and so on) and every "Expected" below is the observed output. Copy the code verbatim.

Verified by hand against the real tools before writing the tests: `mke2fs -q -F -t ext3 -b 1024 -I 128 -O none,has_journal,filetype,sparse_super -J size=1 -N 1024` on a 16 MiB file gives free blocks 15,205 (group 0: 7,082 with `Free blocks: 1111-8192`; group 1: 8,123), free inodes 1,013, `Journal inode: 8`, `Journal backup: inode blocks`, `Total journal blocks: 1024`, `Journal sequence: 0x00000001`, `Journal start: 0`, journal superblock at block 82 byte-identical to ours apart from the uuid, and `s_jnl_blocks` = `i_block` then `0`, `1048576`. The one layout difference is mke2fs's: it interleaves the pointer blocks with the data (`(IND):94, (DIND):351, ...`), while this task follows the spec (data blocks 82..1105 first, pointer blocks 1106..1110 after, the slice-2 `map_blocks` rule); the used range 82..1110 and every counter agree, and `debugfs -R "stat <8>"` and `e2fsck -fn` accept ours. `dumpe2fs` of the emulator's image differs from mke2fs's only in the uuid, times, reserved-block count, `Default mount options` (`journal_data_ordered` versus mke2fs's `user_xattr acl`), and the `signed_directory_hash`, `Overhead clusters`, and directory-hash lines ext2 already omits. Spec section 10.3 (as amended) states two dumpe2fs 1.47.4 lines beyond the format-time fields: `Max transaction length: 1024`, which dumpe2fs derives from `s_maxlen` because `s_max_transaction` is 0, and `Total journal size: 8M` for an 8192-block journal. `dumpe2fs_describes_the_journal_in_both_modes` asserts the first and `a_262144_block_volume_gets_8192_journal_blocks_and_is_clean` the second.

**Files:**
- Create: `crates/ext/src/journal/state.rs` (251 lines): imports and constants (lines 1-23), `JournalState` (lines 25-41), `open` (lines 43-111), `physical`, `offset`, `max_transaction`, `superblock`, `wrap` (lines 113-138), `journal_size` (lines 140-168), `write_journal` (lines 170-232), `JournalInfo` (lines 234-251).
- Create: `crates/ext/tests/common/mod.rs` (21 lines).
- Create: `crates/ext/tests/ext3.rs` (609 lines): helpers (lines 1-63), format, load, inode-8, size, ext2, and validation tests (lines 65-478), layout and owners tests (lines 480-529), the gate helper and gate tests (lines 531-608).
- Modify: `crates/ext/src/superblock.rs`: import (line 6), `ExtFormatOptions.journal`, `Default`, `ext3()` (lines 94-126), `validate` doc (lines 359-365) and feature checks (lines 391-413); tests: import (line 494), three tests (lines 743-797).
- Modify: `crates/ext/src/journal/mod.rs`: module doc and `pub mod state;` (lines 1-8).
- Modify: `crates/ext/src/lib.rs`: `pub use journal::state::{JournalInfo, JournalState};` (line 21).
- Modify: `crates/ext/src/fs.rs`: imports (lines 15-16), `ExtFs` fields and `pub(crate) fn stamp` (lines 53-73), `open_journal` (lines 152-159), `format` doc and tail plus `write_metadata_copies` (lines 162-167, 248-281), `from_image` (lines 283-310), `fs_type`, `needs_recovery`, `journal_info`, `journal_mode` (lines 343-380), `layout` and `journal_runs` (lines 398-503), `block_owners` (lines 701-765), `primary_metadata`, `reparse_metadata`, `write_raw` (lines 786-863); test module import (line 1917) and `a_reparse_keeps_the_session_head_and_armed_crash` (lines 1935-1948).
- Modify: `crates/wasm/src/dto.rs`: `journal: d.journal,` in `TryFrom<ExtFormatOptions>` (line 333). Needed so the wasm crate compiles once `ExtFormatOptions` gains the field; Task 7 replaces it with the parsed journal options.
- Modify: `crates/wasm/src/volume.rs`: `enum Inner` boxes both variants (lines 14-17, 58-70, 131-151, 184, 204). With the journal field `ExtFs` is 544 bytes against `FatFs`'s 312, and clippy's `large_enum_variant` (a `-D warnings` gate) rejects the difference; boxing only `Ext` makes `Fat` the large one, so both are boxed.
- Test: `crates/ext/tests/e2fsprogs.rs`: `mod common;` and `use common::pattern;` in place of the file's own `pattern` (lines 7-9); `mod ext3_format` appended (lines 394-585).

**Interfaces:**

Consumes (from Tasks 1 and 2, on the base branch):
- `fs_core::RegionKind::Journal`; `ext::BlockRole::Journal` (Task 1 already returns `None` for it in `annotate_owned_block`).
- `Superblock` fields `journal_uuid: [u8; 16]`, `journal_inum`, `journal_dev`, `last_orphan`, `default_mount_opts: u32`, `jnl_backup_type: u8`, `jnl_blocks: [u32; 17]`, decoded and encoded at 0xD0, 0xE0, 0xE4, 0xE8, 0x100, 0xFD, 0x10C.
- From `crate::journal` (Task 2): `JOURNAL_INO`, `MIN_JOURNAL_BLOCKS`, `FEATURE_COMPAT_HAS_JOURNAL`, `FEATURE_INCOMPAT_RECOVER`, `JournalMode { Ordered, Data }` with `mount_opts()` and `from_mount_opts(opts) -> Result<JournalMode>` (0x60 is `Unsupported("writeback journaling")`), `JournalOptions { blocks: Option<u32>, mode }` (`Default`: `None`, `Ordered`), `CrashPhase`, `default_journal_blocks(total) -> Option<u32>`, `wrap(index, first, maxlen)`, `JournalSuperblock { .., LEN, START_OFFSET, new(maxlen, uuid), decode(&[u8]) -> Result<Self> (magic only: "journal superblock magic is 0x{:08X}"), encode, validate(data_blocks) (version, s_blocksize, s_maxlen, s_first, features, s_start in that order) }`.
- Existing slice-2 code: `alloc::{AllocCtx, alloc_blocks, contiguous_runs}`, `blockmap::{map_blocks, file_blocks, indirect_blocks, indirect_blocks_needed, MAX_FILE_BLOCKS}`, `Inode`, `Geometry::{inode_location, group_of_block}`, and in `fs.rs` `parse_metadata`, `overlaps`, `run_op`.

Produces (exact):

```rust
// crates/ext/src/superblock.rs
pub struct ExtFormatOptions { pub total_blocks: u32, pub inodes_per_group: Option<u32>, pub label: [u8; 16], pub uuid: [u8; 16], pub journal: Option<JournalOptions> }   // Default: journal None
impl ExtFormatOptions { pub fn ext3() -> Self }   // Default with journal: Some(JournalOptions::default())
// Superblock::validate: compat is 0 or exactly has_journal (other bits: Unsupported("compat features are not supported: {names}"), has_journal not named);
// incompat is filetype, optionally with needs_recovery; needs_recovery without has_journal is
// CorruptImage("needs_recovery is set but the volume has no journal") (checked after unknown incompat bits, before "the filetype feature is required").

// crates/ext/src/journal/state.rs  (re-exported as ext::JournalState, ext::JournalInfo; also ext::journal::state::*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalState {
    pub blocks: Vec<u32>, pub maxlen: u32, pub first: u32, pub sequence: u32, pub head: u32,
    pub mode: JournalMode, pub needs_recovery: bool, pub armed: Option<CrashPhase>,
}
impl JournalState {
    pub fn open(disk: &Disk, sb: &Superblock, geo: &Geometry) -> Result<JournalState>;
    pub fn physical(&self, index: u32) -> u32;          // blocks[index]; panics past maxlen
    pub fn offset(&self, index: u32) -> usize;           // physical(index) * 1024
    pub fn max_transaction(&self) -> u32;                // maxlen / 4
    pub fn superblock(&self, disk: &Disk) -> Result<JournalSuperblock>;
    pub fn wrap(&self, index: u32) -> u32;               // journal::wrap(index, first, maxlen)
}
pub(crate) fn write_journal(fs: &mut ExtFs, options: &JournalOptions) -> Result<JournalState>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalInfo {
    pub inode: u32, pub maxlen: u32, pub first_block: u32, pub sequence: u32, pub start: u32,
    pub head: u32, pub mode: JournalMode, pub needs_recovery: bool, pub max_transaction: u32,
}

// crates/ext/src/fs.rs
pub struct ExtFs { pub(crate) disk, pub(crate) sb, pub(crate) gds, pub(crate) geo, pub(crate) history, pub(crate) now, pub(crate) corrupt, pub(crate) journal: Option<JournalState> }
pub(crate) fn stamp(now: DateTime) -> u32;               // was private; state.rs uses it
impl ExtFs {
    pub fn fs_type(&self) -> &'static str;               // "ext3" when journal.is_some(), else "ext2"
    pub fn needs_recovery(&self) -> bool;                // journal.needs_recovery; false on ext2
    pub fn journal_info(&self) -> Option<JournalInfo>;   // start read from the disk's journal superblock (be32 at 0x1C), the rest from JournalState
    pub fn journal_mode(&self) -> Option<JournalMode>;
}
```

Behaviour later tasks rely on (pinned by this task's tests):
- `JournalState::open` messages, first failure wins, in this order: `Unsupported("journal inode {n} (only the reserved inode 8 is supported)")`; `Unsupported("external journal")` (nonzero `s_journal_uuid` or `s_journal_dev`); `CorruptImage("journal inode 8 is not a regular file (mode 0x{:04X})")`; `CorruptImage("journal inode 8 has size {n}, not a whole number of blocks")`; `CorruptImage("journal inode 8 maps only {k} of its {n} blocks inside the volume")` (a zero pointer, a pointer past the disk, or a block at or past `s_blocks_count`); `Unsupported("journal of {n} blocks is below the 1024-block minimum")`; then `JournalSuperblock::decode` (magic) and `validate` for items 3 and 4 (run with `s_start` masked to 0), `JournalMode::from_mount_opts` (item 5), and `validate` again (item 6, `s_start`). On success: `blocks` = inode 8's data blocks in logical order (`len == maxlen`), `first = s_first`, `sequence = s_sequence`, `head = s_first`, `needs_recovery` = the ext superblock's incompat 0x0004, `armed = None`.
- Format: data blocks from the group of block `(total_blocks - first_data_block) / 2`, then the pointer blocks from the same goal (`blockmap::map_blocks`), so on the default disk data 82..1105 and pointers 1106 (IND), 1107 (DIND), 1108..1110 (second level). Inode 8: mode 0x8180, uid/gid/flags 0, size `maxlen * 1024`, links 1, `i_blocks` = (data + pointer blocks) * 2, atime/ctime/mtime = format's stamp. Journal superblock = `JournalSuperblock::new(maxlen, volume uuid)`; indexes 1.. are zero. Ext superblock (every copy): compat 0x0004, `journal_inum` 8, `default_mount_opts` 0x0040 (ordered) or 0x0020 (data), `jnl_backup_type` 1, `jnl_blocks[..15]` = `i_block`, `[15]` 0, `[16]` `i_size`. Format still writes outside any operation: `history()` is empty.
- Size rules (`InvalidGeometry`, checked before inode 8 is written): below 2048 total blocks, with or without an explicit size, `"a journal needs at least 2048 blocks"`; explicit below 1024, `"journal size must be at least 1024 blocks"`; explicit above half the free blocks of the fresh ext2 layout (8,117 on the default disk) or above `blockmap::MAX_FILE_BLOCKS`, or any size whose data plus pointer blocks exceed the free blocks, `"journal size too big for the volume"`.
- `layout()`: each maximal run of the journal's (sorted) data blocks is a `Region { name: "journal" (or "journal (part k)" from 1 when there are several runs), kind: RegionKind::Journal }` placed in disk order inside the group's data region, which splits into `data (group n)` pieces before and after it (both keep that name; empty pieces are dropped). Journal pointer blocks stay in the data region. The regions come from the last good `JournalState` while the volume is corrupt.
- `block_owners()`: the tree walk as before, then every `JournalState.blocks` entry as `BlockOwner { inode: 8, path: "<journal>", role: BlockRole::Journal }` and every pointer block of inode 8 as `role: BlockRole::Indirect` (journal entries are inserted last, so they win over a cross-linked tree entry).
- The gate: `write_raw` computes the watched ranges before writing: the primary superblock, the primary descriptor table, and with a journal the 1024 bytes of journal index 0, inode 8's 128-byte slot, and each pointer block inode 8 maps. A write overlapping any of them re-runs `parse_metadata` and then `JournalState::open` when `has_journal` is set; success adopts `sb`, `geo`, `gds`, and `journal` (`None` when `has_journal` was cleared, so the volume becomes ext2) and keeps the old `head` and `armed` when `maxlen` is unchanged; failure sets `corruption()` to `CorruptImage("superblock or group descriptors no longer parse after a raw write: {cause}")` (the slice-2 shape; `cause` is the `CorruptImage` text, or the `Display` of any other error) and keeps every last good value. An ext2 volume watches exactly the two slice-2 ranges.
- Test helpers in `crates/ext/tests/common/mod.rs`: `ext3(mode: JournalMode) -> ExtFs` (default disk, mke2fs journal size) and `pattern(len) -> Vec<u8>` (`(i % 251) as u8`), under `#![allow(dead_code)]`. Tasks 4 and 5 append theirs there. Both `tests/ext3.rs` and `tests/e2fsprogs.rs` declare `mod common;`; `e2fsprogs.rs` imports `common::pattern` at its root (so its modules' `use super::pattern` still resolves) and has no copy of any `common` helper. Its `ext3_format` module adds only `ext3_with_blocks(total_blocks, mode)` for the non-default disk sizes.
- `crates/wasm/src/volume.rs`: `enum Inner { Fat(Box<FatFs>), Ext(Box<ExtFs>) }`; construct with `Inner::Ext(Box::new(fs))`; `fs()`/`fs_mut()` use `as_ref()`/`as_mut()`.

Gate commands used throughout (run from the repository root):

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p fs-emulator-wasm --target wasm32-unknown-unknown
```

---

#### Round 1: the format option and the superblock variants

- [ ] **Step 1: Write the failing superblock tests.** In `crates/ext/src/superblock.rs`, `mod tests` starts with these imports; add the `JournalMode` line so they read:

```rust
mod tests {
    use super::*;
    use crate::group::Geometry;
    use crate::journal::JournalMode;
    use fs_core::Error;
```

Then insert these three tests directly above `fn has_superblock_follows_sparse_super_for_groups_0_to_30` (keep its `#[test]` attribute below them):

```rust
    #[test]
    fn validate_accepts_the_ext3_feature_variants() {
        let good = default_superblock();
        let ext3 = Superblock {
            feature_compat: 0x0004,
            ..good.clone()
        };
        assert_eq!(ext3.validate(16_384), Ok(()));
        let recovering = Superblock {
            feature_incompat: 0x0002 | 0x0004,
            ..ext3.clone()
        };
        assert_eq!(recovering.validate(16_384), Ok(()));
        let msg = unsupported_message(&Superblock {
            feature_compat: 0x0004 | 0x0010,
            ..good
        });
        assert_eq!(msg, "compat features are not supported: resize_inode");
        let msg = unsupported_message(&Superblock {
            feature_incompat: 0x0004,
            ..ext3
        });
        assert_eq!(msg, "the filetype feature is required");
    }

    #[test]
    fn validate_calls_needs_recovery_without_a_journal_corrupt() {
        let sb = Superblock {
            feature_incompat: 0x0002 | 0x0004,
            ..default_superblock()
        };
        assert_eq!(
            sb.validate(16_384),
            Err(Error::CorruptImage(
                "needs_recovery is set but the volume has no journal".into()
            ))
        );
    }

    #[test]
    fn options_ext3_is_the_default_disk_with_an_ordered_journal() {
        assert_eq!(ExtFormatOptions::default().journal, None);
        let o = ExtFormatOptions::ext3();
        assert_eq!(
            o.journal,
            Some(JournalOptions {
                blocks: None,
                mode: JournalMode::Ordered
            })
        );
        assert_eq!(
            ExtFormatOptions { journal: None, ..o },
            ExtFormatOptions::default()
        );
    }
```

- [ ] **Step 2: Run them and watch the build fail.**

Run: `cargo test -p ext --lib superblock`
Expected: the build fails with four errors: ``error[E0422]: cannot find struct, variant or union type `JournalOptions` in this scope``, ``error[E0609]: no field `journal` on type `superblock::ExtFormatOptions` ``, ``error[E0599]: no function or associated item named `ext3` found for struct `superblock::ExtFormatOptions` in the current scope``, and ``error[E0560]: struct `superblock::ExtFormatOptions` has no field named `journal` ``.

- [ ] **Step 3: Add the option and the variants.** In `crates/ext/src/superblock.rs`, replace the imports:

```rust
use crate::group::Geometry;
use fs_core::{Error, Result};
```

with:

```rust
use crate::group::Geometry;
use crate::journal::{JournalOptions, FEATURE_COMPAT_HAS_JOURNAL, FEATURE_INCOMPAT_RECOVER};
use fs_core::{Error, Result};
```

Replace the options struct and its `Default`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtFormatOptions {
    pub total_blocks: u32,
    /// `None` derives one inode per 16 KiB (`default_inodes_per_group`).
    pub inodes_per_group: Option<u32>,
    /// The volume name, NUL-padded.
    pub label: [u8; 16],
    pub uuid: [u8; 16],
}

impl Default for ExtFormatOptions {
    fn default() -> Self {
        Self {
            total_blocks: 16_384,
            inodes_per_group: None,
            label: [0; 16],
            uuid: DEFAULT_UUID,
        }
    }
}
```

with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtFormatOptions {
    pub total_blocks: u32,
    /// `None` derives one inode per 16 KiB (`default_inodes_per_group`).
    pub inodes_per_group: Option<u32>,
    /// The volume name, NUL-padded.
    pub label: [u8; 16],
    pub uuid: [u8; 16],
    /// `None` formats ext2; `Some` adds the ext3 journal on inode 8.
    pub journal: Option<JournalOptions>,
}

impl Default for ExtFormatOptions {
    fn default() -> Self {
        Self {
            total_blocks: 16_384,
            inodes_per_group: None,
            label: [0; 16],
            uuid: DEFAULT_UUID,
            journal: None,
        }
    }
}

impl ExtFormatOptions {
    /// The default disk as ext3: an ordered-mode journal of mke2fs's size.
    pub fn ext3() -> Self {
        Self {
            journal: Some(JournalOptions::default()),
            ..Self::default()
        }
    }
}
```

Replace the `validate` doc comment:

```rust
    /// Check that this crate can mount the superblock of a disk holding
    /// `disk_blocks` 1 KiB blocks. The magic is checked first, then the
    /// revision, block size, inode size, and features (`Unsupported`), then
    /// the geometry (`CorruptImage`).
```

with:

```rust
    /// Check that this crate can mount the superblock of a disk holding
    /// `disk_blocks` 1 KiB blocks. The magic is checked first, then the
    /// revision, block size, inode size, and features (`Unsupported`), then
    /// the geometry (`CorruptImage`). The features are ext2's (`filetype`,
    /// `sparse_super`), plus `has_journal` on ext3 and `needs_recovery`
    /// while a transaction is in flight; `needs_recovery` without
    /// `has_journal` is `CorruptImage`.
```

and, inside `validate`, replace the compat and incompat checks:

```rust
        if self.feature_compat != 0 {
            return Err(Error::Unsupported(format!(
                "compat features are not supported: {}",
                feature_names(self.feature_compat, &COMPAT_NAMES)
            )));
        }
        let incompat = self.feature_incompat & !FEATURE_INCOMPAT_FILETYPE;
        if incompat != 0 {
            return Err(Error::Unsupported(format!(
                "incompatible feature {}",
                feature_names(incompat, &INCOMPAT_NAMES)
            )));
        }
        if self.feature_incompat != FEATURE_INCOMPAT_FILETYPE {
```

with:

```rust
        let compat = self.feature_compat & !FEATURE_COMPAT_HAS_JOURNAL;
        if compat != 0 {
            return Err(Error::Unsupported(format!(
                "compat features are not supported: {}",
                feature_names(compat, &COMPAT_NAMES)
            )));
        }
        let incompat =
            self.feature_incompat & !(FEATURE_INCOMPAT_FILETYPE | FEATURE_INCOMPAT_RECOVER);
        if incompat != 0 {
            return Err(Error::Unsupported(format!(
                "incompatible feature {}",
                feature_names(incompat, &INCOMPAT_NAMES)
            )));
        }
        if self.feature_incompat & FEATURE_INCOMPAT_RECOVER != 0
            && self.feature_compat & FEATURE_COMPAT_HAS_JOURNAL == 0
        {
            return Err(Error::CorruptImage(
                "needs_recovery is set but the volume has no journal".into(),
            ));
        }
        if self.feature_incompat & FEATURE_INCOMPAT_FILETYPE == 0 {
```

(the `return Err(Error::Unsupported("the filetype feature is required".into()))` body that follows is unchanged). In `crates/wasm/src/dto.rs`, `impl TryFrom<ExtFormatOptions> for ext::ExtFormatOptions` builds every field; add the new one after `uuid`, so the end of the struct literal reads:

```rust
            uuid: match o.uuid {
                Some(u) => parse_uuid(&u)?,
                None => d.uuid,
            },
            journal: d.journal,
        })
```

- [ ] **Step 4: Run the superblock tests.**

Run: `cargo test -p ext --lib superblock`
Expected: `test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 62 filtered out`.

Run: `cargo build -p fs-emulator-wasm`
Expected: `Finished` (the wasm crate still compiles).

#### Round 2: `JournalState`, the format-time writer, and loading

- [ ] **Step 5: Create the shared test helpers.** Create `crates/ext/tests/common/mod.rs`:

```rust
#![allow(dead_code)]
//! Helpers the ext3 integration tests share. Each test binary that says
//! `mod common;` compiles all of them, so the ones it does not use would
//! otherwise be dead code.

use ext::{ExtFormatOptions, ExtFs, JournalMode, JournalOptions};

/// The default 16 MiB disk formatted as ext3 in `mode`, with mke2fs's
/// journal size (1024 blocks).
pub fn ext3(mode: JournalMode) -> ExtFs {
    ExtFs::format(ExtFormatOptions {
        journal: Some(JournalOptions { blocks: None, mode }),
        ..Default::default()
    })
    .unwrap()
}

/// `len` bytes of the repeating pattern `i % 251`.
pub fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}
```

- [ ] **Step 6: Write the format, load, size, ext2, and validation tests.** Create `crates/ext/tests/ext3.rs` with exactly this content (Rounds 3 and 4 append more tests before the module's closing brace). `inode_8_is_never_exposed_by_the_tree` covers spec section 7's last bullet: the journal has no directory entry, so no read method reaches inode 8:

```rust
//! ext3: the journal on inode 8 (spec `2026-09-24-ext3-journal-design.md`).

mod common;

/// Task 3: format, load, validation, layout, owners, and the gate.
mod format {
    use super::common::ext3;
    use ext::blockmap;
    use ext::{
        BlockRole, ExtFormatOptions, ExtFs, JournalInfo, JournalMode, JournalOptions,
        JournalSuperblock, Superblock, DEFAULT_UUID,
    };
    use fs_core::{Error, RegionKind};

    /// 1980-01-01 00:00:00 UTC, the default `now`, as ext stores it.
    const EPOCH_1980: u32 = 315_532_800;
    /// The journal superblock (journal index 0) on the default disk.
    const JSB: usize = 82 * 1024;
    /// Inode 8's slot: the inode table starts at block 5, 128 bytes each.
    const INODE_8: usize = 5 * 1024 + 7 * 128;
    const BOTH: [JournalMode; 2] = [JournalMode::Ordered, JournalMode::Data];
    /// FNV-1a (64-bit) of the default ext2 image.
    const EXT2_IMAGE_FNV: u64 = 1_437_322_056_522_707_274;

    fn put_u32(image: &mut [u8], offset: usize, value: u32) {
        image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_be32(image: &mut [u8], offset: usize, value: u32) {
        image[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }

    fn with_journal(total_blocks: u32, blocks: Option<u32>) -> fs_core::Result<ExtFs> {
        ExtFs::format(ExtFormatOptions {
            total_blocks,
            journal: Some(JournalOptions {
                blocks,
                mode: JournalMode::Ordered,
            }),
            ..Default::default()
        })
    }

    /// The default ordered-mode image with `patch` applied, loaded again.
    fn planted(patch: impl FnOnce(&mut Vec<u8>)) -> fs_core::Result<ExtFs> {
        let mut image = ext3(JournalMode::Ordered).disk().as_bytes().to_vec();
        patch(&mut image);
        ExtFs::from_image(image)
    }

    fn unsupported(patch: impl FnOnce(&mut Vec<u8>)) -> String {
        match planted(patch) {
            Err(Error::Unsupported(msg)) => msg,
            other => panic!("expected Unsupported, got {:?}", other.err()),
        }
    }

    fn corrupt(patch: impl FnOnce(&mut Vec<u8>)) -> String {
        match planted(patch) {
            Err(Error::CorruptImage(msg)) => msg,
            other => panic!("expected CorruptImage, got {:?}", other.err()),
        }
    }

    #[test]
    fn format_takes_1029_blocks_from_group_0_in_both_modes() {
        for mode in BOTH {
            let fs = ext3(mode);
            let sb = fs.superblock();
            assert_eq!(sb.free_blocks_count, 15_205, "{mode:?}");
            assert_eq!(sb.free_inodes_count, 1_013);
            let gds = fs.group_descriptors();
            assert_eq!(
                (
                    gds[0].free_blocks_count,
                    gds[0].free_inodes_count,
                    gds[0].used_dirs_count
                ),
                (7_082, 501, 2)
            );
            assert_eq!(
                (
                    gds[1].free_blocks_count,
                    gds[1].free_inodes_count,
                    gds[1].used_dirs_count
                ),
                (8_123, 512, 0)
            );
            assert!(fs.history().is_empty());
            assert_eq!(fs.fs_type(), "ext3");
            assert!(!fs.needs_recovery());
            assert_eq!(fs.journal_mode(), Some(mode));
            // Group 0's block bitmap: blocks 1..=1110 used, 1111 free.
            let bits = fs.disk().sector(3);
            let used = |block: usize| bits[(block - 1) / 8] & (1 << ((block - 1) % 8)) != 0;
            assert!((1..=1_110).all(used));
            assert!(!used(1_111));
        }
    }

    #[test]
    fn inode_8_maps_82_to_1105_with_pointer_blocks_1106_to_1110() {
        let fs = ext3(JournalMode::Ordered);
        let inode = fs.inode(8).unwrap();
        assert_eq!(inode.mode, 0x8180);
        assert_eq!((inode.uid, inode.gid, inode.flags), (0, 0, 0));
        assert_eq!(inode.size, 1_024 * 1_024);
        assert_eq!(inode.links_count, 1);
        assert_eq!(inode.blocks, 1_029 * 2);
        assert_eq!(
            (inode.atime, inode.ctime, inode.mtime, inode.dtime),
            (EPOCH_1980, EPOCH_1980, EPOCH_1980, 0)
        );
        assert_eq!(
            blockmap::file_blocks(fs.disk(), &inode),
            (82..1_106).collect::<Vec<u32>>()
        );
        assert_eq!(
            blockmap::indirect_blocks(fs.disk(), &inode),
            vec![(1_106, 1), (1_107, 2), (1_108, 1), (1_109, 1), (1_110, 1)]
        );
    }

    #[test]
    fn journal_index_0_holds_the_format_time_superblock_and_the_log_is_zero() {
        let fs = ext3(JournalMode::Data);
        let jsb = JournalSuperblock::decode(fs.disk().sector(82)).unwrap();
        assert_eq!(jsb, JournalSuperblock::new(1_024, DEFAULT_UUID));
        assert_eq!(
            &fs.disk().sector(82)[..0x20],
            &[
                0xC0, 0x3B, 0x39, 0x98, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 4, 0, //
                0, 0, 4, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0,
            ]
        );
        assert!((83..1_106).all(|b| fs.disk().sector(b).iter().all(|&x| x == 0)));
    }

    #[test]
    fn every_superblock_copy_carries_the_journal_fields() {
        for (mode, opts) in [(JournalMode::Ordered, 0x0040), (JournalMode::Data, 0x0020)] {
            let fs = ext3(mode);
            let inode = fs.inode(8).unwrap();
            for (block, group) in [(1u64, 0u16), (8_193, 1)] {
                let sb = Superblock::decode(fs.disk().sector(block));
                assert_eq!(sb.block_group_nr, group);
                assert_eq!(
                    Superblock {
                        block_group_nr: 0,
                        ..sb.clone()
                    },
                    *fs.superblock()
                );
                assert_eq!(sb.feature_compat, 0x0004);
                assert_eq!(sb.feature_incompat, 0x0002);
                assert_eq!(sb.feature_ro_compat, 0x0001);
                assert_eq!(sb.journal_inum, 8);
                assert_eq!(sb.journal_uuid, [0; 16]);
                assert_eq!((sb.journal_dev, sb.last_orphan), (0, 0));
                assert_eq!(sb.default_mount_opts, opts, "{mode:?}");
                assert_eq!(sb.jnl_backup_type, 1);
                assert_eq!(sb.jnl_blocks[..15], inode.block);
                assert_eq!((sb.jnl_blocks[15], sb.jnl_blocks[16]), (0, 1_048_576));
                assert_eq!(
                    (sb.free_blocks_count, sb.free_inodes_count),
                    (15_205, 1_013)
                );
            }
            // The raw bytes of the primary copy.
            let raw = fs.disk().sector(1);
            assert_eq!(&raw[0x5C..0x60], &[4, 0, 0, 0]);
            assert_eq!(&raw[0xE0..0xE4], &[8, 0, 0, 0]);
            assert_eq!(raw[0xFD], 1);
            assert_eq!(&raw[0x100..0x104], &[opts as u8, 0, 0, 0]);
            assert_eq!(&raw[0x10C..0x110], &82u32.to_le_bytes());
            assert_eq!(&raw[0x13C..0x140], &1_106u32.to_le_bytes());
            assert_eq!(&raw[0x140..0x144], &1_107u32.to_le_bytes());
            assert_eq!(&raw[0x14C..0x150], &1_048_576u32.to_le_bytes());
            // The backup descriptor table matches the primary one.
            assert_eq!(fs.disk().sector(2), fs.disk().sector(8_194));
        }
    }

    #[test]
    fn journal_info_describes_the_fresh_journal() {
        for mode in BOTH {
            assert_eq!(
                ext3(mode).journal_info(),
                Some(JournalInfo {
                    inode: 8,
                    maxlen: 1_024,
                    first_block: 82,
                    sequence: 1,
                    start: 0,
                    head: 1,
                    mode,
                    needs_recovery: false,
                    max_transaction: 256,
                })
            );
        }
    }

    #[test]
    fn from_image_opens_the_journal_it_formatted() {
        for mode in BOTH {
            let fs = ext3(mode);
            let again = ExtFs::from_image(fs.disk().as_bytes().to_vec()).unwrap();
            assert_eq!(again.fs_type(), "ext3");
            assert_eq!(again.journal_info(), fs.journal_info());
            assert_eq!(again.journal_mode(), Some(mode));
            assert_eq!(again.layout(), fs.layout());
            assert_eq!(again.block_owners(), fs.block_owners());
            assert!(again.corruption().is_none());
        }
    }

    #[test]
    fn a_set_needs_recovery_flag_loads_without_replaying() {
        let mut image = ext3(JournalMode::Ordered).disk().as_bytes().to_vec();
        put_u32(&mut image, 1_024 + 0x60, 0x0006);
        let fs = ExtFs::from_image(image.clone()).unwrap();
        assert!(fs.needs_recovery());
        assert!(fs.journal_info().unwrap().needs_recovery);
        assert_eq!(fs.disk().as_bytes(), &image[..]);
        assert!(fs.history().is_empty());
    }

    #[test]
    fn inode_8_is_never_exposed_by_the_tree() {
        for mode in BOTH {
            let fs = ext3(mode);
            let names: Vec<String> = fs
                .list_dir("/")
                .unwrap()
                .into_iter()
                .map(|e| e.name)
                .collect();
            assert_eq!(names, ["lost+found"]);
            for dir in ["/", "/lost+found"] {
                let inodes: Vec<u32> = fs
                    .dir_entries(dir)
                    .unwrap()
                    .into_iter()
                    .map(|(_, _, entry)| entry.inode)
                    .collect();
                assert!(!inodes.contains(&8), "{dir} lists {inodes:?}");
                assert_ne!(fs.lookup(dir), Ok(8));
            }
            for path in ["/<journal>", "/<8>", "/journal"] {
                assert_eq!(fs.lookup(path), Err(Error::NotFound), "{path}");
                assert_eq!(fs.stat(path), Err(Error::NotFound), "{path}");
                assert_eq!(fs.read_file(path), Err(Error::NotFound), "{path}");
            }
        }
    }

    #[test]
    fn the_journal_size_follows_mke2fs() {
        assert_eq!(
            with_journal(2_047, None).err(),
            Some(Error::InvalidGeometry(
                "a journal needs at least 2048 blocks".into()
            ))
        );
        assert_eq!(
            with_journal(2_047, Some(1_024)).err(),
            Some(Error::InvalidGeometry(
                "a journal needs at least 2048 blocks".into()
            ))
        );
        let small = with_journal(2_048, None).unwrap();
        assert_eq!(small.journal_info().unwrap().maxlen, 1_024);
        let fs = with_journal(4_096, None).unwrap();
        assert_eq!(fs.journal_info().unwrap().maxlen, 1_024);
        assert_eq!(
            with_journal(16_384, Some(1_023)).err(),
            Some(Error::InvalidGeometry(
                "journal size must be at least 1024 blocks".into()
            ))
        );
        // Half the 16,234 free blocks of the fresh ext2 layout is 8,117.
        assert_eq!(
            with_journal(16_384, Some(8_118)).err(),
            Some(Error::InvalidGeometry(
                "journal size too big for the volume".into()
            ))
        );
        let fs = with_journal(16_384, Some(8_117)).unwrap();
        let info = fs.journal_info().unwrap();
        assert_eq!((info.maxlen, info.max_transaction), (8_117, 2_029));
        assert_eq!(fs.inode(8).unwrap().size, 8_117 * 1_024);
    }

    #[test]
    fn a_262144_block_volume_gets_8192_journal_blocks() {
        let fs = with_journal(262_144, None).unwrap();
        let info = fs.journal_info().unwrap();
        assert_eq!(info.maxlen, 8_192);
        // mke2fs's goal: the group of block 131,071, group 15.
        assert_eq!(fs.geometry().group_of_block(info.first_block), 15);
    }

    #[test]
    fn ext2_is_unchanged() {
        let mut fs = ExtFs::format(ExtFormatOptions::default()).unwrap();
        assert_eq!(fs.fs_type(), "ext2");
        assert_eq!(fs.journal_info(), None);
        assert_eq!(fs.journal_mode(), None);
        assert!(!fs.needs_recovery());
        let sb = fs.superblock();
        assert_eq!((sb.feature_compat, sb.journal_inum), (0, 0));
        assert_eq!((sb.default_mount_opts, sb.jnl_backup_type), (0, 0));
        assert_eq!(sb.jnl_blocks, [0; 17]);
        assert_eq!(sb.free_blocks_count, 16_234);
        assert!(fs.disk().sector(1)[0xD0..0x150].iter().all(|&b| b == 0));
        assert!(fs.inode(8).unwrap().is_free());
        assert!(fs
            .block_owners()
            .values()
            .all(|o| o.role != BlockRole::Journal));
        assert!(fs.layout().iter().all(|r| r.kind != RegionKind::Journal));
        // The whole image is the one slice 2 formatted (FNV-1a over every
        // byte, taken on the base branch before this task).
        let hash = fs
            .disk()
            .as_bytes()
            .iter()
            .fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
                (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
            });
        assert_eq!(hash, EXT2_IMAGE_FNV);
        // A raw write where ext3 keeps its journal superblock is plain data.
        fs.write_raw(JSB as u64, &[0; 4]).unwrap();
        assert!(fs.corruption().is_none());
    }

    #[test]
    fn validate_accepts_only_the_ext3_feature_variants() {
        let msg = unsupported(|i| put_u32(i, 1_024 + 0x5C, 0x0004 | 0x0010));
        assert_eq!(msg, "compat features are not supported: resize_inode");
        let mut ext2 = ExtFs::format(ExtFormatOptions::default())
            .unwrap()
            .disk()
            .as_bytes()
            .to_vec();
        put_u32(&mut ext2, 1_024 + 0x60, 0x0006);
        assert_eq!(
            ExtFs::from_image(ext2).err(),
            Some(Error::CorruptImage(
                "needs_recovery is set but the volume has no journal".into()
            ))
        );
    }

    #[test]
    fn an_external_or_foreign_journal_inode_is_unsupported() {
        assert_eq!(
            unsupported(|i| put_u32(i, 1_024 + 0xE0, 9)),
            "journal inode 9 (only the reserved inode 8 is supported)"
        );
        assert_eq!(unsupported(|i| i[1_024 + 0xD0] = 1), "external journal");
        assert_eq!(
            unsupported(|i| put_u32(i, 1_024 + 0xE4, 0x0801)),
            "external journal"
        );
    }

    #[test]
    fn a_damaged_journal_inode_is_corrupt_or_too_small() {
        assert_eq!(
            corrupt(|i| i[INODE_8 + 1] = 0x41),
            "journal inode 8 is not a regular file (mode 0x4180)"
        );
        assert_eq!(
            corrupt(|i| put_u32(i, INODE_8 + 4, 1_048_577)),
            "journal inode 8 has size 1048577, not a whole number of blocks"
        );
        assert_eq!(
            corrupt(|i| put_u32(i, INODE_8 + 40 + 5 * 4, 0)),
            "journal inode 8 maps only 5 of its 1024 blocks inside the volume"
        );
        assert_eq!(
            corrupt(|i| put_u32(i, INODE_8 + 40 + 12 * 4, 20_000)),
            "journal inode 8 maps only 12 of its 1024 blocks inside the volume"
        );
        assert_eq!(
            unsupported(|i| put_u32(i, INODE_8 + 4, 1_023 * 1_024)),
            "journal of 1023 blocks is below the 1024-block minimum"
        );
    }

    #[test]
    fn the_journal_superblock_is_checked_field_by_field() {
        assert_eq!(
            corrupt(|i| put_be32(i, JSB, 0)),
            "journal superblock magic is 0x00000000"
        );
        assert_eq!(
            unsupported(|i| put_be32(i, JSB + 0x04, 3)),
            "journal superblock version 3"
        );
        assert_eq!(
            corrupt(|i| put_be32(i, JSB + 0x0C, 4_096)),
            "journal superblock s_blocksize is 4096, not 1024"
        );
        assert_eq!(
            corrupt(|i| put_be32(i, JSB + 0x10, 1_000)),
            "journal superblock s_maxlen is 1000 but the journal inode maps 1024 blocks"
        );
        assert_eq!(
            corrupt(|i| put_be32(i, JSB + 0x14, 2)),
            "journal superblock s_first is 2, not 1"
        );
        assert_eq!(
            unsupported(|i| put_be32(i, JSB + 0x24, 0x1)),
            "journal feature journal_checksum"
        );
        assert_eq!(
            unsupported(|i| put_be32(i, JSB + 0x28, 0x2)),
            "journal feature journal_64bit"
        );
        assert_eq!(
            unsupported(|i| put_be32(i, JSB + 0x28, 0x10)),
            "journal feature journal_checksum_v3"
        );
        assert_eq!(
            corrupt(|i| put_be32(i, JSB + 0x1C, 1_024)),
            "journal superblock s_start is 1024, outside the log 1..1024"
        );
    }

    #[test]
    fn the_mount_options_choose_the_mode_and_refuse_writeback() {
        assert_eq!(
            unsupported(|i| put_u32(i, 1_024 + 0x100, 0x0060)),
            "writeback journaling"
        );
        let fs = planted(|i| put_u32(i, 1_024 + 0x100, 0)).unwrap();
        assert_eq!(fs.journal_mode(), Some(JournalMode::Ordered));
        let fs = planted(|i| put_u32(i, 1_024 + 0x100, 0x0020 | 0x000C)).unwrap();
        assert_eq!(fs.journal_mode(), Some(JournalMode::Data));
    }

    #[test]
    fn the_checks_run_in_the_spec_order() {
        // Item 1 before item 2: a foreign inode number hides a broken inode 8.
        assert_eq!(
            unsupported(|i| {
                put_u32(i, 1_024 + 0xE0, 9);
                i[INODE_8 + 1] = 0x41;
            }),
            "journal inode 9 (only the reserved inode 8 is supported)"
        );
        // Item 2 before item 3.
        assert_eq!(
            corrupt(|i| {
                put_u32(i, INODE_8 + 4, 1_048_577);
                put_be32(i, JSB, 0);
            }),
            "journal inode 8 has size 1048577, not a whole number of blocks"
        );
        // Item 4 before item 5, item 5 before item 6.
        assert_eq!(
            unsupported(|i| {
                put_be32(i, JSB + 0x28, 0x1);
                put_u32(i, 1_024 + 0x100, 0x0060);
            }),
            "journal feature journal_incompat_revoke"
        );
        assert_eq!(
            unsupported(|i| {
                put_u32(i, 1_024 + 0x100, 0x0060);
                put_be32(i, JSB + 0x1C, 5_000);
            }),
            "writeback journaling"
        );
    }
}
```

`EXT2_IMAGE_FNV` is the FNV-1a hash of `ExtFs::format(ExtFormatOptions::default())`'s image computed on the base branch before this task; it must not change.

- [ ] **Step 7: Run them and watch the build fail.**

Run: `cargo test -p ext --test ext3`
Expected: the build fails with 18 errors: ``error[E0432]: unresolved import `ext::JournalInfo` `` once, then ``error[E0599]: no method named `journal_info` found for struct `ExtFs` in the current scope`` (9), the same for `journal_mode` (5) and `needs_recovery` (3).

- [ ] **Step 8: Create `JournalState`, `open`, the format writer, and `JournalInfo`.** Create `crates/ext/src/journal/state.rs`:

```rust
//! The journal as a mounted volume sees it: `JournalState` (the resolved
//! block map and the session's sequence, head, mode, and flags), opening it
//! from an image with the checks of spec section 4, the format-time writer
//! of spec sections 2 and 3, and the `JournalInfo` summary.

use super::{
    default_journal_blocks, wrap, CrashPhase, JournalMode, JournalOptions, JournalSuperblock,
    FEATURE_COMPAT_HAS_JOURNAL, FEATURE_INCOMPAT_RECOVER, JOURNAL_INO, MIN_JOURNAL_BLOCKS,
};
use crate::alloc::{self, AllocCtx};
use crate::blockmap;
use crate::fs::{stamp, ExtFs};
use crate::group::Geometry;
use crate::inode::Inode;
use crate::superblock::{Superblock, BLOCK_SIZE};
use fs_core::{Disk, Error, Result};

const BS: usize = BLOCK_SIZE as usize;
/// `0100600`: a regular file, rw-------, as mke2fs makes the journal inode.
const MODE_JOURNAL: u16 = 0x8180;
/// `s_jnl_backup_type` when `s_jnl_blocks` holds the journal inode's
/// `i_block` (e2fsprogs' `EXT3_JNL_BACKUP_BLOCKS`).
const JNL_BACKUP_BLOCKS: u8 = 1;

/// The journal of a mounted ext3 volume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalState {
    /// The physical block of journal index `i`; `len() == maxlen`.
    pub blocks: Vec<u32>,
    pub maxlen: u32,
    /// The first log index (`s_first`, 1).
    pub first: u32,
    /// The tid the next transaction takes.
    pub sequence: u32,
    /// The journal index the next transaction starts at.
    pub head: u32,
    pub mode: JournalMode,
    /// The ext superblock's `needs_recovery` flag.
    pub needs_recovery: bool,
    pub armed: Option<CrashPhase>,
}

impl JournalState {
    /// Open the journal of a volume whose superblock sets `has_journal`,
    /// checking spec section 4 items 1 to 6 in order and returning the
    /// first failure. Never replays: `sequence` is `s_sequence`, `head` is
    /// `s_first`, nothing is armed.
    pub fn open(disk: &Disk, sb: &Superblock, geo: &Geometry) -> Result<JournalState> {
        // 1. An internal journal on inode 8.
        if sb.journal_inum != JOURNAL_INO {
            return Err(Error::Unsupported(format!(
                "journal inode {} (only the reserved inode 8 is supported)",
                sb.journal_inum
            )));
        }
        if sb.journal_uuid != [0; 16] || sb.journal_dev != 0 {
            return Err(Error::Unsupported("external journal".into()));
        }
        // 2. Inode 8: a regular file of whole blocks, mapped inside the volume.
        let (block, offset) = geo.inode_location(JOURNAL_INO);
        let inode = Inode::decode(disk.read(block as usize * BS + offset, Inode::SIZE));
        if !inode.is_file() {
            return Err(Error::CorruptImage(format!(
                "journal inode 8 is not a regular file (mode 0x{:04X})",
                inode.mode
            )));
        }
        if !inode.size.is_multiple_of(BLOCK_SIZE) {
            return Err(Error::CorruptImage(format!(
                "journal inode 8 has size {}, not a whole number of blocks",
                inode.size
            )));
        }
        // `file_blocks` stops at the first pointer that is zero or past the
        // disk, so a short list is a hole or a block outside the volume.
        let want = inode.size / BLOCK_SIZE;
        let mut blocks = blockmap::file_blocks(disk, &inode);
        let inside = blocks.iter().take_while(|&&b| b < geo.total_blocks).count() as u32;
        if inside < want {
            return Err(Error::CorruptImage(format!(
                "journal inode 8 maps only {inside} of its {want} blocks inside the volume"
            )));
        }
        if want < MIN_JOURNAL_BLOCKS {
            return Err(Error::Unsupported(format!(
                "journal of {want} blocks is below the 1024-block minimum"
            )));
        }
        blocks.truncate(want as usize);
        // 3 and 4. The journal superblock: magic, version, geometry, features.
        let jsb = JournalSuperblock::decode(disk.read(blocks[0] as usize * BS, BS))?;
        JournalSuperblock {
            start: 0,
            ..jsb.clone()
        }
        .validate(want)?;
        // 5. The journal mode bits of the ext superblock.
        let mode = JournalMode::from_mount_opts(sb.default_mount_opts)?;
        // 6. `s_start` (the only check `validate` has left).
        jsb.validate(want)?;
        Ok(JournalState {
            blocks,
            maxlen: want,
            first: jsb.first,
            sequence: jsb.sequence,
            head: jsb.first,
            mode,
            needs_recovery: sb.feature_incompat & FEATURE_INCOMPAT_RECOVER != 0,
            armed: None,
        })
    }

    /// The physical block of journal index `index`. Panics past `maxlen`.
    pub fn physical(&self, index: u32) -> u32 {
        self.blocks[index as usize]
    }

    /// The byte offset of journal index `index` on the disk.
    pub fn offset(&self, index: u32) -> usize {
        self.physical(index) as usize * BS
    }

    /// The most blocks one transaction may tag: jbd2's
    /// `j_max_transaction_buffers`, a quarter of the journal.
    pub fn max_transaction(&self) -> u32 {
        self.maxlen / 4
    }

    /// The journal superblock as the disk holds it now.
    pub fn superblock(&self, disk: &Disk) -> Result<JournalSuperblock> {
        JournalSuperblock::decode(disk.read(self.offset(0), BS))
    }

    /// The ring rule for this journal: `journal::wrap(index, first, maxlen)`.
    pub fn wrap(&self, index: u32) -> u32 {
        wrap(index, self.first, self.maxlen)
    }
}

/// The journal size for `options` on a volume of `total_blocks` whose
/// freshly formatted ext2 layout leaves `free_blocks` free (spec section
/// 3): mke2fs's table, or the explicit size within mke2fs's two bounds.
fn journal_size(total_blocks: u32, free_blocks: u32, options: &JournalOptions) -> Result<u32> {
    let too_big = || Error::InvalidGeometry("journal size too big for the volume".into());
    let Some(default) = default_journal_blocks(total_blocks) else {
        return Err(Error::InvalidGeometry(
            "a journal needs at least 2048 blocks".into(),
        ));
    };
    let blocks = match options.blocks {
        None => default,
        Some(blocks) if blocks < MIN_JOURNAL_BLOCKS => {
            return Err(Error::InvalidGeometry(
                "journal size must be at least 1024 blocks".into(),
            ))
        }
        Some(blocks) if blocks > free_blocks / 2 || blocks > blockmap::MAX_FILE_BLOCKS => {
            return Err(too_big())
        }
        Some(blocks) => blocks,
    };
    // A default-sized journal on a volume whose inode tables leave too
    // little room: the data and pointer blocks must fit.
    if blocks + blockmap::indirect_blocks_needed(blocks) > free_blocks {
        return Err(too_big());
    }
    Ok(blocks)
}

/// Lay the journal down during format (spec sections 2 and 3), after the
/// root and `lost+found` exist: check the size, fill inode 8's slot (inode
/// 8 is already counted used, as inodes 1..=11 are at format), allocate its
/// data blocks from the group of block `(total_blocks - first_data_block) /
/// 2` and then its pointer blocks after them (`blockmap::map_blocks`, the
/// slice-2 rule), and write the journal superblock to journal index 0.
/// Indexes 1 and up are fresh allocations of a zero-filled disk, so they
/// are already zero. The cached superblock gains the section-2 fields and
/// the new counters; `format` then writes every superblock copy.
pub(crate) fn write_journal(fs: &mut ExtFs, options: &JournalOptions) -> Result<JournalState> {
    let maxlen = journal_size(fs.geo.total_blocks, fs.sb.free_blocks_count, options)?;
    let goal = fs
        .geo
        .group_of_block((fs.geo.total_blocks - fs.geo.first_data_block) / 2);
    let secs = stamp(fs.now);
    let mut inode = Inode {
        mode: MODE_JOURNAL,
        links_count: 1,
        size: maxlen * BLOCK_SIZE,
        atime: secs,
        ctime: secs,
        mtime: secs,
        ..Inode::default()
    };
    let blocks = {
        let mut ctx = AllocCtx {
            disk: &mut fs.disk,
            sb: &mut fs.sb,
            gds: &mut fs.gds,
            geo: &fs.geo,
        };
        let blocks = alloc::alloc_blocks(&mut ctx, goal, maxlen)?;
        blockmap::map_blocks(&mut ctx, &mut inode, &blocks, goal)?;
        blocks
    };
    let (block, offset) = fs.geo.inode_location(JOURNAL_INO);
    let mut slot = [0u8; Inode::SIZE];
    inode.encode(&mut slot);
    fs.disk.write(block as usize * BS + offset, &slot);

    let mut jsb = [0u8; JournalSuperblock::LEN];
    JournalSuperblock::new(maxlen, fs.sb.uuid).encode(&mut jsb);
    fs.disk.write(blocks[0] as usize * BS, &jsb);

    let sb = &mut fs.sb;
    sb.feature_compat |= FEATURE_COMPAT_HAS_JOURNAL;
    sb.journal_inum = JOURNAL_INO;
    sb.default_mount_opts = options.mode.mount_opts();
    sb.jnl_backup_type = JNL_BACKUP_BLOCKS;
    sb.jnl_blocks[..15].copy_from_slice(&inode.block);
    sb.jnl_blocks[15] = 0;
    sb.jnl_blocks[16] = inode.size;
    Ok(JournalState {
        blocks,
        maxlen,
        first: 1,
        sequence: 1,
        head: 1,
        mode: options.mode,
        needs_recovery: false,
        armed: None,
    })
}

/// What `ExtFs::journal_info` reports (spec section 7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalInfo {
    /// Always 8.
    pub inode: u32,
    pub maxlen: u32,
    /// The physical block of journal index 0.
    pub first_block: u32,
    /// The tid the next transaction takes.
    pub sequence: u32,
    /// `s_start` as the disk holds it: 0 when the journal is empty.
    pub start: u32,
    pub head: u32,
    pub mode: JournalMode,
    pub needs_recovery: bool,
    /// `maxlen / 4`.
    pub max_transaction: u32,
}
```

In `crates/ext/src/journal/mod.rs`, replace the module doc's last line and the imports:

```rust
//! slices; every JBD2 field is big-endian.

use fs_core::{Error, Result};
use std::fmt;
```

with:

```rust
//! slices; every JBD2 field is big-endian. `state` holds the journal of a
//! mounted volume: opening it, the format-time writer, and `JournalInfo`.

pub mod state;

use fs_core::{Error, Result};
use std::fmt;
```

In `crates/ext/src/lib.rs`, insert between `pub use inode::Inode;` and `pub use journal::{`:

```rust
pub use journal::state::{JournalInfo, JournalState};
```

- [ ] **Step 9: Wire the journal into `ExtFs`.** All in `crates/ext/src/fs.rs`.

Add the journal imports after the `use crate::inode::{..};` block, so the imports read:

```rust
use crate::inode::{
    Inode, MODE_DIR, MODE_LOST_FOUND, MODE_TYPE_DIR, MODE_TYPE_FILE, MODE_TYPE_MASK,
};
use crate::journal::state::{self, JournalInfo, JournalState};
use crate::journal::{JournalMode, JournalSuperblock, FEATURE_COMPAT_HAS_JOURNAL, JOURNAL_INO};
use crate::superblock::{
    ExtFormatOptions, Superblock, BLOCK_SIZE, EXT2_MAGIC, FIRST_INO, LOST_FOUND_INO, ROOT_INO,
    SUPERBLOCK_OFFSET,
};
```

Replace the struct and `stamp`'s signature:

```rust
pub struct ExtFs {
    disk: Disk,
    sb: Superblock,
    gds: Vec<GroupDescriptor>,
    geo: Geometry,
    history: Vec<OpRecord>,
    now: DateTime,
    /// The gate error, or `None` while mounted. `sb`, `gds`, and `geo` keep
    /// their last good values so layout and annotation stay stable.
    corrupt: Option<Error>,
}

/// `now` as the 32-bit seconds ext stores: saturating at both ends, and
/// never below 1 second, so a delete never writes `dtime = 0` (which reads
/// as "not deleted").
fn stamp(now: DateTime) -> u32 {
```

with:

```rust
pub struct ExtFs {
    pub(crate) disk: Disk,
    pub(crate) sb: Superblock,
    pub(crate) gds: Vec<GroupDescriptor>,
    pub(crate) geo: Geometry,
    pub(crate) history: Vec<OpRecord>,
    pub(crate) now: DateTime,
    /// The gate error, or `None` while mounted. `sb`, `gds`, `geo`, and
    /// `journal` keep their last good values so layout and annotation stay
    /// stable.
    pub(crate) corrupt: Option<Error>,
    /// `Some` when the superblock sets `has_journal` (ext3).
    pub(crate) journal: Option<JournalState>,
}

/// `now` as the 32-bit seconds ext stores: saturating at both ends, and
/// never below 1 second, so a delete never writes `dtime = 0` (which reads
/// as "not deleted").
pub(crate) fn stamp(now: DateTime) -> u32 {
```

Insert `open_journal` between `parse_metadata` and `impl ExtFs {`:

```rust
    Ok((sb, geo, gds))
}

/// The journal of a volume whose superblock sets `has_journal` (spec
/// section 4), `None` for ext2. Shared by `from_image` and the gate.
fn open_journal(disk: &Disk, sb: &Superblock, geo: &Geometry) -> Result<Option<JournalState>> {
    if sb.feature_compat & FEATURE_COMPAT_HAS_JOURNAL == 0 {
        return Ok(None);
    }
    JournalState::open(disk, sb, geo).map(Some)
}

impl ExtFs {
```

Replace `format`'s doc comment:

```rust
    /// A freshly formatted volume: every superblock copy and descriptor
    /// table, both bitmaps of every group, zeroed inode tables, inodes 1 to
    /// 10 reserved, the root directory, and `lost+found`. Written outside
    /// any operation, so history starts empty.
    pub fn format(options: ExtFormatOptions) -> Result<ExtFs> {
```

with:

```rust
    /// A freshly formatted volume: every superblock copy and descriptor
    /// table, both bitmaps of every group, zeroed inode tables, inodes 1 to
    /// 10 reserved, the root directory, and `lost+found`; with
    /// `options.journal`, the ext3 journal on inode 8 as well. Written
    /// outside any operation, so history starts empty.
    pub fn format(options: ExtFormatOptions) -> Result<ExtFs> {
```

Replace the end of `format` (from the descriptor table encoding to the end of the function):

```rust
        let mut table = vec![0u8; geo.descriptor_blocks as usize * BS];
        group::encode_table(&gds, &mut table);
        for gl in &geo.groups_layout {
            if let (Some(sb_block), Some(table_block)) = (gl.superblock_block, gl.descriptors_block)
            {
                let mut copy = sb.clone();
                copy.block_group_nr = gl.index as u16;
                let mut bytes = [0u8; Superblock::LEN];
                copy.encode(&mut bytes);
                disk.write(sb_block as usize * BS, &bytes);
                disk.write(table_block as usize * BS, &table);
            }
        }
        Ok(ExtFs {
            disk,
            sb,
            gds,
            geo,
            history: Vec::new(),
            now,
            corrupt: None,
        })
    }
```

with (the copies are now written last, after the journal changed the counters and fields):

```rust
        let mut fs = ExtFs {
            disk,
            sb,
            gds,
            geo,
            history: Vec::new(),
            now,
            corrupt: None,
            journal: None,
        };
        if let Some(journal) = &options.journal {
            fs.journal = Some(state::write_journal(&mut fs, journal)?);
        }
        fs.write_metadata_copies();
        Ok(fs)
    }

    /// Write the cached superblock and descriptor table to every group
    /// that carries a copy (format's last step).
    fn write_metadata_copies(&mut self) {
        let mut table = vec![0u8; self.geo.descriptor_blocks as usize * BS];
        group::encode_table(&self.gds, &mut table);
        for gl in &self.geo.groups_layout {
            if let (Some(sb_block), Some(table_block)) = (gl.superblock_block, gl.descriptors_block)
            {
                let mut copy = self.sb.clone();
                copy.block_group_nr = gl.index as u16;
                let mut bytes = [0u8; Superblock::LEN];
                copy.encode(&mut bytes);
                self.disk.write(sb_block as usize * BS, &bytes);
                self.disk.write(table_block as usize * BS, &table);
            }
        }
    }
```

Replace `from_image`:

```rust
    /// Wrap an existing ext2 image. Trailing bytes past `s_blocks_count`
    /// blocks are dropped; a shorter image is `CorruptImage`, a feature this
    /// crate does not implement is `Unsupported`.
    pub fn from_image(mut bytes: Vec<u8>) -> Result<ExtFs> {
        if bytes.len() < SUPERBLOCK_OFFSET + Superblock::LEN {
            return Err(Error::CorruptImage(format!(
                "image is {} bytes, shorter than the {} that hold the boot block and superblock",
                bytes.len(),
                SUPERBLOCK_OFFSET + Superblock::LEN
            )));
        }
        let (sb, geo, gds) = parse_metadata(&bytes)?;
        bytes.truncate(geo.total_blocks as usize * BS);
        let disk = Disk::from_bytes(BS, bytes)?;
        Ok(ExtFs {
            disk,
            sb,
            gds,
            geo,
            history: Vec::new(),
            now: DateTime::default(),
            corrupt: None,
        })
    }
```

with:

```rust
    /// Wrap an existing ext2 or ext3 image. Trailing bytes past
    /// `s_blocks_count` blocks are dropped; a shorter image is
    /// `CorruptImage`, a feature this crate does not implement is
    /// `Unsupported`. An ext3 journal is opened (spec section 4) but never
    /// replayed, even when the volume needs recovery.
    pub fn from_image(mut bytes: Vec<u8>) -> Result<ExtFs> {
        if bytes.len() < SUPERBLOCK_OFFSET + Superblock::LEN {
            return Err(Error::CorruptImage(format!(
                "image is {} bytes, shorter than the {} that hold the boot block and superblock",
                bytes.len(),
                SUPERBLOCK_OFFSET + Superblock::LEN
            )));
        }
        let (sb, geo, gds) = parse_metadata(&bytes)?;
        bytes.truncate(geo.total_blocks as usize * BS);
        let disk = Disk::from_bytes(BS, bytes)?;
        let journal = open_journal(&disk, &sb, &geo)?;
        Ok(ExtFs {
            disk,
            sb,
            gds,
            geo,
            history: Vec::new(),
            now: DateTime::default(),
            corrupt: None,
            journal,
        })
    }
```

Replace `fs_type`:

```rust
    pub fn fs_type(&self) -> &'static str {
        "ext2"
    }
```

with:

```rust
    /// `"ext3"` when the volume has a journal, else `"ext2"`.
    pub fn fs_type(&self) -> &'static str {
        if self.journal.is_some() {
            "ext3"
        } else {
            "ext2"
        }
    }

    /// Whether the superblock carries `needs_recovery`; false on ext2.
    pub fn needs_recovery(&self) -> bool {
        self.journal.as_ref().is_some_and(|j| j.needs_recovery)
    }

    /// The journal at a glance (spec section 7); `None` on ext2. `start` is
    /// read from the journal superblock on the disk, the rest from the
    /// session's `JournalState`.
    pub fn journal_info(&self) -> Option<JournalInfo> {
        let j = self.journal.as_ref()?;
        let at = j.offset(0) + JournalSuperblock::START_OFFSET;
        let raw = self.disk.read(at, 4);
        Some(JournalInfo {
            inode: JOURNAL_INO,
            maxlen: j.maxlen,
            first_block: j.physical(0),
            sequence: j.sequence,
            start: u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]),
            head: j.head,
            mode: j.mode,
            needs_recovery: j.needs_recovery,
            max_transaction: j.max_transaction(),
        })
    }

    /// The journal mode; `None` on ext2.
    pub fn journal_mode(&self) -> Option<JournalMode> {
        self.journal.as_ref().map(|j| j.mode)
    }
```

- [ ] **Step 10: Box both volume variants in the wasm crate.** `ExtFs` now carries the journal and clippy's `large_enum_variant` rejects `enum Inner` (544 bytes against 312). In `crates/wasm/src/volume.rs` replace:

```rust
enum Inner {
    Fat(FatFs),
    Ext(ExtFs),
}
```

with:

```rust
enum Inner {
    Fat(Box<FatFs>),
    Ext(Box<ExtFs>),
}
```

replace `load`'s body:

```rust
fn load(bytes: Vec<u8>) -> fs_core::Result<Inner> {
    match detect(&bytes) {
        Detected::Ext if has_fat_signature(&bytes) => match ExtFs::from_image(bytes.clone()) {
            Ok(fs) => Ok(Inner::Ext(fs)),
            Err(_) => FatFs::from_image(bytes).map(Inner::Fat),
        },
        Detected::Ext => ExtFs::from_image(bytes).map(Inner::Ext),
        Detected::Fat => FatFs::from_image(bytes).map(Inner::Fat),
        Detected::Unknown => Err(fs_core::Error::Unsupported(
            "no recognisable filesystem signature".into(),
        )),
    }
}
```

with:

```rust
fn load(bytes: Vec<u8>) -> fs_core::Result<Inner> {
    match detect(&bytes) {
        Detected::Ext if has_fat_signature(&bytes) => match ExtFs::from_image(bytes.clone()) {
            Ok(fs) => Ok(Inner::Ext(Box::new(fs))),
            Err(_) => FatFs::from_image(bytes).map(|fs| Inner::Fat(Box::new(fs))),
        },
        Detected::Ext => ExtFs::from_image(bytes).map(|fs| Inner::Ext(Box::new(fs))),
        Detected::Fat => FatFs::from_image(bytes).map(|fs| Inner::Fat(Box::new(fs))),
        Detected::Unknown => Err(fs_core::Error::Unsupported(
            "no recognisable filesystem signature".into(),
        )),
    }
}
```

replace the three accessors:

```rust
    fn fs(&self) -> &dyn FileSystem {
        match &self.inner {
            Inner::Fat(f) => f,
            Inner::Ext(e) => e,
        }
    }

    fn fs_mut(&mut self) -> &mut dyn FileSystem {
        match &mut self.inner {
            Inner::Fat(f) => f,
            Inner::Ext(e) => e,
        }
    }

    /// The FAT volume, or `NotFat` on any other family.
    fn fat(&self) -> Result<&FatFs, JsValue> {
        match &self.inner {
            Inner::Fat(f) => Ok(f),
            Inner::Ext(_) => Err(js_error(NOT_FAT, "not a FAT volume")),
        }
    }
```

with:

```rust
    fn fs(&self) -> &dyn FileSystem {
        match &self.inner {
            Inner::Fat(f) => f.as_ref(),
            Inner::Ext(e) => e.as_ref(),
        }
    }

    fn fs_mut(&mut self) -> &mut dyn FileSystem {
        match &mut self.inner {
            Inner::Fat(f) => f.as_mut(),
            Inner::Ext(e) => e.as_mut(),
        }
    }

    /// The FAT volume, or `NotFat` on any other family.
    fn fat(&self) -> Result<&FatFs, JsValue> {
        match &self.inner {
            Inner::Fat(f) => Ok(f.as_ref()),
            Inner::Ext(_) => Err(js_error(NOT_FAT, "not a FAT volume")),
        }
    }
```

and in `format_fat16` and `format_ext2` replace `inner: Inner::Fat(fs),` with `inner: Inner::Fat(Box::new(fs)),` and `inner: Inner::Ext(fs),` with `inner: Inner::Ext(Box::new(fs)),`. `ext()` keeps `Inner::Ext(e) => Ok(e)` (the `&Box<ExtFs>` derefs to `&ExtFs`), and the tests' `Ok(Inner::Fat(fs)) => assert_eq!(fs.fs_type(), "FAT16")` and `Ok(Inner::Ext(_))` patterns compile unchanged.

- [ ] **Step 11: Run the tests.**

Run: `cargo test -p ext --test ext3`
Expected: `test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

Run: `cargo test -p ext --test ext2`
Expected: `test result: ok. 52 passed; 0 failed` (ext2 unchanged).

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: `Finished` with no warnings.

#### Round 3: the journal region and the `<journal>` owner

- [ ] **Step 12: Write the layout and owner tests.** In `crates/ext/tests/ext3.rs`, insert before the closing `}` of `mod format`, after `the_checks_run_in_the_spec_order` and separated from it by one blank line:

```rust
    #[test]
    fn layout_carves_the_journal_out_of_group_0_data() {
        let fs = ext3(JournalMode::Ordered);
        let got: Vec<(String, u64, u64, RegionKind)> = fs
            .layout()
            .into_iter()
            .map(|r| (r.name, r.sectors.start, r.sectors.end, r.kind))
            .collect();
        let group0: Vec<(String, u64, u64, RegionKind)> = [
            ("inode table (group 0)", 5, 69, RegionKind::Metadata),
            ("data (group 0)", 69, 82, RegionKind::Data),
            ("journal", 82, 1_106, RegionKind::Journal),
            ("data (group 0)", 1_106, 8_193, RegionKind::Data),
            (
                "backup superblock (group 1)",
                8_193,
                8_194,
                RegionKind::Boot,
            ),
        ]
        .into_iter()
        .map(|(n, s, e, k)| (n.to_string(), s, e, k))
        .collect();
        assert_eq!(got.len(), 15);
        assert_eq!(got[5..10], group0[..]);
        assert_eq!(
            got.last().unwrap(),
            &(
                "data (group 1)".to_string(),
                8_261,
                16_384,
                RegionKind::Data
            )
        );
    }

    #[test]
    fn block_owners_list_the_journal_blocks_under_inode_8() {
        let fs = ext3(JournalMode::Ordered);
        let owners = fs.block_owners();
        assert_eq!(owners.len(), 13 + 1_029);
        let journal = |role| ext::BlockOwner {
            inode: 8,
            path: "<journal>".into(),
            role,
        };
        assert!((82..1_106).all(|b| owners[&b] == journal(BlockRole::Journal)));
        assert!((1_106..=1_110).all(|b| owners[&b] == journal(BlockRole::Indirect)));
        assert_eq!(owners[&69].path, "/");
    }
```

- [ ] **Step 13: Run them and watch them fail.**

Run: `cargo test -p ext --test ext3 -- layout_carves block_owners_list`
Expected: `test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 17 filtered out`: `layout_carves_the_journal_out_of_group_0_data` with ``assertion `left == right` failed`` `left: 13` `right: 15`, and `block_owners_list_the_journal_blocks_under_inode_8` with `left: 13` `right: 1042`.

- [ ] **Step 14: Carve the journal region and list its owners.** In `crates/ext/src/fs.rs`, replace `layout` from its doc comment to the end of the function:

```rust
    /// Regions per group in disk order, after the boot block (spec section 7).
    pub fn layout(&self) -> Vec<Region> {
        let g = &self.geo;
        let region = |name: String, start: u32, end: u32, kind: RegionKind| Region {
            name,
            sectors: u64::from(start)..u64::from(end),
            kind,
        };
        let mut regions = vec![region("boot block".into(), 0, 1, RegionKind::Boot)];
        for gl in &g.groups_layout {
            let n = gl.index;
            if let Some(block) = gl.superblock_block {
                let name = if n == 0 {
                    "superblock".to_string()
                } else {
                    format!("backup superblock (group {n})")
                };
                regions.push(region(name, block, block + 1, RegionKind::Boot));
            }
            if let Some(block) = gl.descriptors_block {
                regions.push(region(
                    format!("group descriptors (group {n})"),
                    block,
                    block + g.descriptor_blocks,
                    RegionKind::Metadata,
                ));
            }
            regions.push(region(
                format!("block bitmap (group {n})"),
                gl.block_bitmap,
                gl.block_bitmap + 1,
                RegionKind::AllocationTable,
            ));
            regions.push(region(
                format!("inode bitmap (group {n})"),
                gl.inode_bitmap,
                gl.inode_bitmap + 1,
                RegionKind::AllocationTable,
            ));
            regions.push(region(
                format!("inode table (group {n})"),
                gl.inode_table,
                gl.inode_table + g.inode_table_blocks,
                RegionKind::Metadata,
            ));
            regions.push(region(
                format!("data (group {n})"),
                gl.first_data,
                gl.first_block + gl.block_count,
                RegionKind::Data,
            ));
        }
        regions
    }
```

with `layout` and the new `journal_runs`:

```rust
    /// Regions per group in disk order, after the boot block (spec section
    /// 7). On ext3 each run of journal data blocks is a `journal` region
    /// carved out of the data region it lies in.
    pub fn layout(&self) -> Vec<Region> {
        let g = &self.geo;
        let journal = self.journal_runs();
        let region = |name: String, start: u32, end: u32, kind: RegionKind| Region {
            name,
            sectors: u64::from(start)..u64::from(end),
            kind,
        };
        let mut regions = vec![region("boot block".into(), 0, 1, RegionKind::Boot)];
        for gl in &g.groups_layout {
            let n = gl.index;
            if let Some(block) = gl.superblock_block {
                let name = if n == 0 {
                    "superblock".to_string()
                } else {
                    format!("backup superblock (group {n})")
                };
                regions.push(region(name, block, block + 1, RegionKind::Boot));
            }
            if let Some(block) = gl.descriptors_block {
                regions.push(region(
                    format!("group descriptors (group {n})"),
                    block,
                    block + g.descriptor_blocks,
                    RegionKind::Metadata,
                ));
            }
            regions.push(region(
                format!("block bitmap (group {n})"),
                gl.block_bitmap,
                gl.block_bitmap + 1,
                RegionKind::AllocationTable,
            ));
            regions.push(region(
                format!("inode bitmap (group {n})"),
                gl.inode_bitmap,
                gl.inode_bitmap + 1,
                RegionKind::AllocationTable,
            ));
            regions.push(region(
                format!("inode table (group {n})"),
                gl.inode_table,
                gl.inode_table + g.inode_table_blocks,
                RegionKind::Metadata,
            ));
            // The data region, split around the journal runs inside it.
            let data_end = gl.first_block + gl.block_count;
            let mut at = gl.first_data;
            for &(start, end, ref name) in &journal {
                let (start, end) = (start.max(at), end.min(data_end));
                if start >= end {
                    continue;
                }
                if at < start {
                    regions.push(region(
                        format!("data (group {n})"),
                        at,
                        start,
                        RegionKind::Data,
                    ));
                }
                regions.push(region(name.clone(), start, end, RegionKind::Journal));
                at = end;
            }
            if at < data_end {
                regions.push(region(
                    format!("data (group {n})"),
                    at,
                    data_end,
                    RegionKind::Data,
                ));
            }
        }
        regions
    }

    /// The maximal runs of consecutive physical blocks among the journal's
    /// data blocks, as `(first, end, name)`: `journal`, or `journal (part
    /// k)` from 1 when there is more than one run. Empty on ext2.
    fn journal_runs(&self) -> Vec<(u32, u32, String)> {
        let Some(j) = &self.journal else {
            return Vec::new();
        };
        let mut blocks = j.blocks.clone();
        blocks.sort_unstable();
        blocks.dedup();
        let runs: Vec<(u32, u32)> = alloc::contiguous_runs(&blocks)
            .into_iter()
            .map(|(i, len)| (blocks[i], blocks[i] + len as u32))
            .collect();
        let many = runs.len() > 1;
        runs.into_iter()
            .enumerate()
            .map(|(k, (start, end))| {
                let name = if many {
                    format!("journal (part {})", k + 1)
                } else {
                    "journal".to_string()
                };
                (start, end, name)
            })
            .collect()
    }
```

Replace `block_owners`'s doc comment:

```rust
    /// Which inode and path every directory, data, and indirect block
    /// belongs to, from a walk of the tree from the root. Directories that
    /// do not parse are skipped, so this never fails.
    pub fn block_owners(&self) -> BTreeMap<u32, BlockOwner> {
```

with:

```rust
    /// Which inode and path every directory, data, and indirect block
    /// belongs to, from a walk of the tree from the root. Directories that
    /// do not parse are skipped, so this never fails. On ext3 the journal's
    /// data blocks (role `Journal`) and pointer blocks (`Indirect`) belong
    /// to inode 8 with the path `<journal>`.
    pub fn block_owners(&self) -> BTreeMap<u32, BlockOwner> {
```

and replace its end:

```rust
                stack.push((entry.inode, child));
            }
        }
        owners
    }
```

with:

```rust
                stack.push((entry.inode, child));
            }
        }
        if let Some(j) = &self.journal {
            let owner = |role| BlockOwner {
                inode: JOURNAL_INO,
                path: "<journal>".to_string(),
                role,
            };
            for &block in &j.blocks {
                owners.insert(block, owner(BlockRole::Journal));
            }
            if let Ok(inode) = self.inode(JOURNAL_INO) {
                for (block, _) in blockmap::indirect_blocks(&self.disk, &inode) {
                    owners.insert(block, owner(BlockRole::Indirect));
                }
            }
        }
        owners
    }
```

- [ ] **Step 15: Run the tests.**

Run: `cargo test -p ext --test ext3`
Expected: `test result: ok. 19 passed; 0 failed`.

Run: `cargo test -p ext --test ext2`
Expected: `test result: ok. 52 passed; 0 failed` (`layout_is_the_region_table_of_the_spec` and the owner tests are unchanged on ext2).

#### Round 4: the corruption gate covers the journal

- [ ] **Step 16: Write the gate tests.** In `crates/ext/tests/ext3.rs`, insert before the closing `}` of `mod format`, after `block_owners_list_the_journal_blocks_under_inode_8` and separated from it by one blank line:

```rust
    /// The gate's message for a journal that no longer opens.
    fn gate(cause: &str) -> Option<Error> {
        Some(Error::CorruptImage(format!(
            "superblock or group descriptors no longer parse after a raw write: {cause}"
        )))
    }

    #[test]
    fn a_raw_write_that_breaks_the_journal_superblock_trips_the_gate() {
        let mut fs = ext3(JournalMode::Ordered);
        let info = fs.journal_info();
        let layout = fs.layout();
        fs.write_raw(JSB as u64, &[0; 4]).unwrap();
        assert_eq!(
            fs.corruption().cloned(),
            gate("journal superblock magic is 0x00000000")
        );
        assert!(fs.list_dir("/").is_err());
        assert!(fs.create_file("/x", b"").is_err());
        assert_eq!(fs.layout(), layout, "layout keeps the last good journal");
        fs.write_raw(JSB as u64, &[0xC0, 0x3B, 0x39, 0x98]).unwrap();
        assert!(fs.corruption().is_none());
        assert_eq!(fs.journal_info(), info);
        assert_eq!(fs.list_dir("/").unwrap().len(), 1);
    }

    #[test]
    fn raw_writes_to_inode_8_and_its_pointer_blocks_trip_the_gate() {
        let mut fs = ext3(JournalMode::Ordered);
        fs.write_raw(INODE_8 as u64 + 1, &[0x41]).unwrap();
        assert_eq!(
            fs.corruption().cloned(),
            gate("journal inode 8 is not a regular file (mode 0x4180)")
        );
        fs.write_raw(INODE_8 as u64 + 1, &[0x81]).unwrap();
        assert!(fs.corruption().is_none());

        let pointers = fs.disk().sector(1_106).to_vec();
        fs.write_raw(1_106 * 1_024, &[0; 1_024]).unwrap();
        assert_eq!(
            fs.corruption().cloned(),
            gate("journal inode 8 maps only 12 of its 1024 blocks inside the volume")
        );
        fs.write_raw(1_106 * 1_024, &pointers).unwrap();
        assert!(fs.corruption().is_none());

        // A second-level pointer block of the double-indirect tree.
        fs.write_raw(1_110 * 1_024, &[0; 4]).unwrap();
        assert_eq!(
            fs.corruption().cloned(),
            gate("journal inode 8 maps only 780 of its 1024 blocks inside the volume")
        );
        fs.write_raw(1_110 * 1_024, &862u32.to_le_bytes()).unwrap();
        assert!(fs.corruption().is_none());
    }

    #[test]
    fn raw_writes_to_the_log_or_a_valid_flag_edit_are_adopted() {
        let mut fs = ext3(JournalMode::Ordered);
        fs.write_raw(90 * 1_024, &super::common::pattern(1_024))
            .unwrap();
        assert!(fs.corruption().is_none());
        fs.write_raw(1_024 + 0x60, &0x0006u32.to_le_bytes())
            .unwrap();
        assert!(fs.corruption().is_none());
        assert!(fs.needs_recovery());
        fs.write_raw(1_024 + 0x100, &0x0020u32.to_le_bytes())
            .unwrap();
        assert_eq!(fs.journal_mode(), Some(JournalMode::Data));
        // Clearing has_journal turns the volume into ext2 (and clearing the
        // flag first keeps the superblock valid).
        fs.write_raw(1_024 + 0x60, &0x0002u32.to_le_bytes())
            .unwrap();
        fs.write_raw(1_024 + 0x5C, &0u32.to_le_bytes()).unwrap();
        assert!(fs.corruption().is_none());
        assert_eq!(fs.fs_type(), "ext2");
        assert_eq!(fs.journal_info(), None);
    }
```

In `crates/ext/src/fs.rs`, `mod tests` at the bottom of the file: add the import so its head reads:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::CrashPhase;
```

and insert this test directly above `fn pointer_runs_group_consecutive_blocks_and_zeros` (keep its `#[test]`):

```rust
    #[test]
    fn a_reparse_keeps_the_session_head_and_armed_crash() {
        let mut fs = ExtFs::format(ExtFormatOptions::ext3()).unwrap();
        let journal = fs.journal.as_mut().unwrap();
        journal.head = 7;
        journal.armed = Some(CrashPhase::AfterCommit);
        fs.write_raw(1_024 + 120, b"renamed\0").unwrap();
        assert_eq!(fs.superblock().label(), "renamed");
        let journal = fs.journal.as_ref().unwrap();
        assert_eq!(
            (journal.head, journal.armed),
            (7, Some(CrashPhase::AfterCommit))
        );
    }
```

- [ ] **Step 17: Run them and watch the journal gate tests fail.**

Run: `cargo test -p ext --test ext3 -- the_gate are_adopted`
Expected: `test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 19 filtered out`: the two gate tests with `left: None` against `right: Some(CorruptImage("superblock or group descriptors no longer parse after a raw write: ..."))` (the old gate does not watch the journal), and `raw_writes_to_the_log_or_a_valid_flag_edit_are_adopted` at `assert!(fs.needs_recovery())` (the old re-parse never re-opens the journal).

Run: `cargo test -p ext --lib a_reparse_keeps`
Expected: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 79 filtered out`. This one passes on its first run on purpose: it is a regression pin, not a red test. The old re-parse never touches `journal`, so `head` and `armed` survive it trivially; Step 18's re-parse replaces `journal` with a freshly opened one, and the test pins that it still carries the session's `head` and `armed` over.

- [ ] **Step 18: Extend the gate.** In `crates/ext/src/fs.rs`, replace `primary_metadata`, `reparse_metadata`, and `write_raw`:

```rust
    /// The byte ranges of the primary superblock and descriptor table, from
    /// the last good geometry.
    fn primary_metadata(&self) -> [Range<usize>; 2] {
        let table = self.geo.group(0).descriptors_block.unwrap_or(2) as usize * BS;
        [
            SUPERBLOCK_OFFSET..SUPERBLOCK_OFFSET + Superblock::LEN,
            table..table + self.geo.descriptor_blocks as usize * BS,
        ]
    }

    /// Re-read the primary superblock and descriptor table after a raw write
    /// touched them. A parse that succeeds and fits the disk is adopted (the
    /// disk is the truth); otherwise the last good values stay and the
    /// volume is marked corrupt. Never fails: the bytes stay as written.
    fn reparse_metadata(&mut self) {
        match parse_metadata(self.disk.as_bytes()) {
            Ok((sb, geo, gds)) => {
                self.sb = sb;
                self.geo = geo;
                self.gds = gds;
                self.corrupt = None;
            }
            Err(e) => {
                let cause = match e {
                    Error::CorruptImage(cause) => cause,
                    other => other.to_string(),
                };
                self.corrupt = Some(Error::CorruptImage(format!(
                    "superblock or group descriptors no longer parse after a raw write: {cause}"
                )));
            }
        }
    }

    /// Write bytes anywhere on the disk, journaled like every operation. A
    /// write that touches the primary superblock or descriptor table
    /// re-parses them. Works while the volume is corrupt, so a later write
    /// can repair it.
    pub fn write_raw(&mut self, offset: u64, bytes: &[u8]) -> Result<OpRecord> {
        self.run_op(fs_core::raw_write_op(offset, bytes.len()), |fs| {
            let range = fs_core::raw_write(&mut fs.disk, offset, bytes)?;
            if fs.primary_metadata().iter().any(|m| overlaps(&range, m)) {
                fs.reparse_metadata();
            }
            // Nothing fallible may follow: `run_op` rolls the bytes back on
            // an error, but not `sb`/`gds`/`geo`/`corrupt`.
            Ok(())
        })
    }
```

with:

```rust
    /// The byte ranges a raw write re-parses after: the primary superblock
    /// and descriptor table, from the last good geometry, and on ext3
    /// journal index 0, inode 8's slot, and the journal's pointer blocks
    /// (as inode 8 maps them before the write).
    fn primary_metadata(&self) -> Vec<Range<usize>> {
        let table = self.geo.group(0).descriptors_block.unwrap_or(2) as usize * BS;
        let mut ranges = vec![
            SUPERBLOCK_OFFSET..SUPERBLOCK_OFFSET + Superblock::LEN,
            table..table + self.geo.descriptor_blocks as usize * BS,
        ];
        if let Some(j) = &self.journal {
            ranges.push(j.offset(0)..j.offset(0) + BS);
            let (block, offset) = self.geo.inode_location(JOURNAL_INO);
            let slot = block as usize * BS + offset;
            ranges.push(slot..slot + Inode::SIZE);
            if let Ok(inode) = self.inode(JOURNAL_INO) {
                for (block, _) in blockmap::indirect_blocks(&self.disk, &inode) {
                    ranges.push(block as usize * BS..(block as usize + 1) * BS);
                }
            }
        }
        ranges
    }

    /// Re-read the primary superblock, descriptor table, and (with
    /// `has_journal`) the journal after a raw write touched them. A parse
    /// that succeeds and fits the disk is adopted (the disk is the truth),
    /// keeping the session's journal head and armed crash when the journal
    /// keeps its length; otherwise the last good values stay and the volume
    /// is marked corrupt. Never fails: the bytes stay as written.
    fn reparse_metadata(&mut self) {
        let parsed = parse_metadata(self.disk.as_bytes()).and_then(|(sb, geo, gds)| {
            let journal = open_journal(&self.disk, &sb, &geo)?;
            Ok((sb, geo, gds, journal))
        });
        match parsed {
            Ok((sb, geo, gds, mut journal)) => {
                if let (Some(new), Some(old)) = (journal.as_mut(), self.journal.as_ref()) {
                    if new.maxlen == old.maxlen {
                        new.head = old.head;
                        new.armed = old.armed;
                    }
                }
                self.sb = sb;
                self.geo = geo;
                self.gds = gds;
                self.journal = journal;
                self.corrupt = None;
            }
            Err(e) => {
                let cause = match e {
                    Error::CorruptImage(cause) => cause,
                    other => other.to_string(),
                };
                self.corrupt = Some(Error::CorruptImage(format!(
                    "superblock or group descriptors no longer parse after a raw write: {cause}"
                )));
            }
        }
    }

    /// Write bytes anywhere on the disk, recorded like every operation. A
    /// write that touches the primary superblock or descriptor table, or on
    /// ext3 the journal superblock, inode 8, or a journal pointer block,
    /// re-parses them. Works while the volume is corrupt, so a later write
    /// can repair it.
    pub fn write_raw(&mut self, offset: u64, bytes: &[u8]) -> Result<OpRecord> {
        self.run_op(fs_core::raw_write_op(offset, bytes.len()), |fs| {
            let watched = fs.primary_metadata();
            let range = fs_core::raw_write(&mut fs.disk, offset, bytes)?;
            if watched.iter().any(|m| overlaps(&range, m)) {
                fs.reparse_metadata();
            }
            // Nothing fallible may follow: `run_op` rolls the bytes back on
            // an error, but not `sb`/`gds`/`geo`/`corrupt`.
            Ok(())
        })
    }
```

- [ ] **Step 19: Run the tests.**

Run: `cargo test -p ext --test ext3`
Expected: `test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

Run: `cargo test -p ext --lib`
Expected: `test result: ok. 80 passed; 0 failed` (76 from Tasks 1 and 2 and slice 2, plus the three superblock tests and the re-parse test).

Run: `cargo test -p ext --test ext2`
Expected: `test result: ok. 52 passed; 0 failed` (the slice-2 gate tests still see exactly the two ranges on ext2).

#### Round 5: the e2fsprogs oracle

- [ ] **Step 20: Share the helpers with the oracle and append the ext3 format oracle.** `crates/ext/tests/e2fsprogs.rs` carries its own `pattern`, a copy of `common::pattern`; it now declares the shared module and uses that instead, and the new module takes `common::ext3` for the default disk. Replace:

```rust
use ext::{ExtFormatOptions, ExtFs};
use std::collections::BTreeMap;
```

with:

```rust
mod common;

use common::pattern;
use ext::{ExtFormatOptions, ExtFs};
use std::collections::BTreeMap;
```

and delete these lines together with the blank line that follows them (the slice-2 modules' `use super::pattern` now reaches the import):

```rust
/// `len` bytes of the repeating pattern `i % 251`, for file contents the
/// mutation oracle tests of Tasks 4 and 5 write.
fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}
```

Then, at the end of the file, after one blank line, append the module. `dumpe2fs_describes_the_journal_in_both_modes` checks the format-time lines of spec section 10.3 and `Max transaction length: 1024`, which the amended spec states; the 262,144-block case checks `Total journal size: 8M`:

```rust
/// ext3 Task 3: a fresh ext3 volume as dumpe2fs, debugfs, and e2fsck see it
/// (ext3 spec section 10.3, first bullet).
mod ext3_format {
    use super::common::ext3;
    use super::{assert_clean, both_streams, image_file, run, tool};
    use ext::{ExtFormatOptions, ExtFs, JournalMode, JournalOptions};

    /// `common::ext3` on a disk of `total_blocks` blocks instead of the
    /// default 16,384, for the journal-size cases.
    fn ext3_with_blocks(total_blocks: u32, mode: JournalMode) -> ExtFs {
        ExtFs::format(ExtFormatOptions {
            total_blocks,
            journal: Some(JournalOptions { blocks: None, mode }),
            ..Default::default()
        })
        .unwrap()
    }

    /// The stdout of `tool_name args image`, one entry per line, with every
    /// run of whitespace collapsed to one space: dumpe2fs pads after the
    /// colon (`Journal inode:            8` becomes `Journal inode: 8`).
    /// `None` when the tool is missing.
    fn normalised(fs: &ExtFs, name: &str, tool_name: &str, args: &[&str]) -> Option<Vec<String>> {
        let program = tool(tool_name)?;
        let image = image_file(fs, name);
        let output = run(&program, args, &image);
        std::fs::remove_file(&image).unwrap();
        assert!(
            output.status.success(),
            "{tool_name} {args:?}:\n{}",
            both_streams(&output)
        );
        Some(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
                .collect(),
        )
    }

    fn assert_lines(lines: &[String], want: &[&str], what: &str) {
        for line in want {
            assert!(
                lines.iter().any(|l| l == line),
                "{what}: no line {line:?} in\n{}",
                lines.join("\n")
            );
        }
    }

    #[test]
    fn dumpe2fs_describes_the_journal_in_both_modes() {
        for (mode, opts) in [
            (JournalMode::Ordered, "journal_data_ordered"),
            (JournalMode::Data, "journal_data"),
        ] {
            let fs = ext3(mode);
            let Some(lines) = normalised(&fs, "ext3-dumpe2fs", "dumpe2fs", &["-h"]) else {
                return;
            };
            let mount = format!("Default mount options: {opts}");
            assert_lines(
                &lines,
                &[
                    "Filesystem features: has_journal filetype sparse_super",
                    mount.as_str(),
                    "Free blocks: 15205",
                    "Free inodes: 1013",
                    "Journal inode: 8",
                    "Journal backup: inode blocks",
                    "Journal features: (none)",
                    "Total journal size: 1024k",
                    "Total journal blocks: 1024",
                    "Max transaction length: 1024",
                    "Journal sequence: 0x00000001",
                    "Journal start: 0",
                ],
                &format!("dumpe2fs -h ({mode:?})"),
            );
        }
    }

    #[test]
    fn debugfs_reads_inode_8_as_the_emulator_wrote_it() {
        let fs = ext3(JournalMode::Ordered);
        let Some(lines) = normalised(&fs, "ext3-stat-8", "debugfs", &["-R", "stat <8>"]) else {
            return;
        };
        let text = lines.join("\n");
        for want in [
            "Type: regular Mode: 0600",
            "Size: 1048576",
            "Links: 1 Blockcount: 2058",
            "(0-11):82-93, (IND):1106, (12-267):94-349, (DIND):1107, (IND):1108, \
             (268-523):350-605, (IND):1109, (524-779):606-861, (IND):1110, (780-1023):862-1105",
            "TOTAL: 1029",
        ] {
            assert!(
                text.contains(want),
                "debugfs stat <8>: no {want:?} in\n{text}"
            );
        }
    }

    #[test]
    fn an_mke2fs_ext3_image_loads_with_the_same_journal_geometry() {
        let Some(mke2fs) = tool("mke2fs") else {
            return;
        };
        let dir = std::env::temp_dir().join(format!("ext2-emulator-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let image = dir.join("mke2fs-ext3.img");
        std::fs::write(&image, vec![0u8; 16 * 1024 * 1024]).unwrap();
        let args = [
            "-q",
            "-F",
            "-t",
            "ext3",
            "-b",
            "1024",
            "-I",
            "128",
            "-O",
            "none,has_journal,filetype,sparse_super",
            "-J",
            "size=1",
            "-N",
            "1024",
        ];
        let output = run(&mke2fs, &args, &image);
        let bytes = std::fs::read(&image).unwrap();
        std::fs::remove_file(&image).unwrap();
        assert!(
            output.status.success(),
            "mke2fs:\n{}",
            both_streams(&output)
        );
        let fs = ExtFs::from_image(bytes).unwrap();
        assert_eq!(fs.fs_type(), "ext3");
        assert_eq!(fs.superblock().free_blocks_count, 15_205);
        let info = fs.journal_info().unwrap();
        assert_eq!(
            (info.maxlen, info.first_block, info.sequence, info.start),
            (1_024, 82, 1, 0)
        );
        // mke2fs sets `user_xattr acl` and no journal mode bits: ordered.
        assert_eq!(info.mode, JournalMode::Ordered);
        assert!(!info.needs_recovery);
        let journal_blocks = fs
            .block_owners()
            .values()
            .filter(|o| o.path == "<journal>")
            .count();
        assert_eq!(journal_blocks, 1_029);
    }

    #[test]
    fn e2fsck_calls_both_modes_clean() {
        assert_clean(&ext3(JournalMode::Ordered), "ext3-ordered");
        assert_clean(&ext3(JournalMode::Data), "ext3-data");
    }

    #[test]
    fn a_4096_block_volume_gets_1024_journal_blocks_and_is_clean() {
        let fs = ext3_with_blocks(4_096, JournalMode::Ordered);
        assert_eq!(fs.journal_info().unwrap().maxlen, 1_024);
        assert_clean(&fs, "ext3-4096");
        let Some(lines) = normalised(&fs, "ext3-4096-dumpe2fs", "dumpe2fs", &["-h"]) else {
            return;
        };
        assert_lines(
            &lines,
            &["Total journal blocks: 1024"],
            "dumpe2fs -h (4096)",
        );
    }

    #[test]
    fn a_262144_block_volume_gets_8192_journal_blocks_and_is_clean() {
        let fs = ext3_with_blocks(262_144, JournalMode::Ordered);
        assert_eq!(fs.journal_info().unwrap().maxlen, 8_192);
        assert_clean(&fs, "ext3-262144");
        let Some(lines) = normalised(&fs, "ext3-262144-dumpe2fs", "dumpe2fs", &["-h"]) else {
            return;
        };
        assert_lines(
            &lines,
            &["Total journal blocks: 8192", "Total journal size: 8M"],
            "dumpe2fs -h (262144)",
        );
    }
}
```

- [ ] **Step 21: Run the oracle.**

Run: `cargo test -p ext --test e2fsprogs ext3_format`
Expected: `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out`, and no `e2fsprogs not found; skipping` line (the tools are installed locally; `tool()` finds them under `/opt/homebrew/opt/e2fsprogs/sbin`).

Run: `CI=1 cargo test -p ext --test e2fsprogs`
Expected: `test result: ok. 15 passed; 0 failed` (with `CI` set a missing tool would fail, so this proves none was skipped).

#### Gates and commit

- [ ] **Step 22: Run the gates.**

Run: `cargo fmt --all -- --check`
Expected: no output, exit code 0.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: `Finished` with no warnings or errors.

Run: `cargo test --workspace`
Expected: every `test result:` line reads `ok` with `0 failed`; `ext` lib `80 passed`, `e2fsprogs` `15 passed`, `ext2` `52 passed`, `ext3` `22 passed`; the fat, fs-core, and wasm results are unchanged (fat lib 82, fat16 16, fs-core 34, wasm lib 20).

Run: `cargo build -p fs-emulator-wasm --target wasm32-unknown-unknown`
Expected: `Finished`.

Optional (the wasm boundary did change shape inside `volume.rs`): `wasm-pack test --node crates/wasm` with Node on `PATH`.
Expected: `test result: ok. 15 passed; 0 failed`.

- [ ] **Step 23: Commit.**

```bash
git add crates/ext/src/fs.rs crates/ext/src/journal/mod.rs crates/ext/src/journal/state.rs \
  crates/ext/src/lib.rs crates/ext/src/superblock.rs crates/ext/tests/common/mod.rs \
  crates/ext/tests/ext3.rs crates/ext/tests/e2fsprogs.rs crates/wasm/src/dto.rs \
  crates/wasm/src/volume.rs
git commit -m "feat(ext): ext3 format and load with the journal on inode 8"
```

Expected: one commit, `10 files changed, 1416 insertions(+), 78 deletions(-)`.

---

### Task 4: Transactions and crash

**Spec:** sections 5 (all of it: 5.1 steps 1 to 7, the emitted sequence of 5.2, the mode table of 5.3), 6 (the `arm_crash` / `disarm_crash` / `crash_phase` paragraph, the crash truncation paragraph, and the "While `needs_recovery`" paragraph; `recover()` is Task 5's), 10.1 (classification on a synthetic draft), 10.2 (the bullets on re-running the slice-2 mutation scenarios against 5.2, ordered versus data, the ring, the size limit, the escaped copy, and the crash-phase bullet up to and including "reads see the raw state"; the `recover()` halves are Task 5's), 10.3 (the second bullet, and from the third bullet the plain `logdump` of a crashed image and the `e2fsck -fy` / `-fn` result; the byte comparison with our `recover()` is Task 5's), 13 (every ruling).

On an ext3 volume every path mutation now runs through the post-pass transaction builder of spec 5.1: `run_mutation` hands the unchanged ext2 body to `journal::txn::run_journaled`, which runs it under a draft operation, classifies the touched blocks with `block_owners()` (a block is data exactly when its owner role is `BlockRole::Data`; every draft change is split at 1024-byte boundaries), takes the after-images (setting `needs_recovery` in the primary superblock's), checks the size limit, restores the disk, and records the honest sequence of 5.2: flag set, ordered data home, journal superblock, descriptors and copies (ascending block order, `ESCAPE` for a copy that starts with the magic, the ring rule block by block), commit, checkpoint, journal emptied, flag cleared. An armed crash stops that emission where section 6 says and the operation still succeeds, with the ` (crashed ...)` name suffix and a final `Crashed` event; afterwards the volume refuses every mutation with `NeedsRecovery`, before any check of its arguments, while every read and `write_raw` works on the raw state. ext2 volumes take the old path unchanged, so every slice-2 record is byte-identical and carries no journal event.

Everything below was compiled and run on a scratch branch holding Tasks 1 to 3 plus this task, and again after the plan critique's fix round: fmt, workspace clippy, `cargo test --workspace` including the e2fsprogs oracle (also with `CI=1`, nothing skipped), and the wasm32 build. The rounds were then replayed from the base branch by a script that applied exactly the code blocks below in order and ran each command; every "Expected" is the observed output, every intermediate state passed `cargo fmt --all -- --check`, and the replayed tree was identical to the commit. Copy the code verbatim.

Verified by hand with e2fsprogs 1.47.4 before the tests were written: after a `create_dir` and a 3-block `create_file`, both modes are `e2fsck -fn` clean; a crashed image (each phase, each mode) makes plain `debugfs -R logdump` print `Journal starts at block 11, transaction 2`, `Found expected sequence 2, type 1 (descriptor block) at block 11`, and (after commit) `Found expected sequence 2, type 2 (commit block) at block 19`; `logdump -a` adds the tag lines `FS block 1 logged at journal block 12 (flags 0x0)`, ..., `FS block 69 logged at journal block 18 (flags 0xa)`; and `e2fsck -fy` exits 0 with `recovering journal`, after which `-fn` is clean. The replayed after-commit and during-checkpoint images differ from the uncrashed image only in block 1 (8 bytes of superblock times and state) and in one byte of the journal superblock (`s_sequence`: e2fsck writes tid + 2, spec 6 step 4, Task 5's rule). Three debugfs facts the tests rely on: tag lines appear only with `-a` (`logdump -O` alone prints the headers); `logdump -O` prints `logdump: short read (read 0, expected 1024) while reading journal` on stderr for every image, an `mke2fs -t ext3` image included, so the tests read stdout only; and `-O` walks from journal block 1 and stops at the first header position without the magic, so the logdump test reloads the image first (`from_image` restarts the head at index 1, spec section 4) and the logged transaction then starts at block 1.

Decisions later tasks must know:
- `NeedsRecovery` is spec 5.1 step 1 as amended: each of the five mutations calls `ensure_recovered()` right after the corruption gate (`ensure_mounted()`) and before any path, name, existence, or space check, so a crashed volume answers `Err(NeedsRecovery)` to every mutation whatever its arguments (`delete_file("/missing")` and `create_dir("/lost+found")` included), with nothing recorded. The corruption gate still wins on a volume that is both corrupt and crashed. `run_journaled` itself makes no such check (every caller has made it); it takes the journal by value from `run_mutation`, so it has no `expect`, and writes it back into `fs.journal` at the end.
- After a crash, `txn::reload_metadata` re-decodes `sb` and `gds` from the disk (`Superblock::decode` of the primary copy, `group::decode_table` of the primary table) without validating them: the crash itself wrote those bytes, and only the `needs_recovery` bit and the half-written transaction differ from a state that already validated. So `superblock().feature_incompat == 0x0006`; `journal.needs_recovery = true`, `journal.armed = None`, `head` and `sequence` keep their pre-transaction values, and `journal_info().start` (read from disk) is the transaction's start, written at step 3. Task 5's `recover` is the other reader and does not use `reload_metadata`: after its replay it calls `reparse_metadata()` (Task 3's gate re-parse), which runs `parse_metadata` and `JournalState::open`, so the replayed superblock and descriptors are validated and a bad replay leaves the volume marked corrupt; `recover` then sets `sequence`, `head = first`, and `needs_recovery = false` itself.
- The commit block's `h_commit_sec` is `now.to_unix_seconds().max(1)` as `u64`; the descriptor uuid is `sb.uuid`.
- The transaction length is `descriptor_blocks_needed(n) + n + 1`; `head` after a completed transaction is `wrap(commit index + 1)`, `sequence` is `tid + 1`. A 300 KiB data-mode `create_file` on the default disk tags 310 blocks: `Unsupported("transaction of 310 blocks exceeds the journal's maximum of 256")`.
- The crash stop points live in `emit` from Round 3 (the `armed` field already exists since Task 3); Round 4 adds the API that sets it and the tests that drive it.
- `crates/ext/tests/e2fsprogs.rs`: the slice-2 scripted sequence is now `scripted_sequence::run_script(fs, big)` plus `scripted_sequence::assert_tools_agree(fs, name)` (both `pub(super)`); the slice-2 test calls them with `300 * 1024`, the ext3 tests with 300 KiB (ordered) and 200 KiB (data: 300 KiB would exceed the limit). The ext3 integration tests build an "ext2 twin" (the ext3 image with `has_journal` cleared in the primary superblock, loaded with `from_image`): it allocates exactly as the ext3 volume does, so its record of the same operation is the draft, and `mask_journal` makes the final images comparable.

**Files:**
- Create: `crates/ext/src/journal/txn.rs` (494 lines): imports and constants (lines 1-24), `Classified`, `classify`, `after_images`, `le32` (lines 26-105), `undo`, `Transaction`, `Outcome`, `Emitter`, `emit`, `run_journaled`, `reload_metadata` (lines 107-377), unit tests (lines 379-494).
- Modify: `crates/ext/src/events.rs`: module doc and `CrashPhase` import (lines 1-5), `JournalWriteKind` (lines 17-28), the ten variants (lines 95-161), their Display arms (lines 240-321), `kind` arms (lines 340-349), `region` arms (lines 366-375); test `journal_events_have_their_kind_region_and_text` (lines 542-739).
- Modify: `crates/ext/src/lib.rs`: `JournalWriteKind` in the `events` re-export (line 17).
- Modify: `crates/ext/src/journal/mod.rs`: module doc and `pub mod txn;` (lines 6-10).
- Modify: `crates/ext/src/fs.rs`: the `CrashPhase` import (lines 16-18), `arm_crash`, `disarm_crash`, `crash_phase` (lines 384-406), `ensure_recovered` (lines 546-555), `use crate::journal::txn;` in `mod create` (line 1254), the `run_mutation` doc and dispatch (lines 1294-1307), the `ensure_recovered()` call in `check_new_entry` (lines 1468-1473), `overwrite_file` (line 1622), `unlink_file` (line 1759), and `unlink_dir` (line 1783).
- Modify: `crates/ext/tests/common/mod.rs`: imports (lines 6-7), `mask_journal` and `changes_in_order` (lines 24-59).
- Test: `crates/ext/tests/ext3.rs`: `mod transactions` appended (lines 611-1495): helpers and the 5.2 checker (lines 611-842), the script and record tests (lines 844-1198), the crash scenarios and tests (lines 1200-1494).
- Test: `crates/ext/tests/e2fsprogs.rs`: `scripted_sequence` split into `run_script` and `assert_tools_agree` (lines 292-405); `mod ext3_transactions` appended (lines 600-760), using `common::ext3` (the file declares `mod common;` since Task 3).

**Interfaces:**

Consumes (from Tasks 1 to 3, on the base branch):
- `fs_core::{ByteChange, Disk, Error, OpRecord, Result, finish_op}` with `Disk::{begin_op, end_op, write, event, read, as_bytes}`; `Error::NeedsRecovery` (Task 1).
- `crate::journal::{CrashPhase, JournalMode, JournalSuperblock (SEQUENCE_OFFSET = 0x18), Tag, TAG_ESCAPE, TAGS_PER_DESCRIPTOR, FEATURE_INCOMPAT_RECOVER, descriptor_blocks_needed, encode_descriptor, encode_commit, needs_escape, escape}` and, in tests, `ext::{decode_descriptor, decode_commit, TAG_SAME_UUID, TAG_LAST}` (Task 2).
- `crate::journal::state::JournalState { blocks, maxlen, first, sequence, head, mode, needs_recovery, armed }` with `physical`, `offset`, `wrap`, `max_transaction` (Task 3); `ExtFs`'s `pub(crate)` fields `disk, sb, gds, geo, history, now, journal`; `ExtFs::{needs_recovery, block_owners, journal_info, journal_mode, ensure_mounted}`; `BlockOwner`, `BlockRole::{Data, Journal}`; the test helpers `common::{ext3, pattern}`.

Produces (exact):

```rust
// crates/ext/src/events.rs  (JournalWriteKind re-exported as ext::JournalWriteKind)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalWriteKind { Descriptor, Copy { home: u32, escaped: bool }, Commit }
ExtEvent::RecoveryFlagSet { range }                 // "recovery_flag_set"; "set needs_recovery in the superblock"
ExtEvent::RecoveryFlagCleared { range }             // "recovery_flag_cleared"; "cleared needs_recovery in the superblock"
ExtEvent::TransactionStarted { tid: u32, journal_block: u32, tagged: u32, range }
    // "transaction_started"; "started transaction {tid} at journal block {journal_block} ({tagged} tagged block{s})"
ExtEvent::JournalBlockWritten { tid: u32, kind: JournalWriteKind, index: u32, block: u32, range }
    // "journal_block_written"; Descriptor: "wrote descriptor for transaction {tid} at journal block {index} (block {block})"
    // Copy: "copied block {home} into journal block {index} (block {block})" + " (escaped)" when escaped
    // Commit: "committed transaction {tid} at journal block {index} (block {block})"
ExtEvent::Checkpointed { tid: u32, home: u32, from_index: u32, range }   // "checkpointed"; "checkpointed block {home} from journal block {from_index}"
ExtEvent::JournalEmptied { next_sequence: u32, range }                   // "journal_emptied"; "journal emptied; next transaction {next_sequence}"
ExtEvent::Crashed { phase: CrashPhase, range }                           // "crashed"; "crashed {phase}"
ExtEvent::RecoveryScanned { start: u32, sequence: u32, committed: u32, tagged: u32, range }
    // "recovery_scanned"; start == 0: "journal is clean; nothing to replay";
    // else "scanned the journal from block {start}, sequence {sequence}: {committed} committed transaction{s}, {tagged} tagged block{s}"
ExtEvent::Replayed { tid: u32, home: u32, from_index: u32, range }       // "replayed"; "replayed block {home} from journal block {from_index} (transaction {tid})"
ExtEvent::TransactionDiscarded { tid: u32, tagged: u32, range }          // "transaction_discarded"; "discarded uncommitted transaction {tid} ({tagged} tagged block{s})"
// {s} is "" for 1 and "s" otherwise (the existing `plural`).

// crates/ext/src/journal/txn.rs  (pub mod txn; items crate-private)
pub(crate) struct Classified { pub data_changes: Vec<ByteChange>, pub data_blocks: BTreeSet<u32>, pub metadata_blocks: BTreeSet<u32> }
impl Classified { pub fn tagged(&self, mode: JournalMode) -> Vec<u32>; }   // ascending; Ordered: metadata; Data: metadata ∪ data
pub(crate) fn classify(changes: &[ByteChange], owners: &BTreeMap<u32, BlockOwner>) -> Classified;
pub(crate) fn after_images(disk: &Disk, tagged: &[u32]) -> Vec<Vec<u8>>;   // 1024 bytes each; block 1 gets incompat |= 0x0004
pub(crate) fn run_journaled(fs: &mut ExtFs, j: JournalState, op: String, body: impl FnOnce(&mut ExtFs) -> Result<()>) -> Result<OpRecord>;
    // spec 5.1 steps 2 to 7; `j` is a clone of `fs.journal`, written back (new head and sequence, or needs_recovery) at the end

// crates/ext/src/fs.rs
impl ExtFs {
    pub fn arm_crash(&mut self, phase: CrashPhase) -> Result<()>;   // Err(Unsupported("this volume has no journal")) on ext2
    pub fn disarm_crash(&mut self);                                 // no-op on ext2
    pub fn crash_phase(&self) -> Option<CrashPhase>;                // None on ext2
}
// run_mutation: `if let Some(journal) = self.journal.clone() { return txn::run_journaled(self, journal, op, body); }`, else the slice-2 path.
// fn ensure_recovered(&self) -> Result<()>   (private): Err(NeedsRecovery) while needs_recovery(); called by the five
//   mutations right after ensure_mounted() (check_new_entry for both creates, overwrite_file, unlink_file, unlink_dir).
// Operation names after a crash: "{op} (crashed before commit)" | "{op} (crashed after commit)" | "{op} (crashed during checkpoint)".
// Size limit: Err(Unsupported("transaction of {n} blocks exceeds the journal's maximum of {max}")), max = maxlen / 4.

// crates/ext/tests/common/mod.rs
pub fn mask_journal(image: &mut [u8], fs: &ExtFs);
pub fn changes_in_order(record: &OpRecord) -> Vec<(usize, usize)>;

// crates/ext/tests/e2fsprogs.rs, mod scripted_sequence
pub(super) fn run_script(fs: &mut ExtFs, big: usize);
pub(super) fn assert_tools_agree(fs: &ExtFs, name: &str);
```

The record of a completed transaction, change by change (spec 5.2; `tid = sequence`, `start = head` before the operation; journal index `i` is physical block `82 + i` on the default disk):
1. 4 bytes at 1024 + 0x60: incompat 0x0002 → 0x0006. Event `RecoveryFlagSet`, preceded by every draft event in draft order.
2. Ordered mode only: each draft change portion that lies in a data block, in draft order, with its original offset and bytes. No event.
3. 8 bytes at journal index 0 + 0x18: be32 `tid`, be32 `start` (before: `tid`, 0). `TransactionStarted`.
4. Per descriptor (at most 122 tags): the descriptor block, then each copy (the after-image, escaped when it starts with `C0 3B 39 98`), indexes advancing by the ring rule. `JournalBlockWritten` each.
5. The commit block. `JournalBlockWritten { kind: Commit }`.
6. Each tagged block's after-image home, whole blocks, ascending. `Checkpointed` each.
7. 8 bytes at 0x18: be32 `tid + 1`, be32 0. `JournalEmptied`.
8. 4 bytes at 1024 + 0x60: 0x0006 → 0x0002. `RecoveryFlagCleared`.

Crash truncation: `BeforeCommit` stops after the last copy, `AfterCommit` after the commit, `DuringCheckpoint` after the first checkpoint write; then one `Crashed { phase, range }` with the last change's range.

Gate commands used throughout (run from the repository root):

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p fs-emulator-wasm --target wasm32-unknown-unknown
```

---

#### Round 1: the journal events

- [ ] **Step 1: Write the failing event test.** In `crates/ext/src/events.rs`, `mod tests` ends with the test `every_event_has_its_kind_region_and_text`. Add this test after it, before the module's closing `}`:

```rust
    #[test]
    fn journal_events_have_their_kind_region_and_text() {
        let copy = |home, escaped| JournalWriteKind::Copy { home, escaped };
        let cases: Vec<(ExtEvent, &str, &str)> = vec![
            (
                ExtEvent::RecoveryFlagSet {
                    range: 1120..1124,
                },
                "recovery_flag_set",
                "set needs_recovery in the superblock",
            ),
            (
                ExtEvent::RecoveryFlagCleared {
                    range: 1120..1124,
                },
                "recovery_flag_cleared",
                "cleared needs_recovery in the superblock",
            ),
            (
                ExtEvent::TransactionStarted {
                    tid: 3,
                    journal_block: 17,
                    tagged: 5,
                    range: 84_016..84_024,
                },
                "transaction_started",
                "started transaction 3 at journal block 17 (5 tagged blocks)",
            ),
            (
                ExtEvent::TransactionStarted {
                    tid: 1,
                    journal_block: 1,
                    tagged: 1,
                    range: 84_016..84_024,
                },
                "transaction_started",
                "started transaction 1 at journal block 1 (1 tagged block)",
            ),
            (
                ExtEvent::JournalBlockWritten {
                    tid: 3,
                    kind: JournalWriteKind::Descriptor,
                    index: 17,
                    block: 98,
                    range: 100_352..101_376,
                },
                "journal_block_written",
                "wrote descriptor for transaction 3 at journal block 17 (block 98)",
            ),
            (
                ExtEvent::JournalBlockWritten {
                    tid: 3,
                    kind: copy(5, false),
                    index: 18,
                    block: 99,
                    range: 101_376..102_400,
                },
                "journal_block_written",
                "copied block 5 into journal block 18 (block 99)",
            ),
            (
                ExtEvent::JournalBlockWritten {
                    tid: 3,
                    kind: copy(1111, true),
                    index: 19,
                    block: 100,
                    range: 102_400..103_424,
                },
                "journal_block_written",
                "copied block 1111 into journal block 19 (block 100) (escaped)",
            ),
            (
                ExtEvent::JournalBlockWritten {
                    tid: 3,
                    kind: JournalWriteKind::Commit,
                    index: 21,
                    block: 102,
                    range: 104_448..105_472,
                },
                "journal_block_written",
                "committed transaction 3 at journal block 21 (block 102)",
            ),
            (
                ExtEvent::Checkpointed {
                    tid: 3,
                    home: 5,
                    from_index: 18,
                    range: 5120..6144,
                },
                "checkpointed",
                "checkpointed block 5 from journal block 18",
            ),
            (
                ExtEvent::JournalEmptied {
                    next_sequence: 4,
                    range: 84_016..84_024,
                },
                "journal_emptied",
                "journal emptied; next transaction 4",
            ),
            (
                ExtEvent::Crashed {
                    phase: CrashPhase::BeforeCommit,
                    range: 101_376..102_400,
                },
                "crashed",
                "crashed before commit",
            ),
            (
                ExtEvent::Crashed {
                    phase: CrashPhase::AfterCommit,
                    range: 104_448..105_472,
                },
                "crashed",
                "crashed after commit",
            ),
            (
                ExtEvent::Crashed {
                    phase: CrashPhase::DuringCheckpoint,
                    range: 1024..2048,
                },
                "crashed",
                "crashed during checkpoint",
            ),
            (
                ExtEvent::RecoveryScanned {
                    start: 0,
                    sequence: 4,
                    committed: 0,
                    tagged: 0,
                    range: 84_016..84_024,
                },
                "recovery_scanned",
                "journal is clean; nothing to replay",
            ),
            (
                ExtEvent::RecoveryScanned {
                    start: 17,
                    sequence: 3,
                    committed: 1,
                    tagged: 5,
                    range: 84_016..84_024,
                },
                "recovery_scanned",
                "scanned the journal from block 17, sequence 3: 1 committed transaction, 5 tagged blocks",
            ),
            (
                ExtEvent::RecoveryScanned {
                    start: 1,
                    sequence: 7,
                    committed: 2,
                    tagged: 1,
                    range: 84_016..84_024,
                },
                "recovery_scanned",
                "scanned the journal from block 1, sequence 7: 2 committed transactions, 1 tagged block",
            ),
            (
                ExtEvent::Replayed {
                    tid: 3,
                    home: 5,
                    from_index: 18,
                    range: 5120..6144,
                },
                "replayed",
                "replayed block 5 from journal block 18 (transaction 3)",
            ),
            (
                ExtEvent::TransactionDiscarded {
                    tid: 3,
                    tagged: 5,
                    range: 100_352..101_376,
                },
                "transaction_discarded",
                "discarded uncommitted transaction 3 (5 tagged blocks)",
            ),
            (
                ExtEvent::TransactionDiscarded {
                    tid: 9,
                    tagged: 1,
                    range: 100_352..101_376,
                },
                "transaction_discarded",
                "discarded uncommitted transaction 9 (1 tagged block)",
            ),
        ];
        for (event, kind, text) in cases {
            assert_eq!(event.kind(), kind);
            assert_eq!(event.to_string(), text);
            let region = event.region().expect("every ext event has a region");
            assert!(region.start < region.end, "{event:?}");
            let boxed: Box<dyn Event> = Box::new(event.clone());
            let copy = boxed.clone();
            assert_eq!(copy.kind(), kind);
            assert_eq!(copy.region(), Some(region));
        }
    }
```

Run: `cargo test -p ext --lib events`
Expected: the build fails with ``error[E0433]: cannot find type `CrashPhase` in this scope``, ``error[E0599]: no variant named `RecoveryFlagSet` found for enum `events::ExtEvent` ``, ``error[E0433]: cannot find type `JournalWriteKind` in this scope`` and the like, ending ``error: could not compile `ext` (lib test) due to 25 previous errors``.

- [ ] **Step 2: Add `JournalWriteKind`, the ten variants, their text, kinds, and regions.** All in `crates/ext/src/events.rs`.

Replace the module doc and the first import:

```rust
//! Semantic events an ext2 volume reports while it works, for the change log.

use fs_core::Event;
```

with:

```rust
//! Semantic events an ext2 or ext3 volume reports while it works, for the
//! change log. The journal events follow the transaction sequence of the
//! ext3 spec (section 5.2) and the recovery steps of section 6.

use crate::journal::CrashPhase;
use fs_core::Event;
```

Insert `JournalWriteKind` above the doc comment of `ExtEvent`; replace:

```rust
/// Every variant carries the absolute byte range
```

with:

```rust
/// What a `JournalBlockWritten` wrote into the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalWriteKind {
    Descriptor,
    /// The after-image of home block `home`; `escaped` when its first four
    /// bytes were the journal magic and are zeroed in the copy.
    Copy {
        home: u32,
        escaped: bool,
    },
    Commit,
}

/// Every variant carries the absolute byte range
```

Add the variants at the end of `ExtEvent`; replace its last variant and the closing brace:

```rust
    CountersUpdated {
        free_blocks: u32,
        free_inodes: u32,
        range: Range<usize>,
    },
}
```

with:

```rust
    CountersUpdated {
        free_blocks: u32,
        free_inodes: u32,
        range: Range<usize>,
    },
    /// `needs_recovery` set in the primary superblock (transaction step 1).
    RecoveryFlagSet {
        range: Range<usize>,
    },
    /// `needs_recovery` cleared in the primary superblock (the last step).
    RecoveryFlagCleared {
        range: Range<usize>,
    },
    /// The journal superblock names the transaction: `s_sequence = tid`,
    /// `s_start = journal_block`; `tagged` home blocks follow.
    TransactionStarted {
        tid: u32,
        journal_block: u32,
        tagged: u32,
        range: Range<usize>,
    },
    /// One block written into the log at journal index `index`, physical
    /// block `block`.
    JournalBlockWritten {
        tid: u32,
        kind: JournalWriteKind,
        index: u32,
        block: u32,
        range: Range<usize>,
    },
    /// A tagged block written home from its copy at `from_index`.
    Checkpointed {
        tid: u32,
        home: u32,
        from_index: u32,
        range: Range<usize>,
    },
    /// The journal superblock back at rest: `s_start = 0`, `s_sequence =
    /// next_sequence`.
    JournalEmptied {
        next_sequence: u32,
        range: Range<usize>,
    },
    /// An armed crash stopped the transaction; `range` is the last change's.
    Crashed {
        phase: CrashPhase,
        range: Range<usize>,
    },
    /// Recovery's scan: from journal index `start` (0: the journal is
    /// clean), expecting `sequence`, found `committed` transactions tagging
    /// `tagged` blocks.
    RecoveryScanned {
        start: u32,
        sequence: u32,
        committed: u32,
        tagged: u32,
        range: Range<usize>,
    },
    /// Recovery wrote home block `home` from its copy at `from_index`.
    Replayed {
        tid: u32,
        home: u32,
        from_index: u32,
        range: Range<usize>,
    },
    /// Recovery skipped a transaction that never reached its commit block;
    /// `range` is its descriptor block.
    TransactionDiscarded {
        tid: u32,
        tagged: u32,
        range: Range<usize>,
    },
}
```

Add the Display arms at the end of the `match` in `fmt`; replace:

```rust
                "free counts now {free_blocks} blocks and {free_inodes} inodes"
            ),
        }
```

with:

```rust
                "free counts now {free_blocks} blocks and {free_inodes} inodes"
            ),
            ExtEvent::RecoveryFlagSet { .. } => {
                write!(f, "set needs_recovery in the superblock")
            }
            ExtEvent::RecoveryFlagCleared { .. } => {
                write!(f, "cleared needs_recovery in the superblock")
            }
            ExtEvent::TransactionStarted {
                tid,
                journal_block,
                tagged,
                ..
            } => write!(
                f,
                "started transaction {tid} at journal block {journal_block} ({tagged} tagged block{})",
                plural(*tagged)
            ),
            ExtEvent::JournalBlockWritten {
                tid,
                kind,
                index,
                block,
                ..
            } => match kind {
                JournalWriteKind::Descriptor => write!(
                    f,
                    "wrote descriptor for transaction {tid} at journal block {index} (block {block})"
                ),
                JournalWriteKind::Copy { home, escaped } => {
                    write!(
                        f,
                        "copied block {home} into journal block {index} (block {block})"
                    )?;
                    if *escaped {
                        write!(f, " (escaped)")?;
                    }
                    Ok(())
                }
                JournalWriteKind::Commit => write!(
                    f,
                    "committed transaction {tid} at journal block {index} (block {block})"
                ),
            },
            ExtEvent::Checkpointed {
                home, from_index, ..
            } => write!(f, "checkpointed block {home} from journal block {from_index}"),
            ExtEvent::JournalEmptied { next_sequence, .. } => {
                write!(f, "journal emptied; next transaction {next_sequence}")
            }
            ExtEvent::Crashed { phase, .. } => write!(f, "crashed {phase}"),
            ExtEvent::RecoveryScanned {
                start,
                sequence,
                committed,
                tagged,
                ..
            } => {
                if *start == 0 {
                    write!(f, "journal is clean; nothing to replay")
                } else {
                    write!(
                        f,
                        "scanned the journal from block {start}, sequence {sequence}: \
                         {committed} committed transaction{}, {tagged} tagged block{}",
                        plural(*committed),
                        plural(*tagged)
                    )
                }
            }
            ExtEvent::Replayed {
                tid,
                home,
                from_index,
                ..
            } => write!(
                f,
                "replayed block {home} from journal block {from_index} (transaction {tid})"
            ),
            ExtEvent::TransactionDiscarded { tid, tagged, .. } => write!(
                f,
                "discarded uncommitted transaction {tid} ({tagged} tagged block{})",
                plural(*tagged)
            ),
        }
```

Add the kinds at the end of the `match` in `kind`; replace:

```rust
            ExtEvent::CountersUpdated { .. } => "counters_updated",
```

with:

```rust
            ExtEvent::CountersUpdated { .. } => "counters_updated",
            ExtEvent::RecoveryFlagSet { .. } => "recovery_flag_set",
            ExtEvent::RecoveryFlagCleared { .. } => "recovery_flag_cleared",
            ExtEvent::TransactionStarted { .. } => "transaction_started",
            ExtEvent::JournalBlockWritten { .. } => "journal_block_written",
            ExtEvent::Checkpointed { .. } => "checkpointed",
            ExtEvent::JournalEmptied { .. } => "journal_emptied",
            ExtEvent::Crashed { .. } => "crashed",
            ExtEvent::RecoveryScanned { .. } => "recovery_scanned",
            ExtEvent::Replayed { .. } => "replayed",
            ExtEvent::TransactionDiscarded { .. } => "transaction_discarded",
```

Add the regions to the or-pattern in `region`; replace:

```rust
            | ExtEvent::CountersUpdated { range, .. } => range,
```

with:

```rust
            | ExtEvent::CountersUpdated { range, .. }
            | ExtEvent::RecoveryFlagSet { range }
            | ExtEvent::RecoveryFlagCleared { range }
            | ExtEvent::TransactionStarted { range, .. }
            | ExtEvent::JournalBlockWritten { range, .. }
            | ExtEvent::Checkpointed { range, .. }
            | ExtEvent::JournalEmptied { range, .. }
            | ExtEvent::Crashed { range, .. }
            | ExtEvent::RecoveryScanned { range, .. }
            | ExtEvent::Replayed { range, .. }
            | ExtEvent::TransactionDiscarded { range, .. } => range,
```

In `crates/ext/src/lib.rs`, replace:

```rust
pub use events::{BitmapKind, ExtEvent};
```

with:

```rust
pub use events::{BitmapKind, ExtEvent, JournalWriteKind};
```

- [ ] **Step 3: Run the event tests.**

Run: `cargo test -p ext --lib events`
Expected: `test events::tests::every_event_has_its_kind_region_and_text ... ok`, `test events::tests::journal_events_have_their_kind_region_and_text ... ok`, `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 79 filtered out`.

#### Round 2: classification and after-images

- [ ] **Step 4: Create the module with its failing unit tests.** In `crates/ext/src/journal/mod.rs`, replace:

```rust
//! mounted volume: opening it, the format-time writer, and `JournalInfo`.

pub mod state;
```

with:

```rust
//! mounted volume: opening it, the format-time writer, and `JournalInfo`;
//! `txn` turns each mutation into one transaction (spec section 5).

pub mod state;
pub mod txn;
```

Create `crates/ext/src/journal/txn.rs` with the module doc, this round's imports, the constants, and the tests:

```rust
//! The post-pass transaction builder (spec section 5): run the unchanged
//! ext2 body under a draft operation, classify the blocks it touched, take
//! their after-images, put the disk back, and re-emit the honest write
//! sequence of section 5.2 as the recorded operation, stopping at an armed
//! crash phase (section 6).

use super::{JournalMode, FEATURE_INCOMPAT_RECOVER};
use crate::fs::{BlockOwner, BlockRole};
use crate::superblock::{BLOCK_SIZE, SUPERBLOCK_OFFSET};
use fs_core::{ByteChange, Disk};
use std::collections::{BTreeMap, BTreeSet};

const BS: usize = BLOCK_SIZE as usize;
/// `s_feature_incompat` in the ext superblock.
const INCOMPAT_OFFSET: usize = 0x60;
/// The block that holds the primary superblock (1 KiB blocks).
const PRIMARY_SUPERBLOCK_BLOCK: u32 = (SUPERBLOCK_OFFSET / BS) as u32;

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(role: BlockRole) -> BlockOwner {
        BlockOwner {
            inode: 12,
            path: "/f".into(),
            role,
        }
    }

    fn change(offset: usize, len: usize, before: u8, after: u8) -> ByteChange {
        ByteChange {
            offset,
            before: vec![before; len],
            after: vec![after; len],
        }
    }

    #[test]
    fn a_change_across_a_directory_and_a_data_block_splits_in_two() {
        let owners = BTreeMap::from([
            (100, owner(BlockRole::Directory)),
            (101, owner(BlockRole::Data)),
        ]);
        let spanning = ByteChange {
            offset: 101 * BS - 4,
            before: vec![0, 0, 0, 0, 0, 0, 0, 0],
            after: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let c = classify(&[spanning], &owners);
        assert_eq!(
            c.data_changes,
            vec![ByteChange {
                offset: 101 * BS,
                before: vec![0; 4],
                after: vec![5, 6, 7, 8],
            }]
        );
        assert_eq!(c.data_blocks, BTreeSet::from([101]));
        assert_eq!(c.metadata_blocks, BTreeSet::from([100]));
        assert_eq!(c.tagged(JournalMode::Ordered), vec![100]);
        assert_eq!(c.tagged(JournalMode::Data), vec![100, 101]);
    }

    #[test]
    fn unowned_indirect_and_directory_blocks_are_metadata_in_block_order() {
        let owners = BTreeMap::from([
            (69, owner(BlockRole::Directory)),
            (1114, owner(BlockRole::Indirect)),
            (1111, owner(BlockRole::Data)),
            (1112, owner(BlockRole::Data)),
            (1113, owner(BlockRole::Data)),
        ]);
        let changes = [
            // Data 1111..1114 in one run, then inode 12 (block 6 of the
            // inode table), a bitmap, the superblock counters, the pointer
            // block, the directory.
            change(1111 * BS, 3 * BS, 0, 7),
            change(5 * BS + 11 * 128, 128, 0, 1),
            change(3 * BS + 138, 1, 0, 0xFF),
            change(SUPERBLOCK_OFFSET + 12, 8, 0, 2),
            change(1114 * BS, BS, 0, 3),
            change(69 * BS + 24, 20, 0, 4),
        ];
        let c = classify(&changes, &owners);
        assert_eq!(
            c.data_changes
                .iter()
                .map(|d| (d.offset, d.after.len()))
                .collect::<Vec<_>>(),
            vec![(1111 * BS, BS), (1112 * BS, BS), (1113 * BS, BS)]
        );
        assert_eq!(c.tagged(JournalMode::Ordered), vec![1, 3, 6, 69, 1114]);
        assert_eq!(
            c.tagged(JournalMode::Data),
            vec![1, 3, 6, 69, 1111, 1112, 1113, 1114]
        );
    }

    #[test]
    fn a_data_block_written_twice_keeps_both_portions_in_draft_order() {
        let owners = BTreeMap::from([(200, owner(BlockRole::Data))]);
        let changes = [
            change(200 * BS, 10, 0, 1),
            change(200 * BS + 10, BS - 10, 0, 0),
        ];
        let c = classify(&changes, &owners);
        assert_eq!(c.data_changes, changes.to_vec());
        assert_eq!(c.data_blocks, BTreeSet::from([200]));
        assert!(c.metadata_blocks.is_empty());
        assert!(c.tagged(JournalMode::Ordered).is_empty());
    }

    #[test]
    fn after_images_set_needs_recovery_only_in_the_primary_superblock() {
        let mut disk = Disk::new(BS, 8);
        disk.write(
            SUPERBLOCK_OFFSET + INCOMPAT_OFFSET,
            &0x0002u32.to_le_bytes(),
        );
        disk.write(5 * BS, &[9; BS]);
        disk.write(6 * BS + INCOMPAT_OFFSET, &0x0002u32.to_le_bytes());
        let images = after_images(&disk, &[1, 5, 6]);
        assert_eq!(images.len(), 3);
        assert!(images.iter().all(|image| image.len() == BS));
        let mut want = disk.read(SUPERBLOCK_OFFSET, BS).to_vec();
        want[INCOMPAT_OFFSET] = 0x06;
        assert_eq!(images[0], want);
        assert_eq!(images[1], vec![9; BS]);
        assert_eq!(images[2], disk.read(6 * BS, BS));
        // The disk itself is untouched.
        assert_eq!(disk.read(SUPERBLOCK_OFFSET + INCOMPAT_OFFSET, 1), &[0x02]);
    }
}
```

Run: `cargo test -p ext --lib txn`
Expected: the build fails with four ``error[E0425]: cannot find function `classify` in this scope`` / ``cannot find function `after_images` in this scope`` errors (and ``warning: unused import: `FEATURE_INCOMPAT_RECOVER` ``), ending ``error: could not compile `ext` (lib test) due to 4 previous errors; 1 warning emitted``.

- [ ] **Step 5: Implement the classification and the after-images.** In `crates/ext/src/journal/txn.rs`, insert directly above `#[cfg(test)]`:

```rust
/// What a draft touched, split at block boundaries (spec 5.1 step 3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Classified {
    /// The portions of draft changes that lie in data blocks, in draft
    /// order: what ordered mode writes home at step 2.
    pub data_changes: Vec<ByteChange>,
    /// The data blocks touched.
    pub data_blocks: BTreeSet<u32>,
    /// Every other block touched.
    pub metadata_blocks: BTreeSet<u32>,
}

impl Classified {
    /// The tagged set in ascending block order: the metadata blocks in
    /// ordered mode, metadata and data blocks together in data mode.
    pub fn tagged(&self, mode: JournalMode) -> Vec<u32> {
        match mode {
            JournalMode::Ordered => self.metadata_blocks.iter().copied().collect(),
            JournalMode::Data => self
                .metadata_blocks
                .union(&self.data_blocks)
                .copied()
                .collect(),
        }
    }
}

/// Split every change at 1024-byte block boundaries and sort the pieces: a
/// block is data when `owners` lists it with `BlockRole::Data`, metadata
/// otherwise (superblock and descriptor copies, bitmaps, inode tables,
/// directory and indirect blocks, anything else).
pub(crate) fn classify(changes: &[ByteChange], owners: &BTreeMap<u32, BlockOwner>) -> Classified {
    let mut out = Classified::default();
    for change in changes {
        let end = change.offset + change.after.len();
        let mut at = change.offset;
        while at < end {
            let block = (at / BS) as u32;
            let stop = end.min((at / BS + 1) * BS);
            let is_data = owners
                .get(&block)
                .is_some_and(|owner| owner.role == BlockRole::Data);
            if is_data {
                let (from, to) = (at - change.offset, stop - change.offset);
                out.data_changes.push(ByteChange {
                    offset: at,
                    before: change.before[from..to].to_vec(),
                    after: change.after[from..to].to_vec(),
                });
                out.data_blocks.insert(block);
            } else {
                out.metadata_blocks.insert(block);
            }
            at = stop;
        }
    }
    out
}

/// The 1024-byte after-image of every tagged block as `disk` holds it,
/// with `needs_recovery` set in the primary superblock's image (spec 5.1
/// step 4), so the checkpoint and a replay write the same bytes.
pub(crate) fn after_images(disk: &Disk, tagged: &[u32]) -> Vec<Vec<u8>> {
    tagged
        .iter()
        .map(|&block| {
            let mut image = disk.read(block as usize * BS, BS).to_vec();
            if block == PRIMARY_SUPERBLOCK_BLOCK {
                let at = SUPERBLOCK_OFFSET % BS + INCOMPAT_OFFSET;
                let flags = le32(&image[at..at + 4]) | FEATURE_INCOMPAT_RECOVER;
                image[at..at + 4].copy_from_slice(&flags.to_le_bytes());
            }
            image
        })
        .collect()
}

fn le32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
```

- [ ] **Step 6: Run the unit tests.**

Run: `cargo test -p ext --lib txn`
Expected: `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 81 filtered out` (the four tests `a_change_across_a_directory_and_a_data_block_splits_in_two`, `unowned_indirect_and_directory_blocks_are_metadata_in_block_order`, `a_data_block_written_twice_keeps_both_portions_in_draft_order`, `after_images_set_needs_recovery_only_in_the_primary_superblock`). Nothing outside the tests calls these functions yet, so a non-test build of `ext` warns that they are never used; Round 3 uses them, and clippy runs only at the end.

#### Round 3: the emitted sequence and the record tests

- [ ] **Step 7: Add the two test helpers.** In `crates/ext/tests/common/mod.rs`, replace:

```rust
use ext::{ExtFormatOptions, ExtFs, JournalMode, JournalOptions};
```

with:

```rust
use ext::{BlockRole, ExtFormatOptions, ExtFs, JournalMode, JournalOptions};
use fs_core::OpRecord;
```

and append to the end of the file:

```rust
/// Blank what only an ext3 volume carries, so its image compares equal to
/// an ext2 image of the same operations: every data block of `fs`'s
/// journal, and in every superblock copy `s_journal_uuid`,
/// `s_journal_inum`, `s_journal_dev` (0xD0..0xE8), `s_jnl_backup_type`
/// (0xFD), `s_default_mount_opts` (0x100), `s_jnl_blocks` (0x10C..0x150),
/// the `has_journal` bit of 0x5C, and the `needs_recovery` bit of 0x60.
pub fn mask_journal(image: &mut [u8], fs: &ExtFs) {
    for (&block, owner) in &fs.block_owners() {
        if owner.role == BlockRole::Journal {
            let at = block as usize * 1024;
            image[at..at + 1024].fill(0);
        }
    }
    for gl in &fs.geometry().groups_layout {
        let Some(block) = gl.superblock_block else {
            continue;
        };
        // Block 1 holds the primary copy at byte 1024; a backup starts its block.
        let sb = block as usize * 1024;
        image[sb + 0xD0..sb + 0xE8].fill(0);
        image[sb + 0xFD] = 0;
        image[sb + 0x100..sb + 0x104].fill(0);
        image[sb + 0x10C..sb + 0x150].fill(0);
        image[sb + 0x5C] &= !0x04;
        image[sb + 0x60] &= !0x04;
    }
}

/// `(offset, len)` of every byte change of `record`, in record order.
pub fn changes_in_order(record: &OpRecord) -> Vec<(usize, usize)> {
    record
        .changes
        .iter()
        .map(|c| (c.offset, c.after.len()))
        .collect()
}
```

- [ ] **Step 8: Write the failing record tests.** Append to the end of `crates/ext/tests/ext3.rs`. The script checks every slice-2 mutation scenario against 5.2 in both modes, including a directory that grows a second block (`/wide`); ordered mode also runs `ORDERED_SCRIPT`, a file grown into the double-indirect range (300 KiB, whose data blocks ordered mode never tags). `a_clock_at_the_unix_epoch_commits_at_second_1` pins the floor of `h_commit_sec` (spec section 2):

```rust
/// Task 4: every mutation as one journal transaction (spec section 5) and
/// the armed crash (spec section 6).
mod transactions {
    use super::common::{changes_in_order, ext3, mask_journal, pattern};
    use ext::{
        decode_commit, decode_descriptor, BlockRole, ExtFormatOptions, ExtFs, JournalInfo,
        JournalMode, TAG_ESCAPE, TAG_LAST, TAG_SAME_UUID,
    };
    use fs_core::{DateTime, Error, OpRecord, Result};
    use std::collections::BTreeSet;

    const BS: usize = 1024;
    const BOTH: [JournalMode; 2] = [JournalMode::Ordered, JournalMode::Data];
    /// `s_feature_incompat` of the primary superblock.
    const FLAG: usize = 1024 + 0x60;
    /// `s_sequence` and `s_start` of the journal superblock (block 82).
    const JSB_SEQ: usize = 82 * 1024 + 0x18;
    /// Journal index `i` is physical block `82 + i` on the default disk.
    const JOURNAL_BLOCK_0: u32 = 82;
    const MAGIC: [u8; 4] = [0xC0, 0x3B, 0x39, 0x98];
    /// The event kinds a transaction adds to the body's own.
    const JOURNAL_KINDS: [&str; 7] = [
        "recovery_flag_set",
        "transaction_started",
        "journal_block_written",
        "checkpointed",
        "journal_emptied",
        "recovery_flag_cleared",
        "crashed",
    ];

    fn be(a: u32, b: u32) -> Vec<u8> {
        [a.to_be_bytes(), b.to_be_bytes()].concat()
    }

    /// `(block * 1024, 1024)` for each block: whole-block writes.
    fn whole(blocks: impl IntoIterator<Item = u32>) -> Vec<(usize, usize)> {
        blocks.into_iter().map(|b| (b as usize * BS, BS)).collect()
    }

    fn texts(record: &OpRecord) -> Vec<String> {
        record.events.iter().map(|e| e.to_string()).collect()
    }

    fn count(record: &OpRecord, kind: &str) -> usize {
        record.event_kinds().iter().filter(|&&k| k == kind).count()
    }

    /// The journal ring of the default 1024-block journal.
    fn wrap(index: u32) -> u32 {
        if index >= 1024 {
            index - 1023
        } else {
            index
        }
    }

    /// The ext2 volume with `fs`'s exact layout: the same image with
    /// `has_journal` cleared, so it allocates the same blocks and inodes
    /// and its records are what the ext3 body writes before the post-pass.
    fn ext2_twin(fs: &ExtFs) -> ExtFs {
        let mut image = fs.disk().as_bytes().to_vec();
        image[1024 + 0x5C] &= !0x04;
        let mut twin = ExtFs::from_image(image).unwrap();
        assert_eq!(twin.fs_type(), "ext2");
        twin.set_now(fs.now());
        twin
    }

    /// Check an ext3 record against spec 5.2 step by step: `draft` is the
    /// ext2 twin's record of the same operation (the body's own writes and
    /// events), `before` the journal before it. Returns the tagged blocks.
    fn assert_follows_5_2(
        fs: &ExtFs,
        rec: &OpRecord,
        draft: &OpRecord,
        before: &JournalInfo,
    ) -> Vec<u32> {
        let mode = fs.journal_mode().unwrap();
        let owners = fs.block_owners();
        let is_data = |b: u32| owners.get(&b).is_some_and(|o| o.role == BlockRole::Data);
        // The oracle's own classification of the draft (spec 5.1 step 3).
        let mut tagged = BTreeSet::new();
        let mut data: Vec<(usize, Vec<u8>)> = Vec::new();
        for c in &draft.changes {
            let end = c.offset + c.after.len();
            let mut at = c.offset;
            while at < end {
                let block = (at / BS) as u32;
                let stop = end.min((at / BS + 1) * BS);
                if is_data(block) {
                    data.push((at, c.after[at - c.offset..stop - c.offset].to_vec()));
                    if mode == JournalMode::Data {
                        tagged.insert(block);
                    }
                } else {
                    tagged.insert(block);
                }
                at = stop;
            }
        }
        let tagged: Vec<u32> = tagged.into_iter().collect();
        let n = tagged.len();
        let (tid, start) = (before.sequence, before.head);
        let block_of = |index: u32| JOURNAL_BLOCK_0 + index;
        let ordered = mode == JournalMode::Ordered;

        let mut want_changes = vec![(FLAG, 4)];
        if ordered {
            want_changes.extend(data.iter().map(|(at, bytes)| (*at, bytes.len())));
        }
        want_changes.push((JSB_SEQ, 8));
        let mut want_events = texts(draft);
        want_events.push("set needs_recovery in the superblock".into());
        want_events.push(format!(
            "started transaction {tid} at journal block {start} ({n} tagged block{})",
            if n == 1 { "" } else { "s" }
        ));
        let mut index = start;
        let mut copies = Vec::new();
        for chunk in tagged.chunks(122) {
            want_changes.extend(whole([block_of(index)]));
            want_events.push(format!(
                "wrote descriptor for transaction {tid} at journal block {index} (block {})",
                block_of(index)
            ));
            for &home in chunk {
                index = wrap(index + 1);
                let escaped = fs.disk().read(home as usize * BS, 4) == MAGIC;
                want_changes.extend(whole([block_of(index)]));
                want_events.push(format!(
                    "copied block {home} into journal block {index} (block {}){}",
                    block_of(index),
                    if escaped { " (escaped)" } else { "" }
                ));
                copies.push(index);
            }
            index = wrap(index + 1);
        }
        let commit = index;
        want_changes.extend(whole([block_of(commit)]));
        want_events.push(format!(
            "committed transaction {tid} at journal block {commit} (block {})",
            block_of(commit)
        ));
        for (&home, &from) in tagged.iter().zip(&copies) {
            want_changes.extend(whole([home]));
            want_events.push(format!(
                "checkpointed block {home} from journal block {from}"
            ));
        }
        want_changes.push((JSB_SEQ, 8));
        want_events.push(format!("journal emptied; next transaction {}", tid + 1));
        want_changes.push((FLAG, 4));
        want_events.push("cleared needs_recovery in the superblock".into());
        assert_eq!(changes_in_order(rec), want_changes, "{}", rec.op);
        assert_eq!(texts(rec), want_events, "{}", rec.op);

        // The bytes of every step.
        let c = &rec.changes;
        assert_eq!(
            (&c[0].before[..], &c[0].after[..]),
            (&[2, 0, 0, 0][..], &[6, 0, 0, 0][..])
        );
        let last = c.last().unwrap();
        assert_eq!(
            (&last.before[..], &last.after[..]),
            (&[6, 0, 0, 0][..], &[2, 0, 0, 0][..])
        );
        let mut pos = 1;
        if ordered {
            for (at, bytes) in &data {
                assert_eq!((c[pos].offset, &c[pos].after), (*at, bytes));
                pos += 1;
            }
        }
        assert_eq!(
            (c[pos].before.clone(), c[pos].after.clone()),
            (be(tid, 0), be(tid, start))
        );
        pos += 1;
        // The after-image of `home` as the checkpoint writes it: the final
        // bytes, with needs_recovery still set in the superblock's.
        let after_image = |home: u32| {
            let mut image = fs.disk().read(home as usize * BS, BS).to_vec();
            if home == 1 {
                image[0x60] |= 0x04;
            }
            image
        };
        for chunk in tagged.chunks(122) {
            let tags = decode_descriptor(&c[pos].after).unwrap();
            assert_eq!(tags.iter().map(|t| t.block).collect::<Vec<_>>(), chunk);
            for (i, tag) in tags.iter().enumerate() {
                let same = if i > 0 { TAG_SAME_UUID } else { 0 };
                let last = if i + 1 == tags.len() { TAG_LAST } else { 0 };
                assert_eq!(tag.flags & !TAG_ESCAPE, same | last, "tag {i}");
            }
            pos += 1;
            for tag in &tags {
                let mut copy = after_image(tag.block);
                if tag.flags & TAG_ESCAPE != 0 {
                    copy[..4].fill(0);
                }
                assert!(c[pos].after == copy, "copy of block {}", tag.block);
                pos += 1;
            }
        }
        assert_eq!(
            decode_commit(&c[pos].after),
            Some(fs.now().to_unix_seconds().max(1) as u64)
        );
        pos += 1;
        for &home in &tagged {
            assert!(c[pos].after == after_image(home), "checkpoint of {home}");
            pos += 1;
        }
        assert_eq!(
            (c[pos].before.clone(), c[pos].after.clone()),
            (be(tid, start), be(tid + 1, 0))
        );

        let info = fs.journal_info().unwrap();
        assert_eq!(
            (info.sequence, info.head, info.start, info.needs_recovery),
            (tid + 1, wrap(commit + 1), 0, false),
            "{}",
            rec.op
        );
        tagged
    }

    /// One mutation of the script below.
    #[derive(Debug, Clone, Copy)]
    enum Op {
        Create(&'static str, usize),
        /// An empty file in the directory whose name is the character
        /// repeated 255 times: an entry of 264 bytes.
        CreateLong(&'static str, char),
        Write(&'static str, usize),
        Delete(&'static str),
        Mkdir(&'static str),
        Rmdir(&'static str),
    }

    fn apply(fs: &mut ExtFs, op: Op) -> Result<OpRecord> {
        match op {
            Op::Create(path, len) => fs.create_file(path, &pattern(len)),
            Op::CreateLong(dir, c) => {
                fs.create_file(&format!("{dir}/{}", c.to_string().repeat(255)), b"")
            }
            Op::Write(path, len) => fs.write_file(path, &pattern(len + 5)[5..]),
            Op::Delete(path) => fs.delete_file(path),
            Op::Mkdir(path) => fs.create_dir(path),
            Op::Rmdir(path) => fs.remove_dir(path),
        }
    }

    /// The slice-2 mutation scenarios in one script: small, three-block,
    /// single-indirect, and 200 KiB files, growth, shrinks out of the
    /// indirect range, a same-length overwrite, deletes, a directory made
    /// and removed, and a directory that grows a second block (the fourth
    /// 264-byte entry does not fit beside `.`, `..`, and three others).
    /// 200 KiB is the largest round size a data-mode transaction takes
    /// (256 tagged blocks at most).
    const SCRIPT: [Op; 17] = [
        Op::Mkdir("/dir"),
        Op::Create("/dir/a.txt", 100),
        Op::Create("/three.bin", 3000),
        Op::Create("/indirect.bin", 20 * 1024),
        Op::Write("/three.bin", 5000),
        Op::Write("/indirect.bin", 100),
        Op::Write("/dir/a.txt", 100),
        Op::Delete("/dir/a.txt"),
        Op::Rmdir("/dir"),
        Op::Delete("/three.bin"),
        Op::Create("/big.bin", 200 * 1024),
        Op::Write("/big.bin", 13 * 1024),
        Op::Mkdir("/wide"),
        Op::CreateLong("/wide", 'a'),
        Op::CreateLong("/wide", 'b'),
        Op::CreateLong("/wide", 'c'),
        Op::CreateLong("/wide", 'd'),
    ];

    /// Ordered mode only: a file grown into the double-indirect range. Its
    /// 300 data blocks are never tagged in ordered mode; data mode would tag
    /// them and pass the 256-block limit.
    const ORDERED_SCRIPT: [Op; 2] = [
        Op::Create("/grow.bin", 1000),
        Op::Write("/grow.bin", 300 * 1024),
    ];

    #[test]
    fn every_mutation_follows_5_2_and_ends_equal_to_ext2_in_both_modes() {
        for mode in BOTH {
            let mut fs = ext3(mode);
            fs.set_now(DateTime::new(2024, 5, 6, 7, 8, 9));
            let mut twin = ext2_twin(&fs);
            let mut script = SCRIPT.to_vec();
            if mode == JournalMode::Ordered {
                script.extend(ORDERED_SCRIPT);
            }
            for (k, &op) in script.iter().enumerate() {
                let before = fs.journal_info().unwrap();
                assert_eq!(before.sequence, 1 + k as u32);
                let rec = apply(&mut fs, op).unwrap();
                let draft = apply(&mut twin, op).unwrap();
                assert_eq!(rec.op, draft.op);
                assert_follows_5_2(&fs, &rec, &draft, &before);
                let mut ours = fs.disk().as_bytes().to_vec();
                let mut theirs = twin.disk().as_bytes().to_vec();
                mask_journal(&mut ours, &fs);
                mask_journal(&mut theirs, &fs);
                assert!(ours == theirs, "{mode:?} {op:?}: differs from ext2");
            }
            assert_eq!(fs.history().len(), script.len());
            assert_eq!(fs.list_dir("/").unwrap(), twin.list_dir("/").unwrap());
            assert_eq!(
                fs.read_file("/big.bin").unwrap(),
                pattern(13 * 1024 + 5)[5..]
            );
            assert_eq!(fs.list_dir("/wide").unwrap().len(), 4);
            assert_eq!(fs.stat("/wide").unwrap().size, 2 * BS as u64);
            if mode == JournalMode::Ordered {
                assert_eq!(
                    fs.read_file("/grow.bin").unwrap(),
                    pattern(300 * 1024 + 5)[5..]
                );
            }
        }
    }

    #[test]
    fn a_clock_at_the_unix_epoch_commits_at_second_1() {
        for mode in BOTH {
            let mut fs = ext3(mode);
            fs.set_now(DateTime::from_unix_seconds(0));
            let rec = fs.create_dir("/d").unwrap();
            // Eight tagged blocks: the descriptor at index 1, the copies at
            // 2..=9, the commit at index 10 (block 92).
            assert!(texts(&rec)
                .iter()
                .any(|t| t == "committed transaction 1 at journal block 10 (block 92)"));
            let commit = fs.disk().read(92 * BS, BS);
            assert_eq!(decode_commit(commit), Some(1));
            assert_eq!(commit[48..56], 1u64.to_be_bytes());
        }
    }

    #[test]
    fn a_three_block_create_in_ordered_mode_writes_the_spec_sequence() {
        let mut fs = ext3(JournalMode::Ordered);
        let rec = fs.create_file("/f", &pattern(3000)).unwrap();
        // Data home first (the body's write and its zero tail), then the
        // journal superblock, descriptor at index 1 (block 83), seven copies,
        // the commit at index 9 (block 91), seven checkpoints.
        let mut want = vec![(FLAG, 4)];
        want.extend([
            (1111 * BS, BS),
            (1112 * BS, BS),
            (1113 * BS, 952),
            (1113 * BS + 952, 72),
        ]);
        want.push((JSB_SEQ, 8));
        want.extend(whole(83..=91));
        want.extend(whole([1, 2, 3, 4, 5, 6, 69]));
        want.push((JSB_SEQ, 8));
        want.push((FLAG, 4));
        assert_eq!(changes_in_order(&rec), want);
        let mut kinds = vec![
            "inode_allocated",
            "bitmap_updated",
            "counters_updated",
            "blocks_allocated",
            "bitmap_updated",
            "counters_updated",
            "data_written",
            "inode_written",
            "dir_entry_written",
            "inode_written",
            "recovery_flag_set",
            "transaction_started",
        ];
        kinds.extend(["journal_block_written"; 9]);
        kinds.extend(["checkpointed"; 7]);
        kinds.extend(["journal_emptied", "recovery_flag_cleared"]);
        assert_eq!(rec.event_kinds(), kinds);
        let info = fs.journal_info().unwrap();
        assert_eq!((info.sequence, info.head, info.start), (2, 10, 0));
        assert_eq!(fs.read_file("/f").unwrap(), pattern(3000));
    }

    #[test]
    fn a_three_block_create_in_data_mode_journals_the_data_too() {
        let mut fs = ext3(JournalMode::Data);
        let rec = fs.create_file("/f", &pattern(3000)).unwrap();
        // No data before the journal superblock; ten copies at indexes
        // 2..=11 (blocks 84..=93), the commit at index 12 (block 94).
        let mut want = vec![(FLAG, 4), (JSB_SEQ, 8)];
        want.extend(whole(83..=94));
        want.extend(whole([1, 2, 3, 4, 5, 6, 69, 1111, 1112, 1113]));
        want.push((JSB_SEQ, 8));
        want.push((FLAG, 4));
        assert_eq!(changes_in_order(&rec), want);
        assert_eq!(count(&rec, "journal_block_written"), 12);
        assert_eq!(count(&rec, "checkpointed"), 10);
        let info = fs.journal_info().unwrap();
        assert_eq!((info.sequence, info.head, info.start), (2, 13, 0));
        assert_eq!(fs.read_file("/f").unwrap(), pattern(3000));
    }

    #[test]
    fn head_and_sequence_advance_across_operations() {
        let mut fs = ext3(JournalMode::Ordered);
        let mut heads = vec![fs.journal_info().unwrap().head];
        for i in 0..4 {
            let rec = fs.create_dir(&format!("/d{i}")).unwrap();
            let info = fs.journal_info().unwrap();
            let length = count(&rec, "journal_block_written") as u32;
            assert_eq!(info.head, heads.last().unwrap() + length);
            assert_eq!(info.sequence, 2 + i);
            assert_eq!(info.start, 0);
            heads.push(info.head);
        }
        // Each directory tags 8 blocks (superblock, descriptors, both bitmaps,
        // two inode table blocks, the root's block, its own): 10 log blocks.
        assert_eq!(heads, vec![1, 11, 21, 31, 41]);
    }

    #[test]
    fn ordered_mode_writes_data_home_before_the_journal_superblock_and_never_tags_it() {
        let mut fs = ext3(JournalMode::Ordered);
        let rec = fs.create_file("/f", &pattern(20 * 1024)).unwrap();
        let data: BTreeSet<u32> = (1111..1131).collect();
        let step3 = rec
            .changes
            .iter()
            .position(|c| c.offset == JSB_SEQ)
            .unwrap();
        assert!(step3 > 1);
        for c in &rec.changes[1..step3] {
            assert!(data.contains(&((c.offset / BS) as u32)), "{c:?}");
        }
        for c in &rec.changes[step3..] {
            assert!(!data.contains(&((c.offset / BS) as u32)), "{c:?}");
        }
        assert!(texts(&rec)
            .iter()
            .all(|t| !(1111..1131).any(|b| t.starts_with(&format!("copied block {b} ")))));
        // The pointer block after the data (1131) is metadata: tagged.
        assert!(texts(&rec)
            .iter()
            .any(|t| t.starts_with("copied block 1131 ")));
    }

    #[test]
    fn data_mode_tags_data_and_writes_it_home_only_at_the_checkpoint() {
        let mut fs = ext3(JournalMode::Data);
        let rec = fs.create_file("/f", &pattern(20 * 1024)).unwrap();
        let commit = rec
            .changes
            .iter()
            .position(|c| decode_commit(&c.after).is_some())
            .unwrap();
        for c in &rec.changes[..=commit] {
            let block = (c.offset / BS) as u32;
            assert!(
                !(1111..1131).contains(&block),
                "data home before the commit: {c:?}"
            );
        }
        for b in 1111..1131 {
            assert!(texts(&rec)
                .iter()
                .any(|t| t.starts_with(&format!("copied block {b} "))));
            assert!(texts(&rec)
                .iter()
                .any(|t| t.starts_with(&format!("checkpointed block {b} "))));
        }
    }

    #[test]
    fn transactions_wrap_around_the_end_of_the_log_block_by_block() {
        let mut fs = ext3(JournalMode::Ordered);
        let mut twin = ext2_twin(&fs);
        let mut wrapped = None;
        for i in 0..300 {
            let before = fs.journal_info().unwrap();
            let path = format!("/r{i:03}");
            let rec = fs.create_file(&path, b"").unwrap();
            let draft = twin.create_file(&path, b"").unwrap();
            assert_follows_5_2(&fs, &rec, &draft, &before);
            if fs.journal_info().unwrap().head < before.head {
                wrapped = Some((before.head, rec));
                break;
            }
        }
        let (start, rec) = wrapped.expect("300 creates pass the end of a 1024-block log");
        // The journal writes run up to index 1023 (block 1105) and continue
        // at index 1 (block 83).
        let blocks: Vec<u32> = rec
            .changes
            .iter()
            .map(|c| (c.offset / BS) as u32)
            .filter(|b| (83..=1105).contains(b))
            .collect();
        let wrap_at = blocks.iter().position(|&b| b == 83).unwrap();
        assert!(wrap_at > 0);
        assert_eq!(blocks[wrap_at - 1], 1105);
        assert_eq!(
            blocks,
            (start..1024)
                .chain(1..)
                .take(blocks.len())
                .map(|i| JOURNAL_BLOCK_0 + i)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_data_mode_create_past_the_limit_fails_and_changes_nothing() {
        let mut fs = ext3(JournalMode::Data);
        fs.create_file("/keep", b"keep").unwrap();
        let image = fs.disk().as_bytes().to_vec();
        let sb = fs.superblock().clone();
        let gds = fs.group_descriptors().to_vec();
        let info = fs.journal_info();
        assert_eq!(
            fs.create_file("/big.bin", &pattern(300 * 1024))
                .unwrap_err(),
            Error::Unsupported(
                "transaction of 310 blocks exceeds the journal's maximum of 256".into()
            )
        );
        assert!(fs.disk().as_bytes() == image.as_slice());
        assert_eq!(fs.history().len(), 1);
        assert_eq!(fs.superblock(), &sb);
        assert_eq!(fs.group_descriptors(), &gds[..]);
        assert_eq!(fs.journal_info(), info);
        assert!(!fs.disk().op_open());
        // Ordered mode tags only the metadata, so the same file fits.
        let mut ordered = ext3(JournalMode::Ordered);
        ordered
            .create_file("/big.bin", &pattern(300 * 1024))
            .unwrap();
    }

    #[test]
    fn a_data_block_that_starts_with_the_magic_is_escaped_in_its_copy() {
        let mut fs = ext3(JournalMode::Data);
        let mut data = pattern(1024);
        data[..4].copy_from_slice(&MAGIC);
        let rec = fs.create_file("/magic", &data).unwrap();
        let text = "copied block 1111 into journal block 9 (block 91) (escaped)";
        assert!(texts(&rec).iter().any(|t| t == text), "{:?}", texts(&rec));
        let descriptor = rec.changes.iter().find(|c| c.offset == 83 * BS).unwrap();
        let tags = decode_descriptor(&descriptor.after).unwrap();
        let tag = tags.iter().find(|t| t.block == 1111).unwrap();
        assert_eq!(tag.flags, TAG_ESCAPE | TAG_SAME_UUID | TAG_LAST);
        let copy = rec.changes.iter().find(|c| c.offset == 91 * BS).unwrap();
        assert_eq!(copy.after[..4], [0, 0, 0, 0]);
        assert_eq!(copy.after[4..], data[4..]);
        let home = rec.changes.iter().rfind(|c| c.offset == 1111 * BS).unwrap();
        assert_eq!(home.after, data);
        assert_eq!(fs.read_file("/magic").unwrap(), data);
        assert_eq!(fs.disk().read(1111 * BS, 4), MAGIC);
    }

    #[test]
    fn ext2_mutations_record_no_journal_events() {
        let mut fs = ExtFs::format(ExtFormatOptions::default()).unwrap();
        let records = vec![
            fs.create_dir("/d").unwrap(),
            fs.create_file("/d/f", &pattern(20 * 1024)).unwrap(),
            fs.write_file("/d/f", &pattern(10)).unwrap(),
            fs.delete_file("/d/f").unwrap(),
            fs.remove_dir("/d").unwrap(),
        ];
        for rec in &records {
            assert!(
                rec.event_kinds().iter().all(|k| !JOURNAL_KINDS.contains(k)),
                "{}",
                rec.op
            );
        }
    }
}
```

Run: `cargo test -p ext --test ext3 transactions`
Expected: the build succeeds (with the Round 2 never-used warnings from the `ext` library) and ten tests fail because the mutations still take the ext2 path: `test result: FAILED. 1 passed; 10 failed; 0 ignored; 0 measured; 22 filtered out`. The passing one is `ext2_mutations_record_no_journal_events`; `a_three_block_create_in_ordered_mode_writes_the_spec_sequence` fails with `left: [(4097, 1), (1036, 8), (2060, 6), (3210, 2), (1036, 8), (2060, 6), (1137664, 3000), (1140664, 72), (6528, 128), (70684, 2), (70700, 12), (5248, 128)]` against `right: [(1120, 4), (1137664, 1024), (1138688, 1024), (1139712, 952), (1140664, 72), (83992, 8), (84992, 1024), ...]`, and `a_data_mode_create_past_the_limit_fails_and_changes_nothing` with ``called `Result::unwrap_err()` on an `Ok` value``.

- [ ] **Step 9: Implement the transaction.** In `crates/ext/src/journal/txn.rs`, replace this round's imports:

```rust
use super::{JournalMode, FEATURE_INCOMPAT_RECOVER};
use crate::fs::{BlockOwner, BlockRole};
use crate::superblock::{BLOCK_SIZE, SUPERBLOCK_OFFSET};
use fs_core::{ByteChange, Disk};
use std::collections::{BTreeMap, BTreeSet};
```

with:

```rust
use super::state::JournalState;
use super::{
    descriptor_blocks_needed, encode_commit, encode_descriptor, escape, needs_escape, CrashPhase,
    JournalMode, JournalSuperblock, Tag, FEATURE_INCOMPAT_RECOVER, TAGS_PER_DESCRIPTOR, TAG_ESCAPE,
};
use crate::events::{ExtEvent, JournalWriteKind};
use crate::fs::{BlockOwner, BlockRole, ExtFs};
use crate::group;
use crate::superblock::{Superblock, BLOCK_SIZE, SUPERBLOCK_OFFSET};
use fs_core::{ByteChange, Disk, Error, OpRecord, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
```

and insert directly above `#[cfg(test)]`:

```rust
/// Put every draft change back in reverse order, outside any operation.
fn undo(disk: &mut Disk, draft: &OpRecord) {
    for change in draft.changes.iter().rev() {
        disk.write(change.offset, &change.before);
    }
}

/// One transaction ready to emit.
struct Transaction {
    tid: u32,
    uuid: [u8; 16],
    commit_sec: u64,
    /// Step 2's writes: the data portions in ordered mode, none in data mode.
    data_changes: Vec<ByteChange>,
    tagged: Vec<u32>,
    images: Vec<Vec<u8>>,
}

/// Where the emission stopped.
enum Outcome {
    /// Every step ran; the next transaction starts at `head`.
    Completed { head: u32 },
    /// The armed phase stopped it after the change at `range`.
    Crashed {
        phase: CrashPhase,
        range: Range<usize>,
    },
}

/// Writes under the open operation, remembering the last range written.
struct Emitter<'a> {
    disk: &'a mut Disk,
    last: Range<usize>,
}

impl Emitter<'_> {
    fn write(&mut self, offset: usize, bytes: &[u8]) -> Range<usize> {
        self.disk.write(offset, bytes);
        self.last = offset..offset + bytes.len();
        self.last.clone()
    }

    fn event(&mut self, event: ExtEvent) {
        self.disk.event(Box::new(event));
    }

    /// Set or clear `needs_recovery` in the primary superblock: the four
    /// bytes of `s_feature_incompat` (steps 1 and 8).
    fn recovery_flag(&mut self, set: bool) -> Range<usize> {
        let at = SUPERBLOCK_OFFSET + INCOMPAT_OFFSET;
        let flags = le32(self.disk.read(at, 4));
        let flags = if set {
            flags | FEATURE_INCOMPAT_RECOVER
        } else {
            flags & !FEATURE_INCOMPAT_RECOVER
        };
        self.write(at, &flags.to_le_bytes())
    }

    /// `s_sequence` and `s_start` of the journal superblock, 8 bytes at
    /// 0x18 of journal index 0 (steps 3 and 7).
    fn journal_superblock(&mut self, j: &JournalState, sequence: u32, start: u32) -> Range<usize> {
        let mut bytes = [0u8; 8];
        bytes[..4].copy_from_slice(&sequence.to_be_bytes());
        bytes[4..].copy_from_slice(&start.to_be_bytes());
        self.write(j.offset(0) + JournalSuperblock::SEQUENCE_OFFSET, &bytes)
    }
}

/// Emit spec 5.2 steps 1 to 8 for `txn` under the open operation, stopping
/// where `j.armed` says (spec section 6).
fn emit(disk: &mut Disk, j: &JournalState, txn: &Transaction) -> Outcome {
    let mut out = Emitter { disk, last: 0..0 };
    let tid = txn.tid;
    let crashed = |phase, range| Outcome::Crashed { phase, range };

    // 1. needs_recovery in the primary superblock.
    let range = out.recovery_flag(true);
    out.event(ExtEvent::RecoveryFlagSet { range });
    // 2. Ordered mode: the file data home, as the draft wrote it.
    for change in &txn.data_changes {
        out.write(change.offset, &change.after);
    }
    // 3. The journal superblock names the transaction.
    let start = j.head;
    let range = out.journal_superblock(j, tid, start);
    out.event(ExtEvent::TransactionStarted {
        tid,
        journal_block: start,
        tagged: txn.tagged.len() as u32,
        range,
    });
    // 4. Descriptor blocks, each followed by the copies it tags.
    let mut index = start;
    let mut copy_index = Vec::with_capacity(txn.tagged.len());
    for d in 0..descriptor_blocks_needed(txn.tagged.len()) {
        let part = d * TAGS_PER_DESCRIPTOR..txn.tagged.len().min((d + 1) * TAGS_PER_DESCRIPTOR);
        let copies: Vec<(Tag, Vec<u8>)> = txn.tagged[part.clone()]
            .iter()
            .zip(&txn.images[part])
            .map(|(&home, image)| {
                let mut copy = image.clone();
                let flags = if needs_escape(&copy) {
                    escape(&mut copy);
                    TAG_ESCAPE
                } else {
                    0
                };
                (Tag { block: home, flags }, copy)
            })
            .collect();
        let tags: Vec<Tag> = copies.iter().map(|(tag, _)| *tag).collect();
        let range = out.write(j.offset(index), &encode_descriptor(tid, &txn.uuid, &tags));
        out.event(ExtEvent::JournalBlockWritten {
            tid,
            kind: JournalWriteKind::Descriptor,
            index,
            block: j.physical(index),
            range,
        });
        for (tag, copy) in copies {
            index = j.wrap(index + 1);
            let range = out.write(j.offset(index), &copy);
            out.event(ExtEvent::JournalBlockWritten {
                tid,
                kind: JournalWriteKind::Copy {
                    home: tag.block,
                    escaped: tag.flags & TAG_ESCAPE != 0,
                },
                index,
                block: j.physical(index),
                range,
            });
            copy_index.push(index);
        }
        index = j.wrap(index + 1);
    }
    if j.armed == Some(CrashPhase::BeforeCommit) {
        return crashed(CrashPhase::BeforeCommit, out.last);
    }
    // 5. The commit block.
    let range = out.write(j.offset(index), &encode_commit(tid, txn.commit_sec));
    out.event(ExtEvent::JournalBlockWritten {
        tid,
        kind: JournalWriteKind::Commit,
        index,
        block: j.physical(index),
        range,
    });
    let head = j.wrap(index + 1);
    if j.armed == Some(CrashPhase::AfterCommit) {
        return crashed(CrashPhase::AfterCommit, out.last);
    }
    // 6. Checkpoint: every after-image home, whole blocks, in tag order.
    for ((&home, image), &from_index) in txn.tagged.iter().zip(&txn.images).zip(&copy_index) {
        let range = out.write(home as usize * BS, image);
        out.event(ExtEvent::Checkpointed {
            tid,
            home,
            from_index,
            range,
        });
        if j.armed == Some(CrashPhase::DuringCheckpoint) {
            return crashed(CrashPhase::DuringCheckpoint, out.last);
        }
    }
    // 7. The journal is empty again.
    let range = out.journal_superblock(j, tid + 1, 0);
    out.event(ExtEvent::JournalEmptied {
        next_sequence: tid + 1,
        range,
    });
    // 8. needs_recovery cleared.
    let range = out.recovery_flag(false);
    out.event(ExtEvent::RecoveryFlagCleared { range });
    Outcome::Completed { head }
}

/// A mutation on an ext3 volume whose journal is `j` (spec 5.1 steps 2 to
/// 7): the draft, the classification, the size check, the restore, and the
/// emitted sequence. Step 1, the `NeedsRecovery` refusal, is each
/// mutation's `ensure_recovered` before its own checks. A crash is a
/// successful operation whose record stops at the armed phase. `j` goes
/// back into `fs.journal` with the new `head` and `sequence`, or marked as
/// needing recovery.
pub(crate) fn run_journaled(
    fs: &mut ExtFs,
    mut j: JournalState,
    op: String,
    body: impl FnOnce(&mut ExtFs) -> Result<()>,
) -> Result<OpRecord> {
    // 2. The draft.
    let saved = (fs.sb.clone(), fs.gds.clone(), fs.journal.clone());
    fs.disk.begin_op(op.clone());
    let result = body(fs);
    let draft = fs.disk.end_op();
    let rollback = |fs: &mut ExtFs, draft: &OpRecord, e: Error| {
        undo(&mut fs.disk, draft);
        (fs.sb, fs.gds, fs.journal) = saved;
        Err(e)
    };
    if let Err(e) = result {
        return rollback(fs, &draft, e);
    }
    // 3 and 4. Classify on the post-body disk and take the after-images.
    let classified = classify(&draft.changes, &fs.block_owners());
    let tagged = classified.tagged(j.mode);
    let images = after_images(&fs.disk, &tagged);
    // 5. The size limit.
    let n = tagged.len() as u32;
    if n > j.max_transaction() {
        let e = Error::Unsupported(format!(
            "transaction of {n} blocks exceeds the journal's maximum of {}",
            j.max_transaction()
        ));
        return rollback(fs, &draft, e);
    }
    // 6. Back to the pre-body bytes; `sb` and `gds` keep the body's values.
    undo(&mut fs.disk, &draft);
    // 7. The real operation.
    let name = match j.armed {
        Some(phase) => format!("{op} (crashed {phase})"),
        None => op,
    };
    fs.disk.begin_op(name);
    for event in draft.events {
        fs.disk.event(event);
    }
    let txn = Transaction {
        tid: j.sequence,
        uuid: fs.sb.uuid,
        commit_sec: fs.now.to_unix_seconds().max(1) as u64,
        data_changes: match j.mode {
            JournalMode::Ordered => classified.data_changes,
            JournalMode::Data => Vec::new(),
        },
        tagged,
        images,
    };
    let outcome = emit(&mut fs.disk, &j, &txn);
    if let Outcome::Crashed { phase, range } = &outcome {
        fs.disk.event(Box::new(ExtEvent::Crashed {
            phase: *phase,
            range: range.clone(),
        }));
    }
    let record = fs_core::finish_op(&mut fs.disk, &mut fs.history, Ok(()))?;
    match outcome {
        Outcome::Completed { head } => {
            j.head = head;
            j.sequence = txn.tid + 1;
        }
        Outcome::Crashed { .. } => {
            j.needs_recovery = true;
            j.armed = None;
            reload_metadata(fs);
        }
    }
    fs.journal = Some(j);
    Ok(record)
}

/// Re-decode the cached superblock and descriptors from the disk after a
/// crash, so they describe the raw on-disk state.
fn reload_metadata(fs: &mut ExtFs) {
    let bytes = fs.disk.as_bytes();
    fs.sb = Superblock::decode(&bytes[SUPERBLOCK_OFFSET..SUPERBLOCK_OFFSET + Superblock::LEN]);
    let table = fs.geo.group(0).descriptors_block.unwrap_or(2) as usize * BS;
    let len = fs.geo.descriptor_blocks as usize * BS;
    fs.gds = group::decode_table(&bytes[table..table + len], fs.geo.groups);
}
```

- [ ] **Step 10: Dispatch ext3 mutations to the transaction and gate them on recovery.** In `crates/ext/src/fs.rs`, inside `mod create`, replace:

```rust
    use crate::inode::{Inode, MODE_DIR};
    use crate::superblock::BLOCK_SIZE;
```

with:

```rust
    use crate::inode::{Inode, MODE_DIR};
    use crate::journal::txn;
    use crate::superblock::BLOCK_SIZE;
```

and in `run_mutation` replace:

```rust
        /// through bare `run_op`.
        pub(super) fn run_mutation(
            &mut self,
            op: String,
            body: impl FnOnce(&mut Self) -> Result<()>,
        ) -> Result<OpRecord> {
            let saved_sb
```

with:

```rust
        /// through bare `run_op`. On ext3 the body runs as one journal
        /// transaction instead (`txn::run_journaled`, spec section 5).
        pub(super) fn run_mutation(
            &mut self,
            op: String,
            body: impl FnOnce(&mut Self) -> Result<()>,
        ) -> Result<OpRecord> {
            if let Some(journal) = self.journal.clone() {
                return txn::run_journaled(self, journal, op, body);
            }
            let saved_sb
```

Then add the `NeedsRecovery` gate of spec 5.1 step 1, which every mutation passes right after the corruption gate. Outside `mod create`, below `ensure_mounted`, replace:

```rust
    fn ensure_mounted(&self) -> Result<()> {
        match &self.corrupt {
            None => Ok(()),
            Some(e) => Err(e.clone()),
        }
    }
```

with:

```rust
    fn ensure_mounted(&self) -> Result<()> {
        match &self.corrupt {
            None => Ok(()),
            Some(e) => Err(e.clone()),
        }
    }

    /// The gate every mutation passes right after `ensure_mounted` (ext3
    /// spec 5.1 step 1): a volume that needs recovery refuses the mutation
    /// with `NeedsRecovery` before any path, name, existence, or space
    /// check.
    fn ensure_recovered(&self) -> Result<()> {
        if self.needs_recovery() {
            return Err(Error::NeedsRecovery);
        }
        Ok(())
    }
```

In `mod create`, the check both creates share; replace:

```rust
        /// The checks both creates share, in the spec's order: the
        /// corruption gate, `InvalidPath`, `InvalidName`, `NotFound` or
        /// `NotADirectory` on the parent, `AlreadyExists`.
        fn check_new_entry(&self, path: &str) -> Result<NewEntry> {
            self.ensure_mounted()?;
```

with:

```rust
        /// The checks both creates share, in the spec's order: the
        /// corruption gate, `NeedsRecovery`, `InvalidPath`, `InvalidName`,
        /// `NotFound` or `NotADirectory` on the parent, `AlreadyExists`.
        fn check_new_entry(&self, path: &str) -> Result<NewEntry> {
            self.ensure_mounted()?;
            self.ensure_recovered()?;
```

and in the three other mutations, replace:

```rust
    fn overwrite_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        self.ensure_mounted()?;
```

with:

```rust
    fn overwrite_file(&mut self, path: &str, data: &[u8]) -> Result<OpRecord> {
        self.ensure_mounted()?;
        self.ensure_recovered()?;
```

then:

```rust
    fn unlink_file(&mut self, path: &str) -> Result<OpRecord> {
        self.ensure_mounted()?;
```

with:

```rust
    fn unlink_file(&mut self, path: &str) -> Result<OpRecord> {
        self.ensure_mounted()?;
        self.ensure_recovered()?;
```

and:

```rust
    fn unlink_dir(&mut self, path: &str) -> Result<OpRecord> {
        self.ensure_mounted()?;
```

with:

```rust
    fn unlink_dir(&mut self, path: &str) -> Result<OpRecord> {
        self.ensure_mounted()?;
        self.ensure_recovered()?;
```

No test of this round reaches the gate (their volumes never need recovery); Round 4's `a_crashed_volume_refuses_every_mutation_before_checking_its_arguments` drives it.

- [ ] **Step 11: Run the record tests, then the whole crate.**

Run: `cargo test -p ext --test ext3 transactions`
Expected: `test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 22 filtered out`.

Run: `cargo test -p ext`
Expected: every binary passes, no warnings: `85 passed` (lib), `15 passed` (e2fsprogs), `52 passed` (ext2), `33 passed` (ext3); `mount_linux` and the doc tests run 0.

#### Round 4: the armed crash

- [ ] **Step 12: Write the failing crash tests.** In `crates/ext/tests/ext3.rs`, `mod transactions`, replace the imports:

```rust
    use super::common::{changes_in_order, ext3, mask_journal, pattern};
    use ext::{
        decode_commit, decode_descriptor, BlockRole, ExtFormatOptions, ExtFs, JournalInfo,
        JournalMode, TAG_ESCAPE, TAG_LAST, TAG_SAME_UUID,
    };
```

with:

```rust
    use super::common::{changes_in_order, ext3, mask_journal, pattern};
    use ext::blockmap;
    use ext::{
        decode_commit, decode_descriptor, BlockRole, CrashPhase, ExtFormatOptions, ExtFs,
        JournalInfo, JournalMode, TAG_ESCAPE, TAG_LAST, TAG_SAME_UUID,
    };
```

Then add these items at the end of `mod transactions`: after the test `ext2_mutations_record_no_journal_events` and a blank line, before the module's closing `}` (the last line of the file):

```rust
    const PHASES: [CrashPhase; 3] = [
        CrashPhase::BeforeCommit,
        CrashPhase::AfterCommit,
        CrashPhase::DuringCheckpoint,
    ];

    /// A mutation to crash, with the state it needs first.
    struct Scenario {
        setup: fn(&mut ExtFs),
        op: fn(&mut ExtFs) -> Result<OpRecord>,
    }

    /// `create_file`, `write_file` growing and shrinking, `delete_file`,
    /// `create_dir`, `remove_dir`.
    fn scenarios() -> Vec<Scenario> {
        vec![
            Scenario {
                setup: |_| {},
                op: |fs| fs.create_file("/f", &pattern(3000)),
            },
            Scenario {
                setup: |fs| {
                    fs.create_file("/f", &pattern(1000)).unwrap();
                },
                op: |fs| fs.write_file("/f", &pattern(20 * 1024)),
            },
            Scenario {
                setup: |fs| {
                    fs.create_file("/f", &pattern(20 * 1024)).unwrap();
                },
                op: |fs| fs.write_file("/f", &pattern(100)),
            },
            Scenario {
                setup: |fs| {
                    fs.create_file("/f", &pattern(3000)).unwrap();
                },
                op: |fs| fs.delete_file("/f"),
            },
            Scenario {
                setup: |_| {},
                op: |fs| fs.create_dir("/d"),
            },
            Scenario {
                setup: |fs| {
                    fs.create_dir("/d").unwrap();
                },
                op: |fs| fs.remove_dir("/d"),
            },
        ]
    }

    /// Run `s` twice in `mode`: once whole, once with `phase` armed.
    /// Returns the crashed volume, its record, and the whole record.
    fn crash(mode: JournalMode, s: &Scenario, phase: CrashPhase) -> (ExtFs, OpRecord, OpRecord) {
        let mut whole = ext3(mode);
        (s.setup)(&mut whole);
        let full = (s.op)(&mut whole).unwrap();
        let mut fs = ext3(mode);
        (s.setup)(&mut fs);
        fs.arm_crash(phase).unwrap();
        assert_eq!(fs.crash_phase(), Some(phase));
        let rec = (s.op)(&mut fs).unwrap();
        (fs, rec, full)
    }

    #[test]
    fn each_phase_stops_where_section_6_says_for_every_mutation_in_both_modes() {
        for mode in BOTH {
            for s in scenarios() {
                for phase in PHASES {
                    let mut probe = ext3(mode);
                    (s.setup)(&mut probe);
                    let before = probe.journal_info().unwrap();
                    let history = probe.history().len();
                    let (mut fs, rec, full) = crash(mode, &s, phase);
                    let what = format!("{mode:?} {} {phase:?}", full.op);
                    // The changes: a prefix of the whole record.
                    let n = count(&full, "checkpointed");
                    let after_commit = full.changes.len() - n - 2;
                    let keep = match phase {
                        CrashPhase::BeforeCommit => after_commit - 1,
                        CrashPhase::AfterCommit => after_commit,
                        CrashPhase::DuringCheckpoint => after_commit + 1,
                    };
                    assert_eq!(rec.changes, full.changes[..keep], "{what}");
                    // The events: the same prefix, then `Crashed`.
                    let full_texts = texts(&full);
                    let commit = full_texts
                        .iter()
                        .position(|t| t.starts_with("committed transaction"))
                        .unwrap();
                    let cut = match phase {
                        CrashPhase::BeforeCommit => commit,
                        CrashPhase::AfterCommit => commit + 1,
                        CrashPhase::DuringCheckpoint => commit + 2,
                    };
                    let mut want = full_texts[..cut].to_vec();
                    want.push(format!("crashed {phase}"));
                    assert_eq!(texts(&rec), want, "{what}");
                    let last = rec.changes.last().unwrap();
                    assert_eq!(
                        rec.events.last().unwrap().region(),
                        Some(last.offset..last.offset + last.after.len()),
                        "{what}"
                    );
                    assert_eq!(rec.op, format!("{} (crashed {phase})", full.op));
                    assert_eq!(fs.history().len(), history + 1);
                    assert_eq!(fs.history().last().unwrap().op, rec.op);
                    // The volume needs recovery; the head and sequence wait.
                    assert!(fs.needs_recovery(), "{what}");
                    assert_eq!(fs.crash_phase(), None);
                    let info = fs.journal_info().unwrap();
                    assert_eq!(
                        (info.sequence, info.head, info.start, info.needs_recovery),
                        (before.sequence, before.head, before.head, true),
                        "{what}"
                    );
                    assert_eq!(fs.superblock().feature_incompat, 0x0006, "{what}");
                    assert_eq!(fs.create_dir("/other").unwrap_err(), Error::NeedsRecovery);
                    assert_eq!(fs.history().len(), history + 1);
                    // Reads, inspection, and raw writes see the raw state.
                    assert!(fs.list_dir("/").is_ok());
                    assert!(fs.stat("/").is_ok());
                    assert_eq!(fs.lookup("/lost+found"), Ok(11), "{what}");
                    let root = fs.dir_entries("/").unwrap();
                    assert!(root.iter().any(|(_, _, e)| e.name == b"lost+found"));
                    assert!(!fs.layout().is_empty());
                    assert!(!fs.block_owners().is_empty());
                    assert!(!fs.annotate_sector(1).is_empty());
                    fs.write_raw(16_000 * 1024, &[1]).unwrap();
                    assert_eq!(fs.history().len(), history + 2);
                    assert!(fs.needs_recovery());
                }
            }
        }
    }

    #[test]
    fn a_crashed_create_leaves_the_file_out_of_its_directory_in_every_phase() {
        for mode in BOTH {
            for phase in PHASES {
                let (fs, _, _) = crash(mode, &scenarios()[0], phase);
                let names: Vec<String> = fs
                    .list_dir("/")
                    .unwrap()
                    .into_iter()
                    .map(|e| e.name)
                    .collect();
                assert_eq!(names, vec!["lost+found"], "{mode:?} {phase:?}");
                assert_eq!(fs.read_file("/f").unwrap_err(), Error::NotFound);
                assert_eq!(fs.lookup("/f"), Err(Error::NotFound));
                let root = fs.dir_entries("/").unwrap();
                assert!(root.iter().all(|(_, _, e)| e.name != b"f"), "{root:?}");
            }
        }
    }

    #[test]
    fn ordered_before_commit_leaves_the_data_home_in_blocks_the_bitmap_calls_free() {
        let (fs, _, _) = crash(
            JournalMode::Ordered,
            &scenarios()[0],
            CrashPhase::BeforeCommit,
        );
        let bitmap = fs.disk().sector(3);
        let data = pattern(3000);
        for (k, block) in (1111u32..1114).enumerate() {
            let bit = (block - 1) as usize;
            assert_eq!(
                bitmap[bit / 8] & (1 << (bit % 8)),
                0,
                "block {block} is free"
            );
            let want = &data[k * BS..data.len().min((k + 1) * BS)];
            assert_eq!(&fs.disk().sector(u64::from(block))[..want.len()], want);
        }
        assert_eq!(fs.superblock().free_blocks_count, 15_205);
        // Data mode leaves nothing home.
        let (fs, _, _) = crash(JournalMode::Data, &scenarios()[0], CrashPhase::BeforeCommit);
        assert!(fs.disk().sector(1111).iter().all(|&b| b == 0));
    }

    #[test]
    fn arm_crash_is_unsupported_on_ext2() {
        let mut fs = ExtFs::format(ExtFormatOptions::default()).unwrap();
        assert_eq!(
            fs.arm_crash(CrashPhase::AfterCommit),
            Err(Error::Unsupported("this volume has no journal".into()))
        );
        assert_eq!(fs.crash_phase(), None);
        fs.disarm_crash();
        let rec = fs.create_file("/f", &pattern(3000)).unwrap();
        assert_eq!(rec.op, "create_file /f");
        assert!(!fs.needs_recovery());
    }

    #[test]
    fn disarm_crash_lets_the_next_mutation_complete() {
        let mut fs = ext3(JournalMode::Ordered);
        fs.arm_crash(CrashPhase::BeforeCommit).unwrap();
        fs.arm_crash(CrashPhase::AfterCommit).unwrap();
        assert_eq!(fs.crash_phase(), Some(CrashPhase::AfterCommit));
        fs.disarm_crash();
        assert_eq!(fs.crash_phase(), None);
        let rec = fs.create_file("/f", b"f").unwrap();
        assert_eq!(rec.op, "create_file /f");
        assert!(!fs.needs_recovery());
        assert_eq!(count(&rec, "crashed"), 0);
    }

    #[test]
    fn a_body_that_fails_rolls_back_and_keeps_the_armed_phase() {
        // A 2048-block ext3 volume filled to its last block, whose root
        // block then holds three 255-byte names: a fourth needs a new
        // directory block, and `dir_insert` fails inside the body.
        let mut fs = ExtFs::format(ExtFormatOptions {
            total_blocks: 2048,
            ..ExtFormatOptions::ext3()
        })
        .unwrap();
        let free = fs.superblock().free_blocks_count;
        let data = (1..=free)
            .rev()
            .find(|&d| d + blockmap::indirect_blocks_needed(d) == free)
            .unwrap();
        fs.create_file("/fill", &pattern(data as usize * BS))
            .unwrap();
        assert_eq!(fs.superblock().free_blocks_count, 0);
        for c in ['a', 'b', 'c'] {
            fs.create_file(&format!("/{}", c.to_string().repeat(255)), b"")
                .unwrap();
        }
        let image = fs.disk().as_bytes().to_vec();
        let (sb, info, history) = (
            fs.superblock().clone(),
            fs.journal_info(),
            fs.history().len(),
        );
        fs.arm_crash(CrashPhase::AfterCommit).unwrap();
        assert_eq!(
            fs.create_file(&format!("/{}", "d".repeat(255)), b"")
                .unwrap_err(),
            Error::DiskFull
        );
        assert!(fs.disk().as_bytes() == image.as_slice());
        assert_eq!(fs.superblock(), &sb);
        assert_eq!(fs.journal_info(), info);
        assert_eq!(fs.history().len(), history);
        assert_eq!(fs.crash_phase(), Some(CrashPhase::AfterCommit));
        assert!(!fs.needs_recovery());
    }

    #[test]
    fn a_crashed_volume_refuses_every_mutation_before_checking_its_arguments() {
        let (mut fs, _, _) = crash(
            JournalMode::Ordered,
            &scenarios()[0],
            CrashPhase::AfterCommit,
        );
        let history = fs.history().len();
        assert_eq!(
            fs.delete_file("/missing").unwrap_err(),
            Error::NeedsRecovery
        );
        assert_eq!(
            fs.create_dir("/lost+found").unwrap_err(),
            Error::NeedsRecovery
        );
        assert_eq!(
            fs.create_file("no-slash", b"").unwrap_err(),
            Error::NeedsRecovery
        );
        assert_eq!(
            fs.write_file("/missing", b"x").unwrap_err(),
            Error::NeedsRecovery
        );
        assert_eq!(fs.remove_dir("/").unwrap_err(), Error::NeedsRecovery);
        assert_eq!(fs.history().len(), history);
        fs.write_raw(16_000 * 1024, &[1]).unwrap();
        assert_eq!(fs.history().len(), history + 1);
        assert!(fs.needs_recovery());
    }

    #[test]
    fn arming_while_recovery_is_needed_is_allowed_and_waits() {
        let (mut fs, _, _) = crash(
            JournalMode::Ordered,
            &scenarios()[4],
            CrashPhase::AfterCommit,
        );
        fs.arm_crash(CrashPhase::DuringCheckpoint).unwrap();
        assert_eq!(fs.crash_phase(), Some(CrashPhase::DuringCheckpoint));
        assert_eq!(fs.create_dir("/e").unwrap_err(), Error::NeedsRecovery);
        assert_eq!(fs.crash_phase(), Some(CrashPhase::DuringCheckpoint));
    }
```

Run: `cargo test -p ext --test ext3 transactions`
Expected: the build fails with ``error[E0599]: no method named `arm_crash` found for struct `ExtFs` in the current scope`` (and the same for `crash_phase` and `disarm_crash`), ending ``error: could not compile `ext` (test "ext3") due to 16 previous errors``.

- [ ] **Step 13: Add `arm_crash`, `disarm_crash`, and `crash_phase`.** In `crates/ext/src/fs.rs`, replace:

```rust
use crate::journal::{JournalMode, JournalSuperblock, FEATURE_COMPAT_HAS_JOURNAL, JOURNAL_INO};
```

with:

```rust
use crate::journal::{
    CrashPhase, JournalMode, JournalSuperblock, FEATURE_COMPAT_HAS_JOURNAL, JOURNAL_INO,
};
```

and add the three methods after `journal_mode`; replace:

```rust
        self.journal.as_ref().map(|j| j.mode)
    }
```

with:

```rust
        self.journal.as_ref().map(|j| j.mode)
    }

    /// Make the next mutation stop at `phase` (spec section 6). Allowed
    /// while the volume needs recovery (it waits for the next mutation
    /// after `recover`); `Unsupported` on ext2.
    pub fn arm_crash(&mut self, phase: CrashPhase) -> Result<()> {
        let journal = self
            .journal
            .as_mut()
            .ok_or_else(|| Error::Unsupported("this volume has no journal".into()))?;
        journal.armed = Some(phase);
        Ok(())
    }

    /// Clear an armed crash; nothing to do on ext2.
    pub fn disarm_crash(&mut self) {
        if let Some(journal) = self.journal.as_mut() {
            journal.armed = None;
        }
    }

    /// The armed crash phase, if any; `None` on ext2.
    pub fn crash_phase(&self) -> Option<CrashPhase> {
        self.journal.as_ref().and_then(|j| j.armed)
    }
```

- [ ] **Step 14: Run the transaction tests.**

Run: `cargo test -p ext --test ext3 transactions`
Expected: `test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 22 filtered out`, including `each_phase_stops_where_section_6_says_for_every_mutation_in_both_modes`, `a_crashed_volume_refuses_every_mutation_before_checking_its_arguments`, `ordered_before_commit_leaves_the_data_home_in_blocks_the_bitmap_calls_free`, and `a_body_that_fails_rolls_back_and_keeps_the_armed_phase`.

#### Round 5: e2fsprogs

- [ ] **Step 15: Share the slice-2 scripted sequence.** In `crates/ext/tests/e2fsprogs.rs`, `mod scripted_sequence`, replace everything from the `#[test]` line of `e2fsck_is_clean_and_debugfs_agrees_after_a_scripted_sequence` through the closing `}` of `mod scripted_sequence` (the test body moves into `run_script` and `assert_tools_agree`, unchanged apart from the `big` size and the image `name`) with:

```rust
    /// The scripted sequence of spec section 9 on `fs`: `/big.bin` and
    /// `/grow.bin` reach `big` bytes (300 KiB on ext2; a data-mode ext3
    /// journal takes at most 256 blocks per transaction, so less there).
    pub(super) fn run_script(fs: &mut ExtFs, big: usize) {
        fs.create_file("/small.txt", &pattern(100)).unwrap();
        fs.create_file("/mid.bin", &pattern(20 * 1024)).unwrap();
        fs.create_file("/big.bin", &pattern(big)).unwrap();
        fs.create_file("/indirect.bin", &pattern(20 * 1024))
            .unwrap();
        fs.create_file("/grow.bin", &pattern(1024)).unwrap();
        fs.create_dir("/many").unwrap();
        for i in 0..60 {
            fs.create_file(
                &format!("/many/entry-{i:02}.txt"),
                format!("entry {i}\n").as_bytes(),
            )
            .unwrap();
        }
        // An overwrite that shrinks out of the single-indirect range, with
        // bytes shifted so every kept block changes.
        fs.write_file("/mid.bin", &pattern(5 * 1024 + 7)[7..])
            .unwrap();
        // Growth from one direct block into the double-indirect range
        // (for `big` above 268 KiB).
        fs.write_file("/grow.bin", &pattern(big + 3)[3..]).unwrap();
        // A shrink from the double-indirect range (for `big` above 268 KiB)
        // to part of one block.
        fs.write_file("/big.bin", &pattern(100)).unwrap();
        // A delete that frees a single-indirect block (20 KiB = 20 data blocks).
        fs.delete_file("/indirect.bin").unwrap();
        // Deletes: the first entry of /many's second block, and one merged into its predecessor.
        fs.delete_file("/many/entry-50.txt").unwrap();
        fs.delete_file("/many/entry-07.txt").unwrap();
        fs.create_dir("/scratch").unwrap();
        fs.remove_dir("/scratch").unwrap();
        // A directory that grows past its 12 direct blocks.
        fs.create_dir("/wide").unwrap();
        for i in 0..WIDE_ENTRIES {
            fs.create_file(&format!("/wide/entry-{i:03}"), b"").unwrap();
        }
        assert!(fs.stat("/wide").unwrap().size > 12 * 1024);
        assert_eq!(fs.stat("/grow.bin").unwrap().size, big as u64);
        assert_eq!(fs.stat("/big.bin").unwrap().size, 100);
    }

    /// `e2fsck -fn` is clean and `debugfs` lists, reads, and stats what
    /// `fs` does (skipped without the tools).
    pub(super) fn assert_tools_agree(fs: &ExtFs, name: &str) {
        let (Some(e2fsck), Some(debugfs)) = (super::tool("e2fsck"), super::tool("debugfs")) else {
            return;
        };
        let image = super::image_file(fs, name);
        let fsck = super::run(&e2fsck, &["-fn"], &image);
        let listings: Vec<(&str, std::process::Output)> = ["/", "/many", "/wide"]
            .into_iter()
            .map(|dir| {
                let request = format!("ls -l {dir}");
                (dir, super::run(&debugfs, &["-R", request.as_str()], &image))
            })
            .collect();
        let big = super::run(&debugfs, &["-R", "cat /big.bin"], &image);
        let mid = super::run(&debugfs, &["-R", "cat /mid.bin"], &image);
        let grow = super::run(&debugfs, &["-R", "cat /grow.bin"], &image);
        let big_stat = super::run(&debugfs, &["-R", "stat /big.bin"], &image);
        std::fs::remove_file(&image).unwrap();

        let report = super::both_streams(&fsck);
        assert_eq!(fsck.status.code(), Some(0), "e2fsck -fn:\n{report}");
        assert!(
            !report.contains("Fix?") && !report.contains("FIXED"),
            "e2fsck -fn:\n{report}"
        );
        for (dir, output) in listings {
            assert!(
                output.status.success(),
                "debugfs ls -l {dir}:\n{}",
                super::both_streams(&output)
            );
            let listed = parse_ls_l(&String::from_utf8_lossy(&output.stdout));
            let expected: Vec<(String, u64)> = fs
                .list_dir(dir)
                .unwrap()
                .into_iter()
                .map(|e| (e.name, e.size))
                .collect();
            assert_eq!(listed, expected, "debugfs ls -l {dir}");
        }
        assert!(
            big.stdout == fs.read_file("/big.bin").unwrap(),
            "debugfs cat /big.bin"
        );
        assert!(
            mid.stdout == fs.read_file("/mid.bin").unwrap(),
            "debugfs cat /mid.bin"
        );
        assert!(
            grow.stdout == fs.read_file("/grow.bin").unwrap(),
            "debugfs cat /grow.bin"
        );
        assert_eq!(
            parse_stat(&String::from_utf8_lossy(&big_stat.stdout)),
            (Some(fs.stat("/big.bin").unwrap().size), Some(1)),
            "debugfs stat /big.bin:\n{}",
            super::both_streams(&big_stat)
        );
    }

    #[test]
    fn e2fsck_is_clean_and_debugfs_agrees_after_a_scripted_sequence() {
        let mut fs = ExtFs::format(ExtFormatOptions::default()).unwrap();
        run_script(&mut fs, 300 * 1024);
        assert_tools_agree(&fs, "sequence");
    }
}
```

- [ ] **Step 16: Add the ext3 oracle tests.** Append to the end of `crates/ext/tests/e2fsprogs.rs` (the module takes `ext3` from `common`, which the file declares since Task 3):

```rust
/// ext3 Task 4: journaled mutations as e2fsck and debugfs see them (ext3
/// spec section 10.3, second bullet, and the e2fsck half of the third).
mod ext3_transactions {
    use super::common::ext3;
    use super::scripted_sequence::{assert_tools_agree, run_script};
    use super::{both_streams, image_file, pattern, run, tool};
    use ext::{decode_descriptor, CrashPhase, ExtFs, JournalMode};
    use fs_core::OpRecord;

    const BOTH: [JournalMode; 2] = [JournalMode::Ordered, JournalMode::Data];
    /// Journal index `i` is physical block `82 + i` on the default disk.
    const JOURNAL_BLOCK_0: usize = 82;

    /// The lines `debugfs -R "logdump -a"` prints for transaction `tid` as
    /// `rec` wrote it: the descriptor, one line per tag, and the commit.
    /// The indexes and flags come from the record's bytes: the descriptor
    /// is the first log write, the copies follow it, the commit follows
    /// them.
    fn logdump_lines(rec: &OpRecord, tid: u32, with_commit: bool) -> Vec<String> {
        let log = |offset: usize| {
            (offset / 1024)
                .checked_sub(JOURNAL_BLOCK_0)
                .filter(|index| (1..1024).contains(index))
        };
        let writes: Vec<(usize, &[u8])> = rec
            .changes
            .iter()
            .filter(|c| c.after.len() == 1024)
            .filter_map(|c| log(c.offset).map(|i| (i, c.after.as_slice())))
            .collect();
        let (descriptor, bytes) = writes[0];
        let tags = decode_descriptor(bytes).unwrap();
        let mut lines = vec![format!(
            "Found expected sequence {tid}, type 1 (descriptor block) at block {descriptor}"
        )];
        for (tag, (index, _)) in tags.iter().zip(&writes[1..]) {
            lines.push(format!(
                "FS block {} logged at journal block {index} (flags 0x{:x})",
                tag.block, tag.flags
            ));
        }
        if with_commit {
            lines.push(format!(
                "Found expected sequence {tid}, type 2 (commit block) at block {}",
                writes[tags.len() + 1].0
            ));
        }
        lines
    }

    /// What `debugfs -R "<request>"` prints about transaction `tid`: its
    /// descriptor line, the tag lines that follow it, and its commit line,
    /// trimmed. `None` without debugfs.
    fn logdump(fs: &ExtFs, name: &str, request: &str, tid: u32) -> Option<Vec<String>> {
        let debugfs = tool("debugfs")?;
        let image = image_file(fs, name);
        let output = run(&debugfs, &["-R", request], &image);
        std::fs::remove_file(&image).unwrap();
        assert!(
            output.status.success(),
            "{request}:\n{}",
            both_streams(&output)
        );
        let descriptor = format!("Found expected sequence {tid}, type 1 (descriptor block)");
        let commit = format!("Found expected sequence {tid}, type 2 (commit block)");
        let mut lines = Vec::new();
        let mut in_tags = false;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let line = line.trim();
            if line.starts_with(&descriptor) {
                lines.push(line.to_string());
                in_tags = true;
            } else if in_tags && line.starts_with("FS block ") {
                lines.push(line.to_string());
            } else if line.starts_with(&commit) {
                lines.push(line.to_string());
                break;
            } else if !line.starts_with("Dumping descriptor block") {
                in_tags = false;
            }
        }
        Some(lines)
    }

    #[test]
    fn the_scripted_sequence_is_clean_and_debugfs_agrees_in_both_modes() {
        for (mode, big) in [
            (JournalMode::Ordered, 300 * 1024),
            (JournalMode::Data, 200 * 1024),
        ] {
            let mut fs = ext3(mode);
            run_script(&mut fs, big);
            assert!(!fs.needs_recovery());
            assert_eq!(fs.journal_info().unwrap().start, 0);
            assert_tools_agree(&fs, &format!("ext3-sequence-{}", mode.as_str()));
        }
    }

    #[test]
    fn logdump_reads_our_last_transaction_tag_for_tag() {
        for mode in BOTH {
            let mut fs = ext3(mode);
            run_script(&mut fs, 200 * 1024);
            // A reload restarts the log at index 1 (spec section 4), so the
            // walk logdump makes from block 1 reaches the next transaction.
            let mut fs = ExtFs::from_image(fs.disk().as_bytes().to_vec()).unwrap();
            let tid = fs.journal_info().unwrap().sequence;
            let mut data = pattern(3000);
            data[..4].copy_from_slice(&[0xC0, 0x3B, 0x39, 0x98]);
            let rec = fs.create_file("/logged.bin", &data).unwrap();
            let want = logdump_lines(&rec, tid, true);
            let name = format!("ext3-logdump-{}", mode.as_str());
            let Some(got) = logdump(&fs, &name, "logdump -aO", tid) else {
                return;
            };
            assert_eq!(got, want, "{mode:?}");
            // Data mode tags the file's first block and escapes it.
            let escaped = want.iter().any(|l| l.ends_with("(flags 0x3)"));
            assert_eq!(escaped, mode == JournalMode::Data, "{want:?}");
        }
    }

    #[test]
    fn a_crashed_create_logs_our_tags_and_e2fsck_recovers_it_clean() {
        let (Some(e2fsck), Some(_)) = (tool("e2fsck"), tool("debugfs")) else {
            return;
        };
        for mode in BOTH {
            for phase in [
                CrashPhase::BeforeCommit,
                CrashPhase::AfterCommit,
                CrashPhase::DuringCheckpoint,
            ] {
                let mut fs = ext3(mode);
                fs.create_dir("/d").unwrap();
                let tid = fs.journal_info().unwrap().sequence;
                fs.arm_crash(phase).unwrap();
                let rec = fs.create_file("/d/f", &pattern(3000)).unwrap();
                let name = format!("ext3-crash-{}-{}", mode.as_str(), phase.as_str());
                // Plain logdump starts at s_start: our transaction.
                let with_commit = phase != CrashPhase::BeforeCommit;
                let got = logdump(&fs, &name, "logdump -a", tid).unwrap();
                assert_eq!(got, logdump_lines(&rec, tid, with_commit), "{name}");

                let image = image_file(&fs, &name);
                let fix = run(&e2fsck, &["-fy"], &image);
                let check = run(&e2fsck, &["-fn"], &image);
                std::fs::remove_file(&image).unwrap();
                let text = both_streams(&fix);
                assert!(
                    matches!(fix.status.code(), Some(0 | 1)),
                    "e2fsck -fy {name}:\n{text}"
                );
                assert!(text.contains("recovering journal"), "{name}:\n{text}");
                let text = both_streams(&check);
                assert_eq!(check.status.code(), Some(0), "e2fsck -fn {name}:\n{text}");
                assert!(!text.contains("Fix?"), "{name}:\n{text}");
            }
        }
    }
}
```

Run: `cargo test -p ext --test e2fsprogs`
Expected: `test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`, including `scripted_sequence::e2fsck_is_clean_and_debugfs_agrees_after_a_scripted_sequence`, `ext3_transactions::the_scripted_sequence_is_clean_and_debugfs_agrees_in_both_modes`, `ext3_transactions::logdump_reads_our_last_transaction_tag_for_tag`, and `ext3_transactions::a_crashed_create_logs_our_tags_and_e2fsck_recovers_it_clean`. These pass on their first run on purpose: they are regression pins for Rounds 3 and 4, which already implement the transactions and the crash, not red tests, and the external tools are the oracle they pin against. Without e2fsprogs each prints `e2fsprogs not found; skipping` and passes.

Run: `CI=1 cargo test -p ext --test e2fsprogs`
Expected: the same `18 passed`, with no skip notice.

#### Gates and commit

- [ ] **Step 17: Run every gate.**

Run: `cargo fmt --all -- --check`
Expected: no output, exit 0.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: `Finished` with no warnings.

Run: `cargo test --workspace`
Expected: every suite passes; the ext binaries report `85 passed` (lib), `18 passed` (e2fsprogs), `52 passed` (ext2), `41 passed` (ext3); FAT, fs-core, and wasm counts are unchanged from the base branch (82, 16, 34, 20; `mount_macos` 1 ignored).

Run: `cargo build -p fs-emulator-wasm --target wasm32-unknown-unknown`
Expected: `Finished`.

- [ ] **Step 18: Commit.**

```
git add crates/ext
git commit -m "feat(ext): journal transactions with an armed crash point"
```

---

### Task 5: Journal recovery

**Spec:** sections 6 (`recover()` steps 1 to 6 and the paragraph after them: a tag outside the volume or a descriptor whose copies would run past the log more than once is `CorruptImage` with nothing changed), 7 (its last two sentences: the live parse of `journal_blocks` and `recover`'s scan share one log walker, which this task provides and Task 6 consumes), 5.3 (what recovery leaves behind in each mode after each crash phase), 10.2 (the `recover()` halves of the crash-phase bullet, the "`recover()` on a clean volume" bullet, the "`from_image` of a crashed image" bullet, and the `recover()` clause of the ext2 bullet), 10.3 (the third bullet's `logdump`, `e2fsck`, and byte comparison with `recover()`, the fourth bullet (the escaped block), and the fifth (the ignored ext3 mount test)), 13 (the rulings on `recover()` for a clean volume and on loading never replaying).

`ExtFs::recover()` is jbd2's recovery as `e2fsck -fy` runs it, as one recorded operation named `recover`. A volume that does not need recovery (ext2 included) records an operation with no changes and the single event `journal is clean; nothing to replay`. Otherwise `journal::recovery::scan` reads the log before the operation opens, so its `Unsupported("journal revoke records")` and its `CorruptImage` reasons leave the disk and history untouched; then `journal::recovery::replay`, inside the operation, records `RecoveryScanned` over the journal superblock's `s_sequence`/`s_start`, writes every committed copy home as a whole block in transaction then tag order (the magic restored where `ESCAPE` is set), records `TransactionDiscarded` over the descriptor of an uncommitted transaction, empties the journal with `s_sequence = expected + 1`, and clears `needs_recovery`. Afterwards `sb` and `gds` are re-read and validated by the corruption gate's own `reparse_metadata`, and the session's journal restarts at the first log block with the new sequence. `recover` goes through `run_op`, never through the transaction builder, so an armed crash does not touch it and still waits for the next mutation. The log itself is read by one walker, `journal::recovery::LogWalk`: from `s_start` with `expected = s_sequence` it yields every block with the magic, the expected sequence, and a log block type (a descriptor with its tags and the journal index each copy sits at, a commit, a revoke block) and ends at the first block that does not fit. `scan` turns what it yields into transactions and refuses a revoke block; Task 6's live parse consumes the same walker and shows the revoke block instead.

Everything below was compiled and run on a scratch branch holding Tasks 1 to 4 (after their critique fixes) plus this task: fmt, workspace clippy, `cargo test --workspace` including the e2fsprogs oracle (also with `CI=1`, nothing skipped), and the wasm32 build. The steps were then replayed from that Task-4 state by a script that applied exactly the code blocks below in order, then Task 6's, and ran each command; every "Expected" is the observed output, every intermediate state passed `cargo fmt --all -- --check`, and the replayed tree was byte-identical to the branch. Copy the code verbatim.

Verified by hand with e2fsprogs 1.47.4 before the tests were written, on the images of `create_dir /d` then a 3-block `create_file /d/f` crashed at each phase in each mode: `e2fsck -fy` exits 0 and prints `recovering journal`, and `e2fsck -fn` is then clean. After a commit (after-commit and during-checkpoint), e2fsck's image differs from the uncrashed image only in the primary superblock's `s_wtime` (0x30) and `s_lastcheck` (0x40) and in byte 0x1B of the journal superblock: e2fsck writes `s_sequence` 4 (tid + 2, spec 6 step 4) where the uncrashed volume has 3. Before the commit it equals the pre-operation image plus the journal's log blocks, plus (ordered mode) the file's data in blocks the bitmap calls free, with `s_sequence` 3 (tid + 1). The backup superblock at block 8193 is untouched. On a hand-planted log of two committed transactions that wraps from journal block 1021 to 1, rewrites a block in the second transaction, escapes one tag, and ends with an uncommitted third, e2fsck writes the same bytes as `recover()` apart from `s_wtime` and `s_lastcheck`, with `s_sequence` 10. So the comparison needs no exclusion beyond the spec's list; the "reserved tail" is e2fsprogs' `s_reserved` (0x284..0x3FC), and it never differed.

Decisions later tasks must know:
- `recover()` in order: the corruption gate (`ensure_mounted`: a corrupt volume returns the gate's error with nothing recorded); the clean case (`needs_recovery` false, or ext2), whose `RecoveryScanned` carries `start: 0`, the session's `sequence` (0 on ext2), and `range: 0..0` (spec 6 step 1; its `region()` is `Some(0..0)`, an empty range, unlike every other ext event); `scan`; the operation (`run_op`); `reparse_metadata()`; then `journal.sequence = next_sequence`, `journal.head = journal.first`, `journal.needs_recovery = false`. `journal.armed` survives.
- A volume whose `needs_recovery` flag is set over an empty journal (`s_start == 0`, e.g. a raw write of the flag) records `RecoveryScanned { start: 0, .. }` over 0x18..0x20 of journal index 0 (Display `journal is clean; nothing to replay`), then `JournalEmptied` with `s_sequence + 1`, then `RecoveryFlagCleared`.
- The one log walker is `journal::recovery::LogWalk::new(disk, journal, start, sequence)`, an iterator of `LogEntry { index, sequence, block: LogBlock }` with `LogBlock::{Descriptor(Result<Vec<(u32, Tag)>>), Commit, Revoke}`; `parse_block(disk, journal, index)` parses one block the same way (Task 6's stale walk uses it). It checks each block in jbd2's order (the magic, then the sequence, then the type), so a block with another sequence or of another type ends the walk; a descriptor's copies are skipped and a commit moves `expected` on; a revoke block is yielded and the walk goes on. It also ends after a descriptor whose tags do not parse (yielded with the error) or whose copies run past the end of the log more than once, and when it has walked more than `maxlen - first` blocks, which `lapped()` then reports; a `start` of 0 or outside the log walks nothing.
- `scan` consumes the walker: a revoke block (only one of the expected sequence reaches it) is `Unsupported`; a commit with no descriptor before it is a committed transaction with no tags; descriptors of one sequence merge into one transaction (`descriptor_index` is the first). Beyond the spec's two `CorruptImage` cases (`journal descriptor at journal block {pos} tags block {b}, outside the {n}-block volume`; `journal descriptor at journal block {pos} tags {n} blocks, which run past the end of the {maxlen}-block log more than once`) and `decode_descriptor`'s own, a lapped walk is `CorruptImage("the journal log from journal block {start} runs more than once around the ring")`, so a planted log whose descriptors lead back to themselves cannot hang `recover`. "Outside the volume" is `block >= disk.sector_count()` (the disk holds exactly `s_blocks_count` blocks).
- `journal::recovery::{LogBlock, LogEntry, parse_block, LogWalk, ScannedTransaction, scan, replay}` are `pub(crate)`.
- `crates/ext/src/journal/mod.rs` declares `pub mod recovery;` where rustfmt sorts it, above `pub mod state;`, and the module doc gains one line for `recovery`.
- `crates/ext/tests/ext3.rs`: `mod transactions` now shares `BOTH`, `PHASES`, `FLAG`, `JSB_SEQ`, `MAGIC`, `whole`, `texts`, `Scenario` (with its fields), and `scenarios` as `pub(super)`; `mod recovery` imports them with `use super::transactions::{..}` and keeps only its own `crash` (which returns `Crashed`, with the pre-operation image and the tid) and `be32`.
- `crates/ext/tests/common/mod.rs` gains `SUPERBLOCK_IGNORED` (the spec's superblock exclusions as `(offset, length, name)`) and `assert_images_agree(ours, theirs, geo, whose)`: every block byte-equal except those fields in the superblock copies; the panic names the first differing block and offset and says whose bytes `theirs` are (`"e2fsck's"`, `"the kernel's"`). `e2fsprogs.rs` and `mount_linux.rs` both use it through `mod common;`.
- `crates/ext/tests/e2fsprogs.rs`: `ext3_transactions::{logdump, logdump_lines}` are now `pub(super)`; Task 4's `a_crashed_create_logs_our_tags_and_e2fsck_recovers_it_clean` is deleted, because `ext3_recovery::e2fsck_replays_a_crashed_create_to_the_bytes_recover_writes` runs the same scenario with the same `logdump -a` assertion and the same `e2fsck -fy`/`-fn` checks (in `e2fsck_recovered(fs, name) -> Option<Vec<u8>>`, which also returns the repaired bytes); `ext3_recovery` takes `ext3` from `common`.
- `crates/ext/tests/mount_linux.rs` compares the kernel's replay under the spec's masking (`mask_journal` on both images, then `assert_images_agree`), not byte for byte like the e2fsck oracle: the kernel's unmount rewrites journal superblock fields its own way.

**Files:**
- Create: `crates/ext/src/journal/recovery.rs` (626 lines): module doc (lines 1-7), imports and constants (lines 9-20), `LogBlock`, `LogEntry`, `parse_block` (lines 22-65), `LogWalk` and its `Iterator` impl (lines 67-152), `ScannedTransaction` (lines 154-167), `scan` (lines 169-236), `replay` (lines 238-310), unit tests (lines 312-626).
- Modify: `crates/ext/src/journal/mod.rs`: the `recovery` line of the module doc (line 8), `pub mod recovery;` (line 10).
- Modify: `crates/ext/src/fs.rs`: `use crate::journal::recovery;` (line 15), `recover` after `write_raw` (lines 903-947).
- Test: `crates/ext/tests/ext3.rs`: in `mod transactions`, the shared constants (lines 624-631), `whole` and `texts` (lines 647-654), `PHASES`, `Scenario`, `scenarios` (lines 1200-1214) made `pub(super)`; `mod recovery` appended (lines 1497-2028): imports, `be32`, and the crash helper (lines 1500-1542), the record tests (lines 1544-1850), the image helpers (lines 1852-1886), the image-equality tests (lines 1888-2027).
- Test: `crates/ext/tests/common/mod.rs`: `Geometry` imported (line 6); `SUPERBLOCK_IGNORED` and `assert_images_agree` appended (lines 61-102).
- Test: `crates/ext/tests/e2fsprogs.rs`: in `mod ext3_transactions`, `CrashPhase` dropped from the imports (line 606), `logdump_lines` and `logdump` made `pub(super)` (lines 618 and 653), `a_crashed_create_logs_our_tags_and_e2fsck_recovers_it_clean` deleted; `mod ext3_recovery` appended (lines 723-881).
- Test: `crates/ext/tests/mount_linux.rs`: module doc, `mod common;`, and imports (lines 1-11); `linux_replays_a_crashed_ext3_image_like_recover` appended (lines 78-132).

**Interfaces:**

Consumes (from Tasks 1 to 4, on the base branch):
- `fs_core::{Disk, Error, OpRecord, Result, ByteChange}` with `Disk::{read, write, event, begin_op, end_op, sector_count, as_bytes, sector}`; `Error::{Unsupported, CorruptImage}`.
- From `crate::journal` (Task 2): `read_header -> Option<BlockHeader { blocktype, sequence }>`, `decode_descriptor(&[u8]) -> Result<Vec<Tag>>`, `unescape`, `Tag { block, flags }`, `JournalSuperblock::{SEQUENCE_OFFSET, LEN, new, encode}`, `BLOCKTYPE_DESCRIPTOR`, `BLOCKTYPE_COMMIT`, `BLOCKTYPE_REVOKE`, `TAG_ESCAPE`, `TAG_SAME_UUID`, `TAG_LAST`, `FEATURE_INCOMPAT_RECOVER`, `encode_descriptor`, `encode_commit`, `write_header`, `JournalMode`, `CrashPhase`; the same names from the crate root in tests (`ext::...`).
- `crate::journal::state::JournalState { blocks, maxlen, first, sequence, head, mode, needs_recovery, armed }` with `superblock(&Disk)`, `offset`, `wrap` (Task 3); `ExtFs::{journal_info, needs_recovery, superblock, group_descriptors, geometry, block_owners, corruption, write_raw, arm_crash, crash_phase}` and the private `ensure_mounted`, `run_op`, `reparse_metadata` in `fs.rs`; `Geometry { groups_layout: Vec<GroupLayout { first_block, superblock_block, .. }> }`, `Geometry::group_of_block`.
- `crate::events::ExtEvent::{RecoveryScanned, Replayed, TransactionDiscarded, JournalEmptied, RecoveryFlagCleared}` with their fixed Display strings (Task 4); the crashed state Task 4 leaves (`needs_recovery()` true, `head`/`sequence` at their pre-transaction values, `sb`/`gds` re-decoded from the disk, `journal_info().start` = the transaction's start).
- Test helpers: `common::{ext3, pattern, mask_journal, changes_in_order}` (Tasks 3 and 4); in `tests/ext3.rs` Task 4's `mod transactions` items listed under Files (this task makes them `pub(super)`); in `e2fsprogs.rs` the private `tool`, `image_file`, `run`, `both_streams`, `pattern` (the file's `use common::pattern;`), and `ext3_transactions::{logdump, logdump_lines}` (Task 4; this task makes them `pub(super)`).

Produces (exact):

```rust
// crates/ext/src/fs.rs
impl ExtFs {
    /// Spec section 6 as the recorded operation "recover".
    pub fn recover(&mut self) -> Result<OpRecord>;
}

// crates/ext/src/journal/recovery.rs  (pub mod recovery; items crate-private)
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LogBlock {
    Descriptor(Result<Vec<(u32, Tag)>>),   // (journal index of the copy, tag), or the tags' parse error
    Commit,
    Revoke,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LogEntry { pub index: u32, pub sequence: u32, pub block: LogBlock }
pub(crate) fn parse_block(disk: &Disk, journal: &JournalState, index: u32) -> Option<LogEntry>;   // None: no magic, or not type 1, 2, or 5
pub(crate) struct LogWalk<'a> { .. }   // impl Iterator<Item = LogEntry>
impl<'a> LogWalk<'a> {
    pub(crate) fn new(disk: &'a Disk, journal: &'a JournalState, start: u32, sequence: u32) -> Self;
    pub(crate) fn lapped(&self) -> bool;   // the walk ended by coming around the ring
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScannedTransaction {
    pub tid: u32,
    pub descriptor_index: u32,        // journal index of its first descriptor (of its commit when it tags nothing)
    pub tags: Vec<(u32, Tag)>,        // (journal index of the copy, tag), in log order
    pub committed: bool,
}
pub(crate) fn scan(disk: &Disk, journal: &JournalState) -> Result<Vec<ScannedTransaction>>;
pub(crate) fn replay(disk: &mut Disk, journal: &JournalState, found: &[ScannedTransaction]) -> Result<u32>;   // returns the new s_sequence

// Errors of scan (nothing written, nothing recorded):
// Unsupported("journal revoke records")
// CorruptImage("journal descriptor at journal block {pos} tags block {b}, outside the {n}-block volume")
// CorruptImage("journal descriptor at journal block {pos} tags {n} blocks, which run past the end of the {maxlen}-block log more than once")
// CorruptImage("the journal log from journal block {start} runs more than once around the ring")

// crates/ext/tests/common/mod.rs
pub const SUPERBLOCK_IGNORED: [(usize, usize, &str); 7];
pub fn assert_images_agree(ours: &[u8], theirs: &[u8], geo: &Geometry, whose: &str);
```

The `recover` record after a crash (spec section 6; journal index `i` is physical block `82 + i` on the default disk):
1. No change. `RecoveryScanned { start: s_start, sequence: s_sequence, committed, tagged, range: 82 * 1024 + 0x18..+8 }`.
2. Per committed transaction in order, per tag in order: 1024 bytes at `home * 1024`, the copy (magic restored when `ESCAPE`). `Replayed { tid, home, from_index, range: home * 1024..+1024 }`. An uncommitted transaction: no change, `TransactionDiscarded { tid, tagged, range: its descriptor block }`.
3. 8 bytes at journal index 0 + 0x18: be32 `s_sequence + committed + 1`, be32 0. `JournalEmptied { next_sequence, range }`.
4. 4 bytes at 1024 + 0x60: the incompat word with 0x0004 cleared (0x0006 → 0x0002). `RecoveryFlagCleared { range }`.

The clean record: no change, one `RecoveryScanned { start: 0, sequence, committed: 0, tagged: 0, range: 0..0 }`.

Gate commands used throughout (run from the repository root):

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p fs-emulator-wasm --target wasm32-unknown-unknown
```

---

#### Round 1: the log walk and the scan

- [ ] **Step 1: Declare the module and write the failing walk and scan tests.** In `crates/ext/src/journal/mod.rs`, replace the end of the module doc and the module lines:

```rust
//! `txn` turns each mutation into one transaction (spec section 5).

pub mod state;
pub mod txn;
```

with (rustfmt keeps these lines sorted, so `recovery` goes above `state`):

```rust
//! `txn` turns each mutation into one transaction (spec section 5);
//! `recovery` scans and replays the log as a mount does (section 6).

pub mod recovery;
pub mod state;
pub mod txn;
```

Create `crates/ext/src/journal/recovery.rs` with the module doc and the unit tests of the walk and the scan (the implementation goes above `#[cfg(test)]` in Step 3). The helpers build a 64-block disk whose journal of `maxlen` blocks sits at blocks 20 and up, so the ring and the "past the end twice" case can use an 8- or 16-block log:

```rust
//! Recovery (spec section 6): the walk of the log from `s_start` that
//! jbd2's scan (`PASS_SCAN`) makes, which `journal_blocks` shares for its
//! live parse (spec section 7); the scan built on it; then the replay that
//! writes every committed copy home, empties the journal, and clears
//! `needs_recovery`. `ExtFs::recover` runs `scan` before it opens the
//! `recover` operation, so every check fails with nothing written, and
//! `replay` inside it.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::{
        encode_commit, encode_descriptor, write_header, JournalMode, JournalSuperblock, TAG_ESCAPE,
        TAG_LAST, TAG_SAME_UUID,
    };

    /// A 64-block disk whose journal of `maxlen` blocks sits at blocks
    /// 20.., with `s_sequence` and `s_start` as given.
    fn journal(maxlen: u32, sequence: u32, start: u32) -> (Disk, JournalState) {
        let mut disk = Disk::new(BS, 64);
        let mut jsb = JournalSuperblock::new(maxlen, [7; 16]);
        jsb.sequence = sequence;
        jsb.start = start;
        let mut bytes = [0u8; JournalSuperblock::LEN];
        jsb.encode(&mut bytes);
        disk.write(20 * BS, &bytes);
        let state = JournalState {
            blocks: (20..20 + maxlen).collect(),
            maxlen,
            first: 1,
            sequence,
            head: 1,
            mode: JournalMode::Ordered,
            needs_recovery: true,
            armed: None,
        };
        (disk, state)
    }

    fn put(disk: &mut Disk, j: &JournalState, index: u32, block: &[u8]) {
        disk.write(j.offset(index), block);
    }

    fn descriptor(sequence: u32, homes: &[u32]) -> [u8; 1024] {
        let tags: Vec<Tag> = homes.iter().map(|&block| Tag { block, flags: 0 }).collect();
        encode_descriptor(sequence, &[7; 16], &tags)
    }

    fn header(blocktype: u32, sequence: u32) -> [u8; 1024] {
        let mut block = [0u8; 1024];
        write_header(&mut block, blocktype, sequence);
        block
    }

    fn homes(t: &ScannedTransaction) -> Vec<(u32, u32)> {
        t.tags
            .iter()
            .map(|&(index, tag)| (index, tag.block))
            .collect()
    }

    #[test]
    fn the_walk_yields_a_revoke_block_and_ends_after_a_broken_descriptor() {
        let (mut disk, j) = journal(16, 5, 1);
        put(&mut disk, &j, 1, &descriptor(5, &[40]));
        put(&mut disk, &j, 3, &header(BLOCKTYPE_REVOKE, 5));
        put(&mut disk, &j, 4, &encode_commit(5, 1));
        // Transaction 6: a revoke block, then a descriptor whose tags run
        // off the block without a last tag; the commit after it is never
        // reached.
        put(&mut disk, &j, 5, &header(BLOCKTYPE_REVOKE, 6));
        put(&mut disk, &j, 6, &header(BLOCKTYPE_DESCRIPTOR, 6));
        put(&mut disk, &j, 7, &encode_commit(6, 1));
        let mut walk = LogWalk::new(&disk, &j, 1, 5);
        let entries: Vec<LogEntry> = walk.by_ref().collect();
        let tag = Tag {
            block: 40,
            flags: TAG_LAST,
        };
        let entry = |index, sequence, block| LogEntry {
            index,
            sequence,
            block,
        };
        assert_eq!(
            entries[..4],
            [
                entry(1, 5, LogBlock::Descriptor(Ok(vec![(2, tag)]))),
                entry(3, 5, LogBlock::Revoke),
                entry(4, 5, LogBlock::Commit),
                entry(5, 6, LogBlock::Revoke),
            ]
        );
        assert_eq!(entries.len(), 5);
        assert_eq!((entries[4].index, entries[4].sequence), (6, 6));
        assert!(matches!(entries[4].block, LogBlock::Descriptor(Err(_))));
        assert!(!walk.lapped());
    }

    #[test]
    fn an_empty_journal_scans_to_nothing() {
        let (mut disk, j) = journal(16, 5, 0);
        // A descriptor at the first log block is ignored while s_start is 0.
        put(&mut disk, &j, 1, &descriptor(5, &[40]));
        assert_eq!(scan(&disk, &j).unwrap(), vec![]);
    }

    #[test]
    fn a_committed_transaction_then_an_uncommitted_one() {
        let (mut disk, j) = journal(16, 5, 3);
        let tags = [
            Tag {
                block: 40,
                flags: 0,
            },
            Tag {
                block: 41,
                flags: TAG_ESCAPE,
            },
        ];
        put(&mut disk, &j, 3, &encode_descriptor(5, &[7; 16], &tags));
        put(&mut disk, &j, 6, &encode_commit(5, 1));
        put(&mut disk, &j, 7, &descriptor(6, &[42]));
        let found = scan(&disk, &j).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].tid, found[0].descriptor_index), (5, 3));
        assert!(found[0].committed);
        assert_eq!(homes(&found[0]), vec![(4, 40), (5, 41)]);
        assert_eq!(
            found[0].tags[1].1.flags,
            TAG_ESCAPE | TAG_SAME_UUID | TAG_LAST
        );
        assert_eq!((found[1].tid, found[1].descriptor_index), (6, 7));
        assert!(!found[1].committed);
        assert_eq!(homes(&found[1]), vec![(8, 42)]);
    }

    #[test]
    fn the_scan_follows_the_ring_past_the_last_block() {
        let (mut disk, j) = journal(16, 5, 14);
        put(&mut disk, &j, 14, &descriptor(5, &[40, 41]));
        put(&mut disk, &j, 2, &encode_commit(5, 1));
        put(&mut disk, &j, 3, &descriptor(6, &[42]));
        put(&mut disk, &j, 5, &encode_commit(6, 1));
        let found = scan(&disk, &j).unwrap();
        assert_eq!(homes(&found[0]), vec![(15, 40), (1, 41)]);
        assert_eq!(homes(&found[1]), vec![(4, 42)]);
        assert!(found.iter().all(|t| t.committed));
    }

    #[test]
    fn two_descriptors_of_one_sequence_make_one_transaction() {
        let (mut disk, j) = journal(16, 5, 1);
        put(&mut disk, &j, 1, &descriptor(5, &[40]));
        put(&mut disk, &j, 3, &descriptor(5, &[41]));
        put(&mut disk, &j, 5, &encode_commit(5, 1));
        let found = scan(&disk, &j).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].descriptor_index, 1);
        assert_eq!(homes(&found[0]), vec![(2, 40), (4, 41)]);
        assert!(found[0].committed);
    }

    #[test]
    fn a_commit_without_a_descriptor_is_an_empty_committed_transaction() {
        let (mut disk, j) = journal(16, 5, 1);
        put(&mut disk, &j, 1, &encode_commit(5, 1));
        let found = scan(&disk, &j).unwrap();
        assert_eq!(
            found,
            vec![ScannedTransaction {
                tid: 5,
                descriptor_index: 1,
                tags: vec![],
                committed: true,
            }]
        );
    }

    #[test]
    fn a_stale_sequence_an_unknown_type_or_a_missing_magic_ends_the_log() {
        // After transaction 5 commits, the scan expects sequence 6.
        for end in [
            // A stale descriptor.
            descriptor(4, &[42]),
            // A stale revoke block: the sequence is checked first.
            header(BLOCKTYPE_REVOKE, 4),
            // A version-1 superblock header: not a log block type.
            header(3, 6),
            // No magic.
            [0u8; 1024],
        ] {
            let (mut disk, j) = journal(16, 5, 1);
            put(&mut disk, &j, 1, &descriptor(5, &[40]));
            put(&mut disk, &j, 3, &encode_commit(5, 1));
            put(&mut disk, &j, 4, &end);
            let found = scan(&disk, &j).unwrap();
            assert_eq!(found.len(), 1);
            assert!(found[0].committed);
        }
    }

    #[test]
    fn a_revoke_block_of_the_expected_sequence_is_unsupported() {
        let (mut disk, j) = journal(16, 5, 1);
        put(&mut disk, &j, 1, &descriptor(5, &[40]));
        put(&mut disk, &j, 3, &header(BLOCKTYPE_REVOKE, 5));
        assert_eq!(
            scan(&disk, &j),
            Err(Error::Unsupported("journal revoke records".into()))
        );
    }

    #[test]
    fn a_tag_outside_the_volume_is_corrupt() {
        let (mut disk, j) = journal(16, 5, 1);
        put(&mut disk, &j, 1, &descriptor(5, &[40, 64]));
        assert_eq!(
            scan(&disk, &j),
            Err(Error::CorruptImage(
                "journal descriptor at journal block 1 tags block 64, outside the 64-block volume"
                    .into()
            ))
        );
    }

    #[test]
    fn a_descriptor_whose_copies_pass_the_end_twice_is_corrupt() {
        // An 8-block log holds indexes 1..=7; eight copies after index 7
        // would wrap past the end twice.
        let (mut disk, j) = journal(8, 5, 7);
        put(&mut disk, &j, 7, &descriptor(5, &[40; 8]));
        assert_eq!(
            scan(&disk, &j),
            Err(Error::CorruptImage(
                "journal descriptor at journal block 7 tags 8 blocks, which run past the end \
                 of the 8-block log more than once"
                    .into()
            ))
        );
    }

    #[test]
    fn a_log_that_comes_around_the_ring_is_corrupt() {
        // One descriptor of six copies fills the 7-block log and leads back
        // to itself with the same sequence.
        let (mut disk, j) = journal(8, 5, 1);
        put(&mut disk, &j, 1, &descriptor(5, &[40; 6]));
        assert_eq!(
            scan(&disk, &j),
            Err(Error::CorruptImage(
                "the journal log from journal block 1 runs more than once around the ring".into()
            ))
        );
    }
}
```

- [ ] **Step 2: Run the tests to see them fail.**

Run: `cargo test -p ext --lib journal::recovery`
Expected: FAIL to compile, with (among others) ``error[E0425]: cannot find function `scan` in this scope``, ``error[E0433]: cannot find type `LogWalk` in this scope``, ``error[E0422]: cannot find struct, variant or union type `ScannedTransaction` in this scope``, and ``error: could not compile `ext` (lib test) due to 42 previous errors; 1 warning emitted``.

- [ ] **Step 3: Implement the walk and `scan`.** In `crates/ext/src/journal/recovery.rs`, insert directly above `#[cfg(test)]`, followed by one blank line:

```rust
use super::state::JournalState;
use super::{
    decode_descriptor, read_header, Tag, BLOCKTYPE_COMMIT, BLOCKTYPE_DESCRIPTOR, BLOCKTYPE_REVOKE,
};
use crate::superblock::BLOCK_SIZE;
use fs_core::{Disk, Error, Result};

const BS: usize = BLOCK_SIZE as usize;

/// What a log block holds.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LogBlock {
    /// A descriptor's tags, each with the journal index of the copy it
    /// names (the ring rule from the descriptor's index), or the error when
    /// the tags do not parse.
    Descriptor(Result<Vec<(u32, Tag)>>),
    Commit,
    Revoke,
}

/// One log block: its journal index, its header sequence, what it holds.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LogEntry {
    pub index: u32,
    pub sequence: u32,
    pub block: LogBlock,
}

/// Parse journal index `index` as a log block: `None` without the magic or
/// for a block type other than descriptor, commit, or revoke.
pub(crate) fn parse_block(disk: &Disk, journal: &JournalState, index: u32) -> Option<LogEntry> {
    let bytes = disk.read(journal.offset(index), BS);
    let header = read_header(bytes)?;
    let block = match header.blocktype {
        BLOCKTYPE_DESCRIPTOR => LogBlock::Descriptor(decode_descriptor(bytes).map(|tags| {
            let mut at = index;
            tags.into_iter()
                .map(|tag| {
                    at = journal.wrap(at + 1);
                    (at, tag)
                })
                .collect()
        })),
        BLOCKTYPE_COMMIT => LogBlock::Commit,
        BLOCKTYPE_REVOKE => LogBlock::Revoke,
        _ => return None,
    };
    Some(LogEntry {
        index,
        sequence: header.sequence,
        block,
    })
}

/// The log from `start` with `expected = sequence`, as jbd2's scan walks it
/// (spec section 6, step 2): every block with the magic, the expected
/// sequence, and a log block type, in log order. A descriptor's copies are
/// skipped (`pos = wrap(pos + 1 + tags)`), a commit moves `expected` on,
/// and a revoke block is yielded and the walk goes on: what it means is the
/// caller's (`scan` refuses it, the live parse of `journal_blocks` shows
/// it). The walk ends at the first block that does not fit, after a
/// descriptor whose tags do not parse or whose copies run past the end of
/// the log more than once, and when it has come around the ring (more than
/// `maxlen - first` blocks; see `lapped`). Nothing is walked when `start`
/// is 0 (an empty journal) or outside the log.
pub(crate) struct LogWalk<'a> {
    disk: &'a Disk,
    journal: &'a JournalState,
    pos: u32,
    expected: u32,
    walked: u32,
    lapped: bool,
    done: bool,
}

impl<'a> LogWalk<'a> {
    pub(crate) fn new(
        disk: &'a Disk,
        journal: &'a JournalState,
        start: u32,
        sequence: u32,
    ) -> Self {
        LogWalk {
            disk,
            journal,
            pos: start,
            expected: sequence,
            walked: 0,
            lapped: false,
            done: !(journal.first..journal.maxlen).contains(&start),
        }
    }

    /// Whether the walk ended because it came around the ring.
    pub(crate) fn lapped(&self) -> bool {
        self.lapped
    }
}

impl Iterator for LogWalk<'_> {
    type Item = LogEntry;

    fn next(&mut self) -> Option<LogEntry> {
        let lap = self.journal.maxlen - self.journal.first;
        if self.done {
            return None;
        }
        if self.walked > lap {
            self.lapped = true;
            self.done = true;
            return None;
        }
        let entry = parse_block(self.disk, self.journal, self.pos)
            .filter(|entry| entry.sequence == self.expected);
        let Some(entry) = entry else {
            self.done = true;
            return None;
        };
        let len = match &entry.block {
            LogBlock::Descriptor(Ok(tags)) => 1 + tags.len() as u32,
            LogBlock::Descriptor(Err(_)) => {
                self.done = true;
                return Some(entry);
            }
            LogBlock::Commit => {
                self.expected = self.expected.wrapping_add(1);
                1
            }
            LogBlock::Revoke => 1,
        };
        self.walked += len;
        // `wrap` folds one pass past the end of the log, no more.
        if self.pos + len >= self.journal.maxlen + lap {
            self.done = true;
        } else {
            self.pos = self.journal.wrap(self.pos + len);
        }
        Some(entry)
    }
}

/// One transaction the scan found in the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScannedTransaction {
    pub tid: u32,
    /// The journal index of its first descriptor block (of its commit
    /// block for a transaction that tags nothing).
    pub descriptor_index: u32,
    /// Every tag of its descriptor blocks in log order, each with the
    /// journal index of the copy it names.
    pub tags: Vec<(u32, Tag)>,
    /// Whether its commit block followed; an uncommitted transaction is
    /// always the last one found, and recovery discards it.
    pub committed: bool,
}

/// The transactions `LogWalk` finds from `s_start` (spec section 6, step
/// 2): a descriptor's tags join the transaction of its sequence, and a
/// commit closes it. Nothing is found when `s_start` is 0. `Unsupported`
/// for a revoke block; `CorruptImage` for tags that do not parse, a tag
/// outside the volume (the disk holds exactly the volume's blocks), a
/// descriptor whose copies would run past the end of the log more than
/// once, or a walk that comes around the ring without ending.
pub(crate) fn scan(disk: &Disk, journal: &JournalState) -> Result<Vec<ScannedTransaction>> {
    let jsb = journal.superblock(disk)?;
    let volume = disk.sector_count();
    let lap = journal.maxlen - journal.first;
    let mut found: Vec<ScannedTransaction> = Vec::new();
    let mut walk = LogWalk::new(disk, journal, jsb.start, jsb.sequence);
    for LogEntry {
        index: pos,
        sequence: tid,
        block,
    } in walk.by_ref()
    {
        match block {
            LogBlock::Descriptor(tags) => {
                let tags = tags?;
                let n = tags.len() as u32;
                if pos + 1 + n >= journal.maxlen + lap {
                    return Err(Error::CorruptImage(format!(
                        "journal descriptor at journal block {pos} tags {n} blocks, which run \
                         past the end of the {}-block log more than once",
                        journal.maxlen
                    )));
                }
                if let Some((_, tag)) = tags.iter().find(|(_, tag)| u64::from(tag.block) >= volume)
                {
                    return Err(Error::CorruptImage(format!(
                        "journal descriptor at journal block {pos} tags block {}, outside the \
                         {volume}-block volume",
                        tag.block
                    )));
                }
                match found.last_mut() {
                    Some(t) if t.tid == tid => t.tags.extend(tags),
                    _ => found.push(ScannedTransaction {
                        tid,
                        descriptor_index: pos,
                        tags,
                        committed: false,
                    }),
                }
            }
            LogBlock::Commit => match found.last_mut() {
                Some(t) if t.tid == tid => t.committed = true,
                _ => found.push(ScannedTransaction {
                    tid,
                    descriptor_index: pos,
                    tags: Vec::new(),
                    committed: true,
                }),
            },
            LogBlock::Revoke => return Err(Error::Unsupported("journal revoke records".into())),
        }
    }
    if walk.lapped() {
        return Err(Error::CorruptImage(format!(
            "the journal log from journal block {} runs more than once around the ring",
            jsb.start
        )));
    }
    Ok(found)
}
```

- [ ] **Step 4: Run the walk and scan tests.**

Run: `cargo test -p ext --lib journal::recovery`
Expected: PASS, `test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 85 filtered out`.

#### Round 2: the replay and `recover()`

- [ ] **Step 5: Write the failing replay unit test.** In `crates/ext/src/journal/recovery.rs`, `mod tests` ends with `a_log_that_comes_around_the_ring_is_corrupt`; add this test after it (one blank line between), before the module's closing `}`:

```rust
    #[test]
    fn replay_writes_committed_copies_home_and_empties_the_journal() {
        let (mut disk, j) = journal(16, 5, 1);
        disk.write(
            SUPERBLOCK_OFFSET + INCOMPAT_OFFSET,
            &0x0006u32.to_le_bytes(),
        );
        let tags = [
            Tag {
                block: 40,
                flags: 0,
            },
            Tag {
                block: 41,
                flags: TAG_ESCAPE,
            },
        ];
        put(&mut disk, &j, 1, &encode_descriptor(5, &[7; 16], &tags));
        put(&mut disk, &j, 2, &[0xA1; 1024]);
        put(&mut disk, &j, 3, &[0xB1; 1024]);
        put(&mut disk, &j, 4, &encode_commit(5, 1));
        put(&mut disk, &j, 5, &descriptor(6, &[42]));
        put(&mut disk, &j, 6, &[0xC1; 1024]);
        let found = scan(&disk, &j).unwrap();
        disk.begin_op("recover");
        let next = replay(&mut disk, &j, &found).unwrap();
        let rec = disk.end_op();
        assert_eq!(next, 7);
        assert_eq!(disk.read(40 * BS, BS), &[0xA1; 1024][..]);
        let mut escaped = vec![0xB1; 1024];
        escaped[..4].copy_from_slice(&[0xC0, 0x3B, 0x39, 0x98]);
        assert_eq!(disk.read(41 * BS, BS), &escaped[..]);
        assert!(disk.read(42 * BS, BS).iter().all(|&b| b == 0));
        assert_eq!(disk.read(20 * BS + 0x18, 8), &[0, 0, 0, 7, 0, 0, 0, 0][..]);
        assert_eq!(
            disk.read(SUPERBLOCK_OFFSET + INCOMPAT_OFFSET, 4),
            &[2, 0, 0, 0][..]
        );
        let texts: Vec<String> = rec.events.iter().map(|e| e.to_string()).collect();
        assert_eq!(
            texts,
            vec![
                "scanned the journal from block 1, sequence 5: 1 committed transaction, 2 tagged blocks",
                "replayed block 40 from journal block 2 (transaction 5)",
                "replayed block 41 from journal block 3 (transaction 5)",
                "discarded uncommitted transaction 6 (1 tagged block)",
                "journal emptied; next transaction 7",
                "cleared needs_recovery in the superblock",
            ]
        );
        let changes: Vec<(usize, usize)> = rec
            .changes
            .iter()
            .map(|c| (c.offset, c.after.len()))
            .collect();
        assert_eq!(
            changes,
            vec![
                (40 * BS, BS),
                (41 * BS, BS),
                (20 * BS + 0x18, 8),
                (SUPERBLOCK_OFFSET + INCOMPAT_OFFSET, 4),
            ]
        );
        assert_eq!(rec.events[3].region(), Some(25 * BS..26 * BS));
    }
```

- [ ] **Step 6: Share Task 4's crash scenarios.** `mod recovery` (next step) crashes the same six mutations as Task 4's `mod transactions` in `crates/ext/tests/ext3.rs`, so it imports them rather than copying them. In `mod transactions`, replace the constants at the top of the module:

```rust
    const BS: usize = 1024;
    const BOTH: [JournalMode; 2] = [JournalMode::Ordered, JournalMode::Data];
    /// `s_feature_incompat` of the primary superblock.
    const FLAG: usize = 1024 + 0x60;
    /// `s_sequence` and `s_start` of the journal superblock (block 82).
    const JSB_SEQ: usize = 82 * 1024 + 0x18;
    /// Journal index `i` is physical block `82 + i` on the default disk.
    const JOURNAL_BLOCK_0: u32 = 82;
    const MAGIC: [u8; 4] = [0xC0, 0x3B, 0x39, 0x98];
```

with:

```rust
    const BS: usize = 1024;
    pub(super) const BOTH: [JournalMode; 2] = [JournalMode::Ordered, JournalMode::Data];
    /// `s_feature_incompat` of the primary superblock.
    pub(super) const FLAG: usize = 1024 + 0x60;
    /// `s_sequence` and `s_start` of the journal superblock (block 82).
    pub(super) const JSB_SEQ: usize = 82 * 1024 + 0x18;
    /// Journal index `i` is physical block `82 + i` on the default disk.
    const JOURNAL_BLOCK_0: u32 = 82;
    pub(super) const MAGIC: [u8; 4] = [0xC0, 0x3B, 0x39, 0x98];
```

replace the two helpers above `fn count`:

```rust
    /// `(block * 1024, 1024)` for each block: whole-block writes.
    fn whole(blocks: impl IntoIterator<Item = u32>) -> Vec<(usize, usize)> {
        blocks.into_iter().map(|b| (b as usize * BS, BS)).collect()
    }

    fn texts(record: &OpRecord) -> Vec<String> {
        record.events.iter().map(|e| e.to_string()).collect()
    }
```

with:

```rust
    /// `(block * 1024, 1024)` for each block: whole-block writes.
    pub(super) fn whole(blocks: impl IntoIterator<Item = u32>) -> Vec<(usize, usize)> {
        blocks.into_iter().map(|b| (b as usize * BS, BS)).collect()
    }

    pub(super) fn texts(record: &OpRecord) -> Vec<String> {
        record.events.iter().map(|e| e.to_string()).collect()
    }
```

and replace the start of the crash scenarios (above `fn crash`; the body of `scenarios` stays as it is):

```rust
    const PHASES: [CrashPhase; 3] = [
        CrashPhase::BeforeCommit,
        CrashPhase::AfterCommit,
        CrashPhase::DuringCheckpoint,
    ];

    /// A mutation to crash, with the state it needs first.
    struct Scenario {
        setup: fn(&mut ExtFs),
        op: fn(&mut ExtFs) -> Result<OpRecord>,
    }

    /// `create_file`, `write_file` growing and shrinking, `delete_file`,
    /// `create_dir`, `remove_dir`.
    fn scenarios() -> Vec<Scenario> {
```

with:

```rust
    pub(super) const PHASES: [CrashPhase; 3] = [
        CrashPhase::BeforeCommit,
        CrashPhase::AfterCommit,
        CrashPhase::DuringCheckpoint,
    ];

    /// A mutation to crash, with the state it needs first.
    pub(super) struct Scenario {
        pub(super) setup: fn(&mut ExtFs),
        pub(super) op: fn(&mut ExtFs) -> Result<OpRecord>,
    }

    /// `create_file`, `write_file` growing and shrinking, `delete_file`,
    /// `create_dir`, `remove_dir`.
    pub(super) fn scenarios() -> Vec<Scenario> {
```

Run: `cargo test -p ext --test ext3 transactions::`
Expected: PASS, `test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 22 filtered out`. The library build before it still warns ``struct `LogWalk` is never constructed``, ``function `scan` is never used``, and six more dead-code warnings for the other items of Step 3 (nothing outside the unit tests calls them until Step 10). There is no commit in between.

- [ ] **Step 7: Write the failing `recover()` record tests.** Append to the end of `crates/ext/tests/ext3.rs`, after the last module (`mod transactions` at this point), one blank line after its closing `}`; the module order becomes `format`, `transactions`, `recovery` (Task 6 appends `inspect` after it). `crash` keeps the image before the crashed operation and the crashed tid; the scenarios, constants, and `texts`/`whole` come from `mod transactions`. The foreign-log test plants two committed transactions (the first wrapping from journal block 1021 to its commit at 1) and an uncommitted third with the Task 2 codecs:

```rust
/// Task 5: `recover()` (spec section 6), judged against the uncrashed and
/// the pre-operation images.
mod recovery {
    use super::common::{changes_in_order, ext3, pattern};
    use super::transactions::{scenarios, texts, whole, Scenario, FLAG, JSB_SEQ, MAGIC};
    use ext::{
        encode_commit, encode_descriptor, write_header, CrashPhase, ExtFormatOptions, ExtFs,
        JournalMode, Tag, BLOCKTYPE_REVOKE, TAG_ESCAPE,
    };
    use fs_core::{Error, OpRecord};

    const BS: usize = 1024;

    fn be32(image: &[u8], at: usize) -> u32 {
        u32::from_be_bytes([image[at], image[at + 1], image[at + 2], image[at + 3]])
    }

    /// A volume that crashed at a phase of a scenario's operation.
    struct Crashed {
        fs: ExtFs,
        /// The image after the setup, before the crashed operation.
        before: Vec<u8>,
        /// The crashed operation's record.
        rec: OpRecord,
        /// The crashed transaction's tid.
        tid: u32,
    }

    fn crash(mode: JournalMode, s: &Scenario, phase: CrashPhase) -> Crashed {
        let mut fs = ext3(mode);
        (s.setup)(&mut fs);
        let before = fs.disk().as_bytes().to_vec();
        let tid = fs.journal_info().unwrap().sequence;
        fs.arm_crash(phase).unwrap();
        let rec = (s.op)(&mut fs).unwrap();
        assert!(fs.needs_recovery());
        Crashed {
            fs,
            before,
            rec,
            tid,
        }
    }

    #[test]
    fn the_recover_record_replays_every_tag_in_order_then_empties_the_journal() {
        for (mode, homes) in [
            (JournalMode::Ordered, vec![1, 2, 3, 4, 5, 6, 69]),
            (
                JournalMode::Data,
                vec![1, 2, 3, 4, 5, 6, 69, 1111, 1112, 1113],
            ),
        ] {
            for phase in [CrashPhase::AfterCommit, CrashPhase::DuringCheckpoint] {
                let Crashed { mut fs, rec, .. } = crash(mode, &scenarios()[0], phase);
                assert_eq!(rec.op, format!("create_file /f (crashed {phase})"));
                let rec = fs.recover().unwrap();
                let what = format!("{mode:?} {phase:?}");
                // The descriptor is at journal block 1 (block 83); copy k is
                // at journal block k + 2.
                let n = homes.len();
                let mut want = whole(homes.iter().copied());
                want.push((JSB_SEQ, 8));
                want.push((FLAG, 4));
                assert_eq!(changes_in_order(&rec), want, "{what}");
                let mut events = vec![format!(
                    "scanned the journal from block 1, sequence 1: 1 committed transaction, \
                     {n} tagged blocks"
                )];
                for (k, home) in homes.iter().enumerate() {
                    events.push(format!(
                        "replayed block {home} from journal block {} (transaction 1)",
                        k + 2
                    ));
                }
                events.push("journal emptied; next transaction 3".into());
                events.push("cleared needs_recovery in the superblock".into());
                assert_eq!(texts(&rec), events, "{what}");
                assert_eq!(rec.events[0].region(), Some(JSB_SEQ..JSB_SEQ + 8));
                for (k, &home) in homes.iter().enumerate() {
                    let at = home as usize * BS;
                    assert_eq!(rec.events[k + 1].region(), Some(at..at + BS), "{what}");
                    let copy = fs.disk().read((84 + k) * BS, BS);
                    assert!(rec.changes[k].after == copy, "{what}: replay of {home}");
                }
                let c = &rec.changes;
                assert_eq!(c[n].before, [0, 0, 0, 1, 0, 0, 0, 1]);
                assert_eq!(c[n].after, [0, 0, 0, 3, 0, 0, 0, 0]);
                // The replayed superblock carries the flag; step 5 clears it.
                assert_eq!(c[n + 1].before, [6, 0, 0, 0]);
                assert_eq!(c[n + 1].after, [2, 0, 0, 0]);
                assert_eq!(rec.events[n + 1].region(), Some(JSB_SEQ..JSB_SEQ + 8));
                assert_eq!(rec.events[n + 2].region(), Some(FLAG..FLAG + 4));
                assert_eq!(fs.read_file("/f").unwrap(), pattern(3000));
            }
        }
    }

    #[test]
    fn a_transaction_without_its_commit_is_discarded_and_nothing_goes_home() {
        for (mode, n) in [(JournalMode::Ordered, 7), (JournalMode::Data, 10)] {
            let Crashed {
                mut fs,
                before,
                tid,
                ..
            } = crash(mode, &scenarios()[0], CrashPhase::BeforeCommit);
            let rec = fs.recover().unwrap();
            assert_eq!(changes_in_order(&rec), vec![(JSB_SEQ, 8), (FLAG, 4)]);
            assert_eq!(
                texts(&rec),
                vec![
                    format!(
                        "scanned the journal from block 1, sequence {tid}: 0 committed \
                         transactions, 0 tagged blocks"
                    ),
                    format!("discarded uncommitted transaction {tid} ({n} tagged blocks)"),
                    format!("journal emptied; next transaction {}", tid + 1),
                    "cleared needs_recovery in the superblock".into(),
                ]
            );
            // The discard points at the descriptor, journal block 1.
            assert_eq!(rec.events[1].region(), Some(83 * BS..84 * BS));
            assert_eq!(rec.changes[0].after, [0, 0, 0, 2, 0, 0, 0, 0]);
            assert_eq!(rec.changes[1].after, [2, 0, 0, 0]);
            // Outside the journal's blocks (82..=1105) the image is the one
            // before the operation, plus in ordered mode the file's data,
            // already home in blocks 1111..=1113 at step 2.
            let mut want = before;
            if mode == JournalMode::Ordered {
                want[1111 * BS..1111 * BS + 3000].copy_from_slice(&pattern(3000));
            }
            let image = fs.disk().as_bytes();
            assert!(image[..82 * BS] == want[..82 * BS], "{mode:?}");
            assert!(image[1106 * BS..] == want[1106 * BS..], "{mode:?}");
        }
    }

    #[test]
    fn recover_on_a_clean_volume_records_one_event_and_no_change() {
        let mut used = ext3(JournalMode::Data);
        used.create_file("/f", b"f").unwrap();
        let volumes = [
            ext3(JournalMode::Ordered),
            used,
            ExtFs::format(ExtFormatOptions::default()).unwrap(),
        ];
        for mut fs in volumes {
            let image = fs.disk().as_bytes().to_vec();
            let (history, info) = (fs.history().len(), fs.journal_info());
            let rec = fs.recover().unwrap();
            assert_eq!(rec.op, "recover");
            assert!(rec.changes.is_empty());
            assert_eq!(rec.event_kinds(), vec!["recovery_scanned"]);
            assert_eq!(texts(&rec), vec!["journal is clean; nothing to replay"]);
            assert_eq!(rec.events[0].region(), Some(0..0));
            assert_eq!(fs.history().len(), history + 1);
            assert_eq!(fs.history().last().unwrap().op, "recover");
            assert!(fs.disk().as_bytes() == image.as_slice());
            assert_eq!(fs.journal_info(), info);
        }
    }

    #[test]
    fn an_escaped_copy_goes_home_with_its_magic() {
        for phase in [CrashPhase::AfterCommit, CrashPhase::DuringCheckpoint] {
            let mut fs = ext3(JournalMode::Data);
            let mut data = pattern(1024);
            data[..4].copy_from_slice(&MAGIC);
            fs.arm_crash(phase).unwrap();
            fs.create_file("/magic", &data).unwrap();
            // Journal block 9 (block 91) holds the escaped copy; the home
            // block is still zero.
            assert_eq!(fs.disk().read(91 * BS, 4), [0, 0, 0, 0]);
            assert!(fs.disk().sector(1111).iter().all(|&b| b == 0));
            let rec = fs.recover().unwrap();
            let replayed = "replayed block 1111 from journal block 9 (transaction 1)";
            assert!(texts(&rec).iter().any(|t| t == replayed), "{phase:?}");
            assert_eq!(fs.disk().sector(1111), &data[..]);
            assert_eq!(fs.read_file("/magic").unwrap(), data);
        }
    }

    #[test]
    fn a_revoke_block_in_the_log_is_unsupported_and_changes_nothing() {
        let Crashed { mut fs, .. } = crash(
            JournalMode::Ordered,
            &scenarios()[0],
            CrashPhase::AfterCommit,
        );
        // The commit is at journal block 9 (block 91); a revoke block of
        // the next sequence follows it.
        let mut revoke = [0u8; 12];
        write_header(&mut revoke, BLOCKTYPE_REVOKE, 2);
        fs.write_raw(92 * BS as u64, &revoke).unwrap();
        let image = fs.disk().as_bytes().to_vec();
        let history = fs.history().len();
        assert_eq!(
            fs.recover().unwrap_err(),
            Error::Unsupported("journal revoke records".into())
        );
        assert!(fs.disk().as_bytes() == image.as_slice());
        assert_eq!(fs.history().len(), history);
        assert!(fs.needs_recovery());
    }

    #[test]
    fn a_tag_outside_the_volume_is_corrupt_and_changes_nothing() {
        let Crashed { mut fs, .. } = crash(
            JournalMode::Ordered,
            &scenarios()[0],
            CrashPhase::AfterCommit,
        );
        let tags = [Tag {
            block: 16_384,
            flags: 0,
        }];
        let descriptor = encode_descriptor(1, &fs.superblock().uuid, &tags);
        fs.write_raw(83 * BS as u64, &descriptor).unwrap();
        let image = fs.disk().as_bytes().to_vec();
        let history = fs.history().len();
        assert_eq!(
            fs.recover().unwrap_err(),
            Error::CorruptImage(
                "journal descriptor at journal block 1 tags block 16384, outside the \
                 16384-block volume"
                    .into()
            )
        );
        assert!(fs.disk().as_bytes() == image.as_slice());
        assert_eq!(fs.history().len(), history);
        assert!(fs.needs_recovery());
    }

    #[test]
    fn a_foreign_log_with_two_committed_transactions_replays_both_in_order() {
        let fs = ext3(JournalMode::Ordered);
        let uuid = fs.superblock().uuid;
        let mut image = fs.disk().as_bytes().to_vec();
        let mut put = |index: usize, block: &[u8]| {
            let at = (82 + index) * BS;
            image[at..at + BS].copy_from_slice(block);
        };
        let tag = |block, flags| Tag { block, flags };
        let mut escaped = [0xC3; 1024];
        escaped[..4].fill(0);
        // Transaction 7 wraps: descriptor at 1021, copies at 1022 and 1023,
        // commit at 1. Transaction 8: descriptor at 2, copies at 3 (block
        // 2001 again) and 4 (escaped), commit at 5. Transaction 9:
        // descriptor at 6, its copy at 7, never committed.
        put(
            1021,
            &encode_descriptor(7, &uuid, &[tag(2000, 0), tag(2001, 0)]),
        );
        put(1022, &[0xA1; 1024]);
        put(1023, &[0xB1; 1024]);
        put(1, &encode_commit(7, 1));
        put(
            2,
            &encode_descriptor(8, &uuid, &[tag(2001, 0), tag(2002, TAG_ESCAPE)]),
        );
        put(3, &[0xB2; 1024]);
        put(4, &escaped);
        put(5, &encode_commit(8, 1));
        put(6, &encode_descriptor(9, &uuid, &[tag(2003, 0)]));
        put(7, &[0xD1; 1024]);
        // s_sequence 7, s_start 1021, and needs_recovery.
        image[JSB_SEQ..JSB_SEQ + 8].copy_from_slice(&[0, 0, 0, 7, 0, 0, 0x03, 0xFD]);
        image[FLAG] = 0x06;
        let mut fs = ExtFs::from_image(image).unwrap();
        assert!(fs.needs_recovery());
        assert_eq!(fs.journal_info().unwrap().start, 1021);

        let rec = fs.recover().unwrap();
        assert_eq!(
            texts(&rec),
            vec![
                "scanned the journal from block 1021, sequence 7: 2 committed transactions, \
                 4 tagged blocks",
                "replayed block 2000 from journal block 1022 (transaction 7)",
                "replayed block 2001 from journal block 1023 (transaction 7)",
                "replayed block 2001 from journal block 3 (transaction 8)",
                "replayed block 2002 from journal block 4 (transaction 8)",
                "discarded uncommitted transaction 9 (1 tagged block)",
                "journal emptied; next transaction 10",
                "cleared needs_recovery in the superblock",
            ]
        );
        assert_eq!(rec.events[5].region(), Some(88 * BS..89 * BS));
        assert_eq!(fs.disk().sector(2000), &[0xA1; 1024][..]);
        assert_eq!(fs.disk().sector(2001), &[0xB2; 1024][..]);
        let mut home = [0xC3; 1024];
        home[..4].copy_from_slice(&MAGIC);
        assert_eq!(fs.disk().sector(2002), &home[..]);
        assert!(fs.disk().sector(2003).iter().all(|&b| b == 0));
        assert_eq!(be32(fs.disk().as_bytes(), JSB_SEQ), 10);
        let info = fs.journal_info().unwrap();
        assert_eq!(
            (info.sequence, info.head, info.start, info.needs_recovery),
            (10, 1, 0, false)
        );
    }

    #[test]
    fn a_flag_set_over_an_empty_journal_is_cleared_and_the_sequence_moves_on() {
        let mut fs = ext3(JournalMode::Ordered);
        fs.write_raw(FLAG as u64, &[6, 0, 0, 0]).unwrap();
        assert!(fs.needs_recovery());
        let rec = fs.recover().unwrap();
        assert_eq!(changes_in_order(&rec), vec![(JSB_SEQ, 8), (FLAG, 4)]);
        assert_eq!(
            texts(&rec),
            vec![
                "journal is clean; nothing to replay",
                "journal emptied; next transaction 2",
                "cleared needs_recovery in the superblock",
            ]
        );
        assert_eq!(rec.events[0].region(), Some(JSB_SEQ..JSB_SEQ + 8));
        assert!(!fs.needs_recovery());
        assert_eq!(fs.journal_info().unwrap().sequence, 2);
    }

    #[test]
    fn recover_never_crashes_and_an_armed_phase_waits_for_the_next_mutation() {
        let Crashed { mut fs, .. } = crash(
            JournalMode::Ordered,
            &scenarios()[4],
            CrashPhase::AfterCommit,
        );
        fs.arm_crash(CrashPhase::DuringCheckpoint).unwrap();
        let rec = fs.recover().unwrap();
        assert_eq!(rec.op, "recover");
        assert!(!rec.event_kinds().contains(&"crashed"));
        assert!(!fs.needs_recovery());
        assert_eq!(fs.crash_phase(), Some(CrashPhase::DuringCheckpoint));
        let rec = fs.create_dir("/e").unwrap();
        assert_eq!(rec.op, "create_dir /e (crashed during checkpoint)");
        assert!(fs.needs_recovery());
    }

    #[test]
    fn recover_fails_with_the_gate_error_while_the_volume_is_corrupt() {
        let mut fs = ext3(JournalMode::Ordered);
        // Zero the ext magic (0x38 of the primary superblock).
        fs.write_raw(1024 + 0x38, &[0, 0]).unwrap();
        let err = fs.corruption().unwrap().clone();
        let history = fs.history().len();
        assert_eq!(fs.recover().unwrap_err(), err);
        assert_eq!(fs.history().len(), history);
    }
}
```

- [ ] **Step 8: Run both to see them fail.**

Run: `cargo test -p ext --lib journal::recovery`
Expected: FAIL to compile: ``error[E0425]: cannot find function `replay` in this scope``, ``error[E0425]: cannot find value `SUPERBLOCK_OFFSET` in this scope``, ``error[E0425]: cannot find value `INCOMPAT_OFFSET` in this scope``; ``could not compile `ext` (lib test) due to 7 previous errors``.

Run: `cargo test -p ext --test ext3 recovery::`
Expected: FAIL to compile: ``error[E0599]: no method named `recover` found for struct `ExtFs` in the current scope``; ``could not compile `ext` (test "ext3") due to 10 previous errors``. The library build before it still carries the eight dead-code warnings of Step 6.

- [ ] **Step 9: Implement `replay`.** In `crates/ext/src/journal/recovery.rs`, replace the imports and constants from Step 3:

```rust
use super::state::JournalState;
use super::{
    decode_descriptor, read_header, Tag, BLOCKTYPE_COMMIT, BLOCKTYPE_DESCRIPTOR, BLOCKTYPE_REVOKE,
};
use crate::superblock::BLOCK_SIZE;
use fs_core::{Disk, Error, Result};

const BS: usize = BLOCK_SIZE as usize;
```

with:

```rust
use super::state::JournalState;
use super::{
    decode_descriptor, read_header, unescape, JournalSuperblock, Tag, BLOCKTYPE_COMMIT,
    BLOCKTYPE_DESCRIPTOR, BLOCKTYPE_REVOKE, FEATURE_INCOMPAT_RECOVER, TAG_ESCAPE,
};
use crate::events::ExtEvent;
use crate::superblock::{BLOCK_SIZE, SUPERBLOCK_OFFSET};
use fs_core::{Disk, Error, Result};

const BS: usize = BLOCK_SIZE as usize;
/// `s_feature_incompat` in the ext superblock.
const INCOMPAT_OFFSET: usize = 0x60;
```

and insert directly above `#[cfg(test)]`, followed by one blank line:

```rust
/// Spec section 6 steps 2 (the event) to 5, under the caller's open
/// operation, for what `scan` found: `RecoveryScanned` over `s_sequence`
/// and `s_start`; every committed transaction in order, each tag in order,
/// its copy written home as a whole block with the magic restored where
/// `ESCAPE` is set (`Replayed`); `TransactionDiscarded` over the
/// descriptor of an uncommitted one; the journal superblock emptied with
/// `s_sequence = expected + 1` (`JournalEmptied`); `needs_recovery`
/// cleared in the primary superblock (`RecoveryFlagCleared`). Returns the
/// new `s_sequence`.
pub(crate) fn replay(
    disk: &mut Disk,
    journal: &JournalState,
    found: &[ScannedTransaction],
) -> Result<u32> {
    let jsb = journal.superblock(disk)?;
    let at = journal.offset(0) + JournalSuperblock::SEQUENCE_OFFSET;
    let committed = found.iter().filter(|t| t.committed).count() as u32;
    let tagged = found
        .iter()
        .filter(|t| t.committed)
        .map(|t| t.tags.len() as u32)
        .sum();
    disk.event(Box::new(ExtEvent::RecoveryScanned {
        start: jsb.start,
        sequence: jsb.sequence,
        committed,
        tagged,
        range: at..at + 8,
    }));
    for t in found {
        if !t.committed {
            let from = journal.offset(t.descriptor_index);
            disk.event(Box::new(ExtEvent::TransactionDiscarded {
                tid: t.tid,
                tagged: t.tags.len() as u32,
                range: from..from + BS,
            }));
            continue;
        }
        for &(index, tag) in &t.tags {
            let mut copy = disk.read(journal.offset(index), BS).to_vec();
            if tag.flags & TAG_ESCAPE != 0 {
                unescape(&mut copy);
            }
            let home = tag.block as usize * BS;
            disk.write(home, &copy);
            disk.event(Box::new(ExtEvent::Replayed {
                tid: t.tid,
                home: tag.block,
                from_index: index,
                range: home..home + BS,
            }));
        }
    }
    // Every commit moved `expected` on by one; jbd2 then takes one more
    // (`++info.end_transaction`), so no stale commit can match again.
    let next_sequence = jsb.sequence.wrapping_add(committed).wrapping_add(1);
    let mut bytes = [0u8; 8];
    bytes[..4].copy_from_slice(&next_sequence.to_be_bytes());
    disk.write(at, &bytes);
    disk.event(Box::new(ExtEvent::JournalEmptied {
        next_sequence,
        range: at..at + 8,
    }));
    let flag = SUPERBLOCK_OFFSET + INCOMPAT_OFFSET;
    let raw = disk.read(flag, 4);
    let flags = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) & !FEATURE_INCOMPAT_RECOVER;
    disk.write(flag, &flags.to_le_bytes());
    disk.event(Box::new(ExtEvent::RecoveryFlagCleared {
        range: flag..flag + 4,
    }));
    Ok(next_sequence)
}
```

- [ ] **Step 10: Add `ExtFs::recover`.** In `crates/ext/src/fs.rs`, replace:

```rust
use crate::journal::state::{self, JournalInfo, JournalState};
```

with:

```rust
use crate::journal::recovery;
use crate::journal::state::{self, JournalInfo, JournalState};
```

Then `write_raw` ends its `impl ExtFs` block; replace its tail and the block's closing `}`:

```rust
            // Nothing fallible may follow: `run_op` rolls the bytes back on
            // an error, but not `sb`/`gds`/`geo`/`corrupt`.
            Ok(())
        })
    }
}
```

with:

```rust
            // Nothing fallible may follow: `run_op` rolls the bytes back on
            // an error, but not `sb`/`gds`/`geo`/`corrupt`.
            Ok(())
        })
    }

    /// Replay or discard what the journal holds, as a mount does (spec
    /// section 6), as the recorded operation `recover`. A volume that does
    /// not need recovery (ext2 included) records the operation with no
    /// changes and the one event `journal is clean; nothing to replay`.
    /// Otherwise the scan runs first, so its `Unsupported` (a revoke block)
    /// or `CorruptImage` (a bad tag or log) returns with nothing written;
    /// then every committed copy goes home, the journal is emptied, and
    /// `needs_recovery` is cleared. Afterwards `sb` and `gds` are re-read
    /// and validated as the corruption gate does (a failure marks the
    /// volume corrupt), the next transaction takes `s_sequence` at the
    /// first log block, and an armed crash still waits for the next
    /// mutation: `recover` itself never crashes. Fails with the gate error
    /// while the volume is corrupt.
    pub fn recover(&mut self) -> Result<OpRecord> {
        self.ensure_mounted()?;
        let journal = match &self.journal {
            Some(j) if j.needs_recovery => j.clone(),
            other => {
                let sequence = other.as_ref().map_or(0, |j| j.sequence);
                return self.run_op("recover".into(), |fs| {
                    fs.disk.event(Box::new(ExtEvent::RecoveryScanned {
                        start: 0,
                        sequence,
                        committed: 0,
                        tagged: 0,
                        range: 0..0,
                    }));
                    Ok(())
                });
            }
        };
        let found = recovery::scan(&self.disk, &journal)?;
        let mut next_sequence = journal.sequence;
        let record = self.run_op("recover".into(), |fs| {
            next_sequence = recovery::replay(&mut fs.disk, &journal, &found)?;
            Ok(())
        })?;
        self.reparse_metadata();
        if let Some(j) = self.journal.as_mut() {
            j.sequence = next_sequence;
            j.head = j.first;
            j.needs_recovery = false;
        }
        Ok(record)
    }
}
```

- [ ] **Step 11: Run the replay and record tests.**

Run: `cargo test -p ext --lib journal::recovery`
Expected: PASS, `test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 85 filtered out`.

Run: `cargo test -p ext --test ext3 recovery::`
Expected: PASS, `test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out`, no warnings.

#### Round 3: the image-equality tests

- [ ] **Step 12: Add the image helpers and the image-equality tests.** In `crates/ext/tests/ext3.rs`, `mod recovery`, replace the imports from Step 7:

```rust
    use super::common::{changes_in_order, ext3, pattern};
    use super::transactions::{scenarios, texts, whole, Scenario, FLAG, JSB_SEQ, MAGIC};
    use ext::{
        encode_commit, encode_descriptor, write_header, CrashPhase, ExtFormatOptions, ExtFs,
        JournalMode, Tag, BLOCKTYPE_REVOKE, TAG_ESCAPE,
    };
    use fs_core::{Error, OpRecord};
```

with:

```rust
    use super::common::{changes_in_order, ext3, mask_journal, pattern};
    use super::transactions::{
        scenarios, texts, whole, Scenario, BOTH, FLAG, JSB_SEQ, MAGIC, PHASES,
    };
    use ext::{
        encode_commit, encode_descriptor, write_header, CrashPhase, ExtFormatOptions, ExtFs,
        JournalMode, Tag, BLOCKTYPE_REVOKE, TAG_ESCAPE,
    };
    use fs_core::{ByteChange, Error, OpRecord};
    use std::collections::BTreeSet;
```

and add at the end of `mod recovery`, after `recover_fails_with_the_gate_error_while_the_volume_is_corrupt` (one blank line between) and before the module's closing `}`. For every scenario, phase, and mode, the recovered image (under `mask_journal`) must be the uncrashed image after a commit and the pre-operation image plus the data the crashed record wrote home before its journal superblock write (ordered mode; nothing in data mode) before it; `s_sequence` must be tid + 2 after a replay and tid + 1 after a discard; and a reload of the crashed image must recover to the same bytes:

```rust
    /// `image` with `fs`'s journal masked away (`mask_journal`).
    fn masked(image: &[u8], fs: &ExtFs) -> Vec<u8> {
        let mut image = image.to_vec();
        mask_journal(&mut image, fs);
        image
    }

    /// Whether `block`'s bit is set in the block bitmap `image` holds.
    fn allocated(fs: &ExtFs, image: &[u8], block: u32) -> bool {
        let geo = fs.geometry();
        let group = geo.group_of_block(block) as usize;
        let bitmap = fs.group_descriptors()[group].block_bitmap as usize * BS;
        let bit = (block - geo.groups_layout[group].first_block) as usize;
        image[bitmap + bit / 8] & (1 << (bit % 8)) != 0
    }

    /// The image of `s` run to completion in `mode`.
    fn uncrashed(mode: JournalMode, s: &Scenario) -> Vec<u8> {
        let mut fs = ext3(mode);
        (s.setup)(&mut fs);
        (s.op)(&mut fs).unwrap();
        fs.disk().as_bytes().to_vec()
    }

    /// What a crashed record wrote between the flag (its first change) and
    /// the journal superblock: ordered mode's data home (spec 5.2 step 2);
    /// nothing in data mode.
    fn data_home(rec: &OpRecord) -> &[ByteChange] {
        let step3 = rec
            .changes
            .iter()
            .position(|c| c.offset == JSB_SEQ)
            .unwrap();
        &rec.changes[1..step3]
    }

    #[test]
    fn recover_leaves_the_uncrashed_or_the_pre_operation_image_in_every_case() {
        for mode in BOTH {
            for s in scenarios() {
                let whole = uncrashed(mode, &s);
                for phase in PHASES {
                    let Crashed {
                        mut fs,
                        before,
                        rec,
                        tid,
                    } = crash(mode, &s, phase);
                    let what = format!("{mode:?} {phase:?} {}", rec.op);
                    let history = fs.history().len();
                    let recovered = fs.recover().unwrap();
                    assert_eq!(recovered.op, "recover", "{what}");
                    assert_eq!(fs.history().len(), history + 1, "{what}");
                    let image = fs.disk().as_bytes().to_vec();
                    // After a commit the transaction is replayed: the image of
                    // the whole operation. Before it, the transaction is
                    // discarded: the image before the operation, plus in
                    // ordered mode the data already written home.
                    let want = match phase {
                        CrashPhase::BeforeCommit => {
                            let mut want = before;
                            let data = data_home(&rec);
                            if mode == JournalMode::Data {
                                assert!(data.is_empty(), "{what}");
                            }
                            for c in data {
                                want[c.offset..c.offset + c.after.len()].copy_from_slice(&c.after);
                            }
                            want
                        }
                        _ => whole.clone(),
                    };
                    assert!(
                        masked(&image, &fs) == masked(&want, &fs),
                        "{what}: differs from the expected image"
                    );
                    // The journal is empty and `s_sequence` follows section 6:
                    // tid + 2 after a replay, tid + 1 after a discard.
                    let next = match phase {
                        CrashPhase::BeforeCommit => tid + 1,
                        _ => tid + 2,
                    };
                    assert_eq!(
                        (be32(&image, JSB_SEQ), be32(&image, JSB_SEQ + 4)),
                        (next, 0),
                        "{what}"
                    );
                    assert_eq!(image[FLAG..FLAG + 4], [2, 0, 0, 0], "{what}");
                    assert_eq!(fs.superblock().feature_incompat, 0x0002, "{what}");
                    let info = fs.journal_info().unwrap();
                    assert_eq!(
                        (info.sequence, info.head, info.start, info.needs_recovery),
                        (next, 1, 0, false),
                        "{what}"
                    );
                    assert!(!fs.needs_recovery(), "{what}");
                    // Mutations run again, from the first log block.
                    let rec = fs.create_dir("/after").unwrap();
                    let started = format!("started transaction {next} at journal block 1 (");
                    assert!(
                        texts(&rec).iter().any(|t| t.starts_with(&started)),
                        "{what}: {:?}",
                        texts(&rec)
                    );
                }
            }
        }
    }

    #[test]
    fn ordered_before_commit_keeps_the_data_home_and_the_bitmap_as_it_was() {
        // Per scenario, how many of the blocks written home were free
        // before the operation: the fresh allocations the discarded
        // transaction never recorded.
        let fresh = [3, 19, 0, 0, 0, 0];
        for (s, fresh) in scenarios().iter().zip(fresh) {
            let Crashed {
                mut fs,
                before,
                rec,
                ..
            } = crash(JournalMode::Ordered, s, CrashPhase::BeforeCommit);
            fs.recover().unwrap();
            let image = fs.disk().as_bytes().to_vec();
            let mut free = BTreeSet::new();
            for c in data_home(&rec) {
                assert_eq!(fs.disk().read(c.offset, c.after.len()), &c.after[..]);
                let block = (c.offset / BS) as u32;
                let was = allocated(&fs, &before, block);
                assert_eq!(allocated(&fs, &image, block), was, "block {block}");
                if !was {
                    free.insert(block);
                }
            }
            assert_eq!(free.len(), fresh, "{}", rec.op);
        }
        // The created file is in no directory; its data is orphaned in
        // blocks the bitmap calls free.
        let Crashed { mut fs, .. } = crash(
            JournalMode::Ordered,
            &scenarios()[0],
            CrashPhase::BeforeCommit,
        );
        fs.recover().unwrap();
        assert_eq!(fs.read_file("/f").unwrap_err(), Error::NotFound);
        assert_eq!(fs.superblock().free_blocks_count, 15_205);
        assert_eq!(fs.disk().read(1111 * BS, 3000), &pattern(3000)[..]);
    }

    #[test]
    fn a_reloaded_crashed_image_recovers_to_the_same_bytes_as_the_volume_in_place() {
        for mode in BOTH {
            for s in scenarios() {
                for phase in PHASES {
                    let Crashed { mut fs, .. } = crash(mode, &s, phase);
                    let what = format!("{mode:?} {phase:?}");
                    let mut loaded = ExtFs::from_image(fs.disk().as_bytes().to_vec()).unwrap();
                    assert!(loaded.needs_recovery(), "{what}");
                    assert!(loaded.history().is_empty());
                    assert_eq!(
                        loaded.journal_info().unwrap().start,
                        fs.journal_info().unwrap().start,
                        "{what}"
                    );
                    let here = fs.recover().unwrap();
                    let there = loaded.recover().unwrap();
                    assert!(fs.disk().as_bytes() == loaded.disk().as_bytes(), "{what}");
                    assert_eq!(here.changes, there.changes, "{what}");
                    assert_eq!(texts(&here), texts(&there), "{what}");
                    assert_eq!(fs.journal_info(), loaded.journal_info(), "{what}");
                    assert_eq!(fs.superblock(), loaded.superblock(), "{what}");
                    assert_eq!(fs.group_descriptors(), loaded.group_descriptors());
                }
            }
        }
    }
```

- [ ] **Step 13: Run the recovery tests.**

Run: `cargo test -p ext --test ext3 recovery::`
Expected: PASS, `test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out`.

#### Round 4: e2fsck as the replay oracle

- [ ] **Step 14: Share the logdump helpers.** In `crates/ext/tests/e2fsprogs.rs`, `mod ext3_transactions`, replace:

```rust
    fn logdump_lines(rec: &OpRecord, tid: u32, with_commit: bool) -> Vec<String> {
```

with:

```rust
    pub(super) fn logdump_lines(rec: &OpRecord, tid: u32, with_commit: bool) -> Vec<String> {
```

and replace:

```rust
    fn logdump(fs: &ExtFs, name: &str, request: &str, tid: u32) -> Option<Vec<String>> {
```

with:

```rust
    pub(super) fn logdump(fs: &ExtFs, name: &str, request: &str, tid: u32) -> Option<Vec<String>> {
```

- [ ] **Step 15: Add the image comparison to the shared test helpers.** `e2fsprogs.rs` (here) and `mount_linux.rs` (Step 19) both compare a repaired image with `recover()`'s. In `crates/ext/tests/common/mod.rs`, replace:

```rust
use ext::{BlockRole, ExtFormatOptions, ExtFs, JournalMode, JournalOptions};
```

with:

```rust
use ext::{BlockRole, ExtFormatOptions, ExtFs, Geometry, JournalMode, JournalOptions};
```

and append to the end of the file, one blank line after the closing `}` of `changes_in_order`:

```rust
/// The superblock fields e2fsck or a kernel mount may rewrite on its own
/// account, as `(offset, length, name)`: the mount and check bookkeeping,
/// the lifetime write counter, and the reserved tail (`s_reserved`, up to
/// `s_checksum`).
pub const SUPERBLOCK_IGNORED: [(usize, usize, &str); 7] = [
    (0x2C, 4, "s_mtime"),
    (0x30, 4, "s_wtime"),
    (0x34, 2, "s_mnt_count"),
    (0x3A, 2, "s_state"),
    (0x40, 4, "s_lastcheck"),
    (0x178, 8, "s_kbytes_written"),
    (0x284, 0x178, "s_reserved"),
];

/// `ours` and `theirs` hold the same bytes in every block, except that in
/// the superblock copies the fields of `SUPERBLOCK_IGNORED` may differ.
/// The panic names the first differing block and offset; `whose` names
/// where `theirs` came from (`"e2fsck's"`, `"the kernel's"`).
pub fn assert_images_agree(ours: &[u8], theirs: &[u8], geo: &Geometry, whose: &str) {
    assert_eq!(ours.len(), theirs.len(), "the images differ in length");
    let superblocks: Vec<usize> = geo
        .groups_layout
        .iter()
        .filter_map(|g| g.superblock_block)
        .map(|b| b as usize)
        .collect();
    for (block, (a, b)) in ours.chunks(1024).zip(theirs.chunks(1024)).enumerate() {
        let is_superblock = superblocks.contains(&block);
        let ignored = |at: usize| {
            is_superblock
                && SUPERBLOCK_IGNORED
                    .iter()
                    .any(|&(offset, len, _)| (offset..offset + len).contains(&at))
        };
        if let Some(at) = (0..a.len()).find(|&at| a[at] != b[at] && !ignored(at)) {
            panic!(
                "block {block} differs at offset 0x{at:03X}: ours 0x{:02X}, {whose} 0x{:02X}",
                a[at], b[at]
            );
        }
    }
}
```

- [ ] **Step 16: Remove Task 4's crashed-create oracle.** The next step's `e2fsck_replays_a_crashed_create_to_the_bytes_recover_writes` runs the same scenario (`create_dir /d`, then a 3000-byte `create_file /d/f`, per mode and phase), makes the same `logdump -a` assertion, and runs the same `e2fsck -fy` (exit 0 or 1, `recovering journal`) and `e2fsck -fn` (exit 0, no `Fix?`) checks, then also compares the bytes; Task 4's test is subsumed. In `crates/ext/tests/e2fsprogs.rs`, `mod ext3_transactions`, delete this test (it is the module's last item) together with the blank line above it, so the module's closing `}` follows `logdump_reads_our_last_transaction_tag_for_tag`:

```rust
    #[test]
    fn a_crashed_create_logs_our_tags_and_e2fsck_recovers_it_clean() {
        let (Some(e2fsck), Some(_)) = (tool("e2fsck"), tool("debugfs")) else {
            return;
        };
        for mode in BOTH {
            for phase in [
                CrashPhase::BeforeCommit,
                CrashPhase::AfterCommit,
                CrashPhase::DuringCheckpoint,
            ] {
                let mut fs = ext3(mode);
                fs.create_dir("/d").unwrap();
                let tid = fs.journal_info().unwrap().sequence;
                fs.arm_crash(phase).unwrap();
                let rec = fs.create_file("/d/f", &pattern(3000)).unwrap();
                let name = format!("ext3-crash-{}-{}", mode.as_str(), phase.as_str());
                // Plain logdump starts at s_start: our transaction.
                let with_commit = phase != CrashPhase::BeforeCommit;
                let got = logdump(&fs, &name, "logdump -a", tid).unwrap();
                assert_eq!(got, logdump_lines(&rec, tid, with_commit), "{name}");

                let image = image_file(&fs, &name);
                let fix = run(&e2fsck, &["-fy"], &image);
                let check = run(&e2fsck, &["-fn"], &image);
                std::fs::remove_file(&image).unwrap();
                let text = both_streams(&fix);
                assert!(
                    matches!(fix.status.code(), Some(0 | 1)),
                    "e2fsck -fy {name}:\n{text}"
                );
                assert!(text.contains("recovering journal"), "{name}:\n{text}");
                let text = both_streams(&check);
                assert_eq!(check.status.code(), Some(0), "e2fsck -fn {name}:\n{text}");
                assert!(!text.contains("Fix?"), "{name}:\n{text}");
            }
        }
    }
```

and, since nothing else in the module names `CrashPhase`, replace its import:

```rust
    use ext::{decode_descriptor, CrashPhase, ExtFs, JournalMode};
```

with:

```rust
    use ext::{decode_descriptor, ExtFs, JournalMode};
```

Run: `cargo test -p ext --test e2fsprogs ext3_transactions`
Expected: PASS, `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out` (the scripted sequence and `logdump_reads_our_last_transaction_tag_for_tag`; with e2fsprogs installed), no warnings.

- [ ] **Step 17: Add the replay oracle tests.** Append to the end of `crates/ext/tests/e2fsprogs.rs`, one blank line after the closing `}` of `mod ext3_transactions`:

```rust
/// ext3 Task 5: `e2fsck -fy` as the replay oracle (ext3 spec section 10.3,
/// third and fourth bullets): on a copy of a crashed image it must write
/// exactly the bytes `recover()` writes.
mod ext3_recovery {
    use super::common::{assert_images_agree, ext3, SUPERBLOCK_IGNORED};
    use super::ext3_transactions::{logdump, logdump_lines};
    use super::{both_streams, image_file, pattern, run, tool};
    use ext::{encode_commit, encode_descriptor, CrashPhase, ExtFs, JournalMode, Tag, TAG_ESCAPE};

    const BOTH: [JournalMode; 2] = [JournalMode::Ordered, JournalMode::Data];
    const PHASES: [CrashPhase; 3] = [
        CrashPhase::BeforeCommit,
        CrashPhase::AfterCommit,
        CrashPhase::DuringCheckpoint,
    ];
    const MAGIC: [u8; 4] = [0xC0, 0x3B, 0x39, 0x98];
    /// Run `e2fsck -fy` on a copy of `fs`'s image and `e2fsck -fn` after
    /// it, check both, and return the repaired bytes. `None` without
    /// e2fsck.
    fn e2fsck_recovered(fs: &ExtFs, name: &str) -> Option<Vec<u8>> {
        let e2fsck = tool("e2fsck")?;
        let image = image_file(fs, name);
        let fix = run(&e2fsck, &["-fy"], &image);
        let check = run(&e2fsck, &["-fn"], &image);
        let bytes = std::fs::read(&image).unwrap();
        std::fs::remove_file(&image).unwrap();
        let text = both_streams(&fix);
        assert!(
            matches!(fix.status.code(), Some(0 | 1)),
            "e2fsck -fy {name}:\n{text}"
        );
        assert!(text.contains("recovering journal"), "{name}:\n{text}");
        let text = both_streams(&check);
        assert_eq!(check.status.code(), Some(0), "e2fsck -fn {name}:\n{text}");
        assert!(!text.contains("Fix?"), "{name}:\n{text}");
        Some(bytes)
    }

    #[test]
    fn assert_images_agree_ignores_only_the_listed_superblock_fields() {
        let fs = ext3(JournalMode::Ordered);
        let ours = fs.disk().as_bytes().to_vec();
        let mut theirs = ours.clone();
        for (offset, len, _) in SUPERBLOCK_IGNORED {
            for sb in [1024, 8193 * 1024] {
                theirs[sb + offset..sb + offset + len].fill(0xEE);
            }
        }
        let geo = fs.geometry();
        assert_images_agree(&ours, &theirs, geo, "e2fsck's");
        for at in [1024 + 0x0C, 8193 * 1024 + 0x60, 82 * 1024 + 0x18, 69 * 1024] {
            let mut theirs = ours.clone();
            theirs[at] ^= 1;
            let message = std::panic::catch_unwind(|| {
                assert_images_agree(&ours, &theirs, geo, "e2fsck's");
            })
            .unwrap_err();
            let message = message.downcast_ref::<String>().unwrap();
            let want = format!("block {} differs at offset 0x{:03X}", at / 1024, at % 1024);
            assert!(message.starts_with(&want), "{message}");
        }
    }

    #[test]
    fn e2fsck_replays_a_crashed_create_to_the_bytes_recover_writes() {
        if tool("e2fsck").is_none() || tool("debugfs").is_none() {
            return;
        }
        for mode in BOTH {
            for phase in PHASES {
                let mut fs = ext3(mode);
                fs.create_dir("/d").unwrap();
                let tid = fs.journal_info().unwrap().sequence;
                fs.arm_crash(phase).unwrap();
                let rec = fs.create_file("/d/f", &pattern(3000)).unwrap();
                let name = format!("ext3-replay-{}-{}", mode.as_str(), phase.as_str());
                // logdump reads our descriptor, tags, and (once written) commit.
                let with_commit = phase != CrashPhase::BeforeCommit;
                let got = logdump(&fs, &name, "logdump -a", tid).unwrap();
                assert_eq!(got, logdump_lines(&rec, tid, with_commit), "{name}");

                let theirs = e2fsck_recovered(&fs, &name).unwrap();
                fs.recover().unwrap();
                assert_images_agree(fs.disk().as_bytes(), &theirs, fs.geometry(), "e2fsck's");
            }
        }
    }

    #[test]
    fn e2fsck_replays_an_escaped_copy_to_the_bytes_recover_writes() {
        if tool("e2fsck").is_none() {
            return;
        }
        for phase in [CrashPhase::AfterCommit, CrashPhase::DuringCheckpoint] {
            let mut fs = ext3(JournalMode::Data);
            let mut data = pattern(3000);
            data[..4].copy_from_slice(&MAGIC);
            fs.arm_crash(phase).unwrap();
            let rec = fs.create_file("/magic", &data).unwrap();
            assert!(
                rec.events
                    .iter()
                    .any(|e| e.to_string().ends_with("(block 91) (escaped)")),
                "the first data block's copy is escaped"
            );
            let name = format!("ext3-replay-escaped-{}", phase.as_str());
            let theirs = e2fsck_recovered(&fs, &name).unwrap();
            fs.recover().unwrap();
            assert_images_agree(fs.disk().as_bytes(), &theirs, fs.geometry(), "e2fsck's");
            assert_eq!(theirs[1111 * 1024..1111 * 1024 + 4], MAGIC);
            assert_eq!(fs.read_file("/magic").unwrap(), data);
        }
    }

    #[test]
    fn e2fsck_replays_a_foreign_log_of_two_transactions_to_the_bytes_recover_writes() {
        if tool("e2fsck").is_none() {
            return;
        }
        let fs = ext3(JournalMode::Ordered);
        let uuid = fs.superblock().uuid;
        let mut image = fs.disk().as_bytes().to_vec();
        let mut put = |index: usize, block: &[u8]| {
            let at = (82 + index) * 1024;
            image[at..at + 1024].copy_from_slice(block);
        };
        let tag = |block, flags| Tag { block, flags };
        let mut escaped = [0xC3; 1024];
        escaped[..4].fill(0);
        // Transaction 7 wraps from journal block 1021 to its commit at 1;
        // transaction 8 rewrites block 2001 and escapes 2002; transaction 9
        // never commits.
        put(
            1021,
            &encode_descriptor(7, &uuid, &[tag(2000, 0), tag(2001, 0)]),
        );
        put(1022, &[0xA1; 1024]);
        put(1023, &[0xB1; 1024]);
        put(1, &encode_commit(7, 1));
        put(
            2,
            &encode_descriptor(8, &uuid, &[tag(2001, 0), tag(2002, TAG_ESCAPE)]),
        );
        put(3, &[0xB2; 1024]);
        put(4, &escaped);
        put(5, &encode_commit(8, 1));
        put(6, &encode_descriptor(9, &uuid, &[tag(2003, 0)]));
        put(7, &[0xD1; 1024]);
        let jsb = 82 * 1024 + 0x18;
        image[jsb..jsb + 8].copy_from_slice(&[0, 0, 0, 7, 0, 0, 0x03, 0xFD]);
        image[1024 + 0x60] = 0x06;
        let mut fs = ExtFs::from_image(image).unwrap();
        let theirs = e2fsck_recovered(&fs, "ext3-replay-foreign").unwrap();
        fs.recover().unwrap();
        assert_images_agree(fs.disk().as_bytes(), &theirs, fs.geometry(), "e2fsck's");
        assert_eq!(theirs[jsb..jsb + 8], [0, 0, 0, 10, 0, 0, 0, 0]);
        assert_eq!(theirs[2002 * 1024..2002 * 1024 + 4], MAGIC);
    }
}
```

- [ ] **Step 18: Run the oracle tests, locally and as CI runs them.**

Run: `cargo test -p ext --test e2fsprogs ext3_recovery`
Expected: PASS, `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 17 filtered out` (with e2fsprogs installed; without it each test prints `e2fsprogs not found; skipping` and passes).

Run: `CI=1 cargo test -p ext --test e2fsprogs ext3_recovery`
Expected: PASS, `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 17 filtered out`.

#### Round 5: the ignored ext3 mount test

- [ ] **Step 19: Add the Linux mount case.** Spec 10.3's last bullet asks for `recover()`'s image "with the same masking", which is 10.2's: `mask_journal` on both images, then the superblock exclusions of `assert_images_agree`. The e2fsck oracle's stricter byte-equal journal superblock does not carry over, because the kernel's unmount rewrites journal superblock fields its own way. In `crates/ext/tests/mount_linux.rs`, replace:

```rust
//! Manual validation: loop-mount the exported image read-only with Linux
//! and read files back. Run with
//! `cargo test -p ext --test mount_linux -- --ignored` as root or with
//! passwordless sudo.
#![cfg(target_os = "linux")]

use ext::{ExtFormatOptions, ExtFs};
```

with:

```rust
//! Manual validation: loop-mount the exported image read-only with Linux
//! and read files back, and let the kernel replay a crashed ext3 image.
//! Run with `cargo test -p ext --test mount_linux -- --ignored` as root or
//! with passwordless sudo.
#![cfg(target_os = "linux")]

mod common;

use common::{assert_images_agree, mask_journal};
use ext::{CrashPhase, ExtFormatOptions, ExtFs};
```

and append to the end of the file, one blank line after the closing `}` of `linux_mounts_the_image_and_reads_files`:

```rust
/// ext3 spec section 10.3, last bullet: a read-write mount replays the
/// journal of a crashed after-commit image; after the unmount the image
/// must equal what `recover()` writes under the spec's masking: both images
/// pass through `mask_journal` (the journal's blocks and the journal fields
/// of the superblocks), and the superblock copies may differ in the fields
/// of `SUPERBLOCK_IGNORED`. Unlike the e2fsck oracle, the journal
/// superblock is not compared byte for byte: the kernel's unmount rewrites
/// its fields in its own way.
#[test]
#[ignore]
fn linux_replays_a_crashed_ext3_image_like_recover() {
    let mut fs = ExtFs::format(ExtFormatOptions::ext3()).unwrap();
    fs.create_dir("/docs").unwrap();
    fs.arm_crash(CrashPhase::AfterCommit).unwrap();
    fs.create_file("/docs/hello.txt", b"hello from the emulator\n")
        .unwrap();
    assert!(fs.needs_recovery());

    let dir = std::env::temp_dir().join(format!("ext3-emulator-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let image = dir.join("crashed.img");
    let mount = dir.join("mnt");
    std::fs::create_dir_all(&mount).unwrap();
    std::fs::write(&image, fs.disk().as_bytes()).unwrap();

    let attach = as_root("mount")
        .args(["-t", "ext3", "-o", "loop,rw"])
        .arg(&image)
        .arg(&mount)
        .output()
        .unwrap();
    assert!(
        attach.status.success(),
        "mount failed: {}",
        String::from_utf8_lossy(&attach.stderr)
    );
    let detach = as_root("umount").arg(&mount).output().unwrap();
    assert!(
        detach.status.success(),
        "umount failed: {}",
        String::from_utf8_lossy(&detach.stderr)
    );
    let mut theirs = std::fs::read(&image).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    fs.recover().unwrap();
    let mut ours = fs.disk().as_bytes().to_vec();
    mask_journal(&mut ours, &fs);
    mask_journal(&mut theirs, &fs);
    assert_images_agree(&ours, &theirs, fs.geometry(), "the kernel's");
    assert_eq!(
        fs.read_file("/docs/hello.txt").unwrap(),
        b"hello from the emulator\n"
    );
}
```

- [ ] **Step 20: Build the mount tests.**

Run: `cargo test -p ext --test mount_linux`
Expected on macOS (observed; the file is `#![cfg(target_os = "linux")]`, so nothing in it compiles): `running 0 tests`, `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.
Expected on Linux (inferred, not observed: no Linux host was available, so this is what libtest prints for the file's two `#[ignore]` tests): `test linux_mounts_the_image_and_reads_files ... ignored`, `test linux_replays_a_crashed_ext3_image_like_recover ... ignored`, `test result: ok. 0 passed; 0 failed; 2 ignored`. What was observed instead: a copy of the file without the `cfg` line, built as its own test binary on macOS, lists both tests as ignored and passes `cargo clippy -p ext --tests -- -D warnings`. The test itself needs a Linux host with root or passwordless sudo: `cargo test -p ext --test mount_linux -- --ignored`.

#### Gates and commit

- [ ] **Step 21: Run every gate.**

Run: `cargo fmt --all -- --check`
Expected: no output, exit 0.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: `Finished`, no warnings.

Run: `cargo test --workspace`
Expected: every binary passes: ext unit 97, `e2fsprogs` 21, `ext2` 52, `ext3` 54, `mount_linux` 0 (2 ignored on Linux), fat unit 82, `fat16` 16, `mount_macos` 1 ignored, fs-core 34, wasm unit 20, and the empty wasm and doc-test binaries.

Run: `CI=1 cargo test -p ext --test e2fsprogs`
Expected: `test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

Run: `cargo build -p fs-emulator-wasm --target wasm32-unknown-unknown`
Expected: `Finished`.

- [ ] **Step 22: Commit.**

```bash
git add crates/ext/src/journal/recovery.rs crates/ext/src/journal/mod.rs crates/ext/src/fs.rs crates/ext/tests/ext3.rs crates/ext/tests/common/mod.rs crates/ext/tests/e2fsprogs.rs crates/ext/tests/mount_linux.rs
git commit -m "feat(ext): journal recovery that matches e2fsck's replay"
```

---

### Task 6: Journal inspection

**Spec:** section 7 (the `journal_blocks()` and `annotate_sector` bullets, including the last sentences of the first: the classification never fails, a revoke block of the expected sequence is a live `Revoke`, and the live parse shares `recover`'s log walker; `journal_info`, `layout`, and `block_owners` are Task 3's), section 6 (the scan rules of `recover()` step 2, which the live parse reuses through Task 5's walker: start at `s_start` with `expected = s_sequence`, stop at a block without the magic, with another sequence, or of another type; a descriptor's tags claim the following indexes by the ring rule; a commit advances `expected`), section 1 (`JournalBlock`, `JournalBlockKind` are public), 10.2 (the ring bullet's "`journal_blocks()` reports the stale transactions"), and the slice-2 spec's section 7 (annotation shape: a byte range within the block, a label, a value; pointer blocks as `pointer[i..j]` runs).

`ExtFs::journal_blocks()` lists every journal index in order with its physical block, what it holds (`Superblock`, `Descriptor`, `Copy { home, escaped }`, `Commit`, `Revoke`, `Unused`), the transaction it belongs to, and whether it is `stale`. The classifier (`journal::inspect::classify`, crate-private) reads the raw journal: index 0 is the superblock; when the on-disk `s_start` lies in `first..maxlen` it first parses the live transaction(s) from `s_start` by consuming Task 5's log walker (`journal::recovery::LogWalk`, the one `recover`'s scan uses), with a revoke block of the expected sequence shown as a live `Revoke` where the scan refuses it (`stale: false`); then it walks indexes `first..maxlen` in order, skipping what is already classified, and parses each block with Task 5's `parse_block`: every log block there is a stale leftover: a descriptor (claiming the following indexes by the ring rule as its copies until one is already classified), a commit, or a revoke block; a header of any other type (a superblock inside the log) and every block without a header that nothing claims is `Unused` with `tid: None`, `stale: false`. `annotate_sector` of a journal data block (role `BlockRole::Journal`) now explains it by that classification: the journal superblock field by field, a descriptor's header and tags, a commit's header and commit time, a copy as one annotation naming its home and transaction, an unused block as one annotation; the journal's pointer blocks (owned by `<journal>` with role `Indirect`) keep slice 2's pointer-run annotations. Metadata blocks still win first (`annotate_sector` checks the group metadata before it consults `block_owners`), so nothing about ext2 or the other blocks changes.

This task runs after Task 5 and builds on it. Everything below was compiled and run on a scratch branch holding Tasks 1 to 5 (after their critique fixes) plus this task: fmt, workspace clippy, `cargo test --workspace` including the e2fsprogs oracle (also with `CI=1`, nothing skipped), and the wasm32 build. The steps were then replayed after Task 5's by a script that applied exactly the code blocks below in order and ran each command; every "Expected" is the observed output, every intermediate state passed `cargo fmt --all -- --check`, and the replayed tree was byte-identical to the branch. Copy the code verbatim.

Decisions later tasks must know:
- `crates/ext/src/journal/mod.rs`: rustfmt keeps consecutive `mod` lines sorted, so `pub mod inspect;` is the FIRST `pub mod` line (above Task 5's `pub mod recovery;`), and the four read `inspect`, `recovery`, `state`, `txn`. The module doc gains one line for `inspect`.
- `crate::fs::note` and `crate::fs::uuid` are now `pub(crate)` (inspect.rs builds its annotations with them).
- `journal_blocks()` reads `s_start` and `s_sequence` from the journal superblock on the disk, never from `JournalState`, so it describes a crashed volume, a volume loaded from a crashed image, and a corrupt volume (it uses the last good block map) alike. If index 0 does not decode or `s_start` is outside `first..maxlen`, nothing is live (the walker walks nothing).
- There is one log walker (spec section 7): the live parse consumes `LogWalk` and differs from `recover`'s scan only on revoke blocks, which it marks as a live `Revoke` and goes on; it never fails. It ends where the walk ends, at a block already classified, or at a descriptor whose tags do not parse (still a `Descriptor`, claiming no copies) or one of whose copies is already classified. The stale walk is a different rule (every index in order, any sequence) but parses with the same `parse_block`.
- Classification facts pinned by the tests on the default disk (journal index `i` = block `82 + i`): after one ordered 3-block `create_file`, index 1 is `Descriptor` tid 1, 2..=8 are copies of homes 1, 2, 3, 4, 5, 6, 69, 9 is `Commit`, all `stale: true`; a crashed second transaction starts at `journal_info().start == 10` with `tid == journal_info().sequence == 2`, `stale: false` (no commit block for `BeforeCommit`). After 127 empty-file creates the head is 1018 and a `create_dir` straddles the end (1018..=1023 then 1..=4); transaction 1's leftover copies at 5..=7 are `Unused` and its commit at 8 stays a stale `Commit`.
- `JournalBlockKind` → Task 7's DTO `kind` strings: `superblock`, `descriptor`, `copy` (with `home`, `escaped`), `commit`, `revoke`, `unused`; `tid` is `None` exactly for `Superblock` and `Unused`.
- Annotation strings (label → value), all pinned by the tests below:
  - journal superblock (index 0): `magic` `0xC03B3998` (0..4), `block type` `4` (4..8), `block size` `1024` (12..16), `maxlen` `1024` (16..20), `first` `1` (20..24), `sequence` (24..28), `start` (28..32), `uuid` in the 8-4-4-4-12 form, e.g. `e2f5ee00-2026-4923-8000-000000000001` (48..64), `users` = `s_nr_users` (64..68), all decimal except magic and uuid; exactly these nine, in this order.
  - descriptor: `magic` (0..4), `block type` `1` (4..8), `sequence` (8..12), then `tag k` (k from 1) over its 8 bytes with value `block N, flags F`, where F is the set flags by name in bit order joined with ` | ` (`escape`, `same_uuid`, `deleted`, `last`; an unnamed bit as `0x0010`) or `none`; a `uuid` annotation (16 bytes) follows every tag without `same_uuid` (the first tag, on our images). E.g. `tag 1` = `block 1, flags none`, `tag 7` = `block 69, flags same_uuid | last`, an escaped data copy's tag `block 1111, flags escape | same_uuid | last`.
  - commit: `magic`, `block type` `2`, `sequence`, then `commit time` (48..56) = `{secs} ({date})`, e.g. `315532800 (1980-01-01 00:00:00)` (the ext superblock's time format).
  - revoke: the three header annotations only (`block type` `5`).
  - copy: one annotation over 0..1024, label `journal copy`, value `copy of block {home} for transaction {tid}` plus ` (stale)` when stale, then ` (escaped)` when escaped.
  - unused (including a superblock-type header inside the log): one annotation over 0..1024, label `journal`, value `unused journal block`.
  - journal pointer blocks: slice 2's `pointer[i..j]` runs, e.g. block 1106 = `pointer[0..255]` `blocks 94..349`.
- `annotate_sector` of a journal block classifies the whole journal per call (1024 header reads); the doc comment of `annotate_journal_block` says so, and a bulk caller in slice 4 should call `journal_blocks()` once instead of relying on per-sector speed.
- `crates/ext/tests/ext3.rs`: `mod inspect` imports `BOTH`, `JOURNAL_BLOCK_0`, and `MAGIC` from `mod transactions` (this task makes `JOURNAL_BLOCK_0` `pub(super)`; Task 5 did the other two).

**Files:**
- Create: `crates/ext/src/journal/inspect.rs` (567 lines): module doc, imports, `BS` (lines 1-16), `JournalBlockKind` and `JournalBlock` (lines 18-46), `Slot`, `Walk` (`set`, `taken`, `mark`, `live`, `stale`), `classify` (lines 48-152), `SUPERBLOCK_FIELDS`, `TAG_FLAG_NAMES`, `annotate`, `header_notes`, `tag_notes`, `flag_names` (lines 154-274), unit tests (lines 276-567).
- Modify: `crates/ext/src/journal/mod.rs`: the `inspect` line of the module doc (line 9), `pub mod inspect;` (line 11).
- Modify: `crates/ext/src/lib.rs`: `pub use journal::inspect::{JournalBlock, JournalBlockKind};` (line 21).
- Modify: `crates/ext/src/fs.rs`: the `inspect` import (line 15), `journal_blocks` (lines 381-390), `note` made `pub(crate)` (lines 962-967), `uuid` made `pub(crate)` (line 990), the `annotate_owned_block` doc and its `Journal` arm (lines 1871-1896), `annotate_journal_block` (lines 1898-1908).
- Test: `crates/ext/tests/ext3.rs`: `JOURNAL_BLOCK_0` made `pub(super)` in `mod transactions` (line 630); `mod inspect` appended after `mod recovery` (lines 2030-2421): helpers (lines 2032-2075), the `journal_blocks` tests (lines 2077-2238), the annotation helpers and tests (lines 2240-2420).

**Interfaces:**

Consumes (from Tasks 1 to 5, on the base branch):
- `crate::journal::{BlockHeader, Tag { block, flags }, COMMIT_SEC_OFFSET, HEADER_LEN, TAG_BYTES, TAG_ESCAPE, TAG_SAME_UUID, TAG_DELETED, TAG_LAST}` and the private `be32_at(b: &[u8], i: usize) -> u32` of `journal/mod.rs` (reachable from the child module); in unit tests `encode_descriptor`, `encode_commit`, `write_header`, `JournalSuperblock::{new, encode, SEQUENCE_OFFSET}`, `JournalMode`, `BLOCKTYPE_DESCRIPTOR`, `BLOCKTYPE_REVOKE`, `BLOCKTYPE_SUPERBLOCK_V2` (Task 2).
- `crate::journal::recovery::{LogWalk, LogEntry { index, sequence, block }, LogBlock::{Descriptor(Result<Vec<(u32, Tag)>>), Commit, Revoke}, parse_block}` (Task 5): `LogWalk::new(disk, journal, start, sequence)` iterates the log from `start`, walking nothing when `start` is outside `first..maxlen`; `parse_block(disk, journal, index) -> Option<LogEntry>`.
- `crate::journal::state::JournalState { blocks, maxlen, first, sequence, head, mode, needs_recovery, armed }` with `offset`, `wrap`, `superblock(&Disk) -> Result<JournalSuperblock>` (Task 3); `ExtFs`'s `pub(crate)` fields `disk`, `journal`; `ExtFs::{block_owners, journal_info, annotate_sector, annotate_owned_block, block_bytes}`; `BlockRole::{Data, Directory, Indirect, Journal}`; the slice-2 helpers `note`, `uuid`, `annotate_pointer_block` in `fs.rs`.
- `ExtFs::{arm_crash, create_file, create_dir, write_raw, set_now}` and `CrashPhase` (Task 4); the test helpers `common::{ext3, pattern}` (Task 3); `mod transactions`'s `BOTH` and `MAGIC` (made `pub(super)` by Task 5) and `JOURNAL_BLOCK_0` in `tests/ext3.rs`.

Produces (exact):

```rust
// crates/ext/src/journal/inspect.rs  (pub mod inspect; re-exported as ext::JournalBlock, ext::JournalBlockKind)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalBlockKind { Superblock, Descriptor, Copy { home: u32, escaped: bool }, Commit, Revoke, Unused }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalBlock { pub index: u32, pub block: u32, pub kind: JournalBlockKind, pub tid: Option<u32>, pub stale: bool }
pub(crate) fn classify(disk: &Disk, journal: &JournalState) -> Vec<JournalBlock>;   // one entry per journal index, len == maxlen
pub(crate) fn annotate(bytes: &[u8], entry: &JournalBlock) -> Vec<Annotation>;      // the section-7 annotations of one journal block

// crates/ext/src/fs.rs
impl ExtFs {
    pub fn journal_blocks(&self) -> Vec<JournalBlock>;   // empty on ext2
}
pub(crate) fn note(range: Range<usize>, label: impl Into<String>, value: impl Into<String>) -> Annotation;   // was private
pub(crate) fn uuid(bytes: &[u8]) -> String;                                                            // was private
// annotate_owned_block: BlockRole::Journal => the journal annotations (was None); Indirect unchanged
```

Gate commands used throughout (run from the repository root):

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p fs-emulator-wasm --target wasm32-unknown-unknown
```

---

#### Round 1: the classifier

- [ ] **Step 1: Write the failing classifier tests.** Create `crates/ext/src/journal/inspect.rs` with the imports the classifier will use, the two public types, and the unit tests (the classifier comes in Step 2):

```rust
//! The journal as the explorer sees it (spec section 7): what every journal
//! block holds, whether it belongs to the live transaction or is a stale
//! leftover of an earlier one, and the annotations `annotate_sector` shows
//! for a journal data block.

use super::recovery::{parse_block, LogBlock, LogEntry, LogWalk};
use super::state::JournalState;
use super::TAG_ESCAPE;
use crate::superblock::BLOCK_SIZE;
use fs_core::Disk;

const BS: usize = BLOCK_SIZE as usize;

/// What one journal block holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalBlockKind {
    /// Journal index 0.
    Superblock,
    Descriptor,
    /// The logged copy of home block `home`; `escaped` when its tag sets
    /// `ESCAPE` (the copy's first four bytes were the magic and are zero).
    Copy {
        home: u32,
        escaped: bool,
    },
    Commit,
    Revoke,
    /// No journal header and no descriptor claims it.
    Unused,
}

/// One journal index: its physical block, what it holds, the transaction it
/// belongs to, and whether that transaction is over (`stale`: a leftover
/// the journal no longer needs) or is the live one `s_start` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalBlock {
    pub index: u32,
    pub block: u32,
    pub kind: JournalBlockKind,
    pub tid: Option<u32>,
    pub stale: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::{
        encode_commit, encode_descriptor, write_header, JournalMode, JournalSuperblock, Tag,
        BLOCKTYPE_REVOKE, BLOCKTYPE_SUPERBLOCK_V2,
    };
    use JournalBlockKind::{Commit, Descriptor, Revoke, Superblock, Unused};

    /// A 16-block journal whose index `i` is physical block `10 + i`, on a
    /// 64-block disk, with a fresh journal superblock.
    fn journal() -> (Disk, JournalState) {
        let mut disk = Disk::new(BS, 64);
        let j = JournalState {
            blocks: (10..26).collect(),
            maxlen: 16,
            first: 1,
            sequence: 1,
            head: 1,
            mode: JournalMode::Ordered,
            needs_recovery: false,
            armed: None,
        };
        let mut sb = [0u8; BS];
        JournalSuperblock::new(16, [7; 16]).encode(&mut sb);
        disk.write(j.offset(0), &sb);
        (disk, j)
    }

    /// Point `s_start` and `s_sequence` at a live transaction (0: empty).
    fn set_start(disk: &mut Disk, j: &JournalState, start: u32, sequence: u32) {
        let at = j.offset(0) + JournalSuperblock::SEQUENCE_OFFSET;
        disk.write(at, &sequence.to_be_bytes());
        disk.write(at + 4, &start.to_be_bytes());
    }

    /// Log transaction `tid` from `start`: a descriptor, one copy per home
    /// (the copy of home `h` is the byte `h` repeated), and a commit when
    /// `commit`. Returns the index after the last block written.
    fn log(
        disk: &mut Disk,
        j: &JournalState,
        start: u32,
        tid: u32,
        homes: &[u32],
        commit: bool,
    ) -> u32 {
        let tags: Vec<Tag> = homes.iter().map(|&block| Tag { block, flags: 0 }).collect();
        disk.write(j.offset(start), &encode_descriptor(tid, &[7; 16], &tags));
        let mut at = start;
        for &home in homes {
            at = j.wrap(at + 1);
            disk.write(j.offset(at), &[home as u8; BS]);
        }
        at = j.wrap(at + 1);
        if commit {
            disk.write(j.offset(at), &encode_commit(tid, 1_000));
            at = j.wrap(at + 1);
        }
        at
    }

    fn copy(home: u32) -> JournalBlockKind {
        JournalBlockKind::Copy {
            home,
            escaped: false,
        }
    }

    /// `(kind, tid, stale)` per index.
    fn summary(disk: &Disk, j: &JournalState) -> Vec<(JournalBlockKind, Option<u32>, bool)> {
        classify(disk, j)
            .into_iter()
            .map(|b| (b.kind, b.tid, b.stale))
            .collect()
    }

    fn unused() -> (JournalBlockKind, Option<u32>, bool) {
        (Unused, None, false)
    }

    #[test]
    fn a_fresh_journal_is_its_superblock_and_unused_blocks() {
        let (disk, j) = journal();
        let blocks = classify(&disk, &j);
        assert_eq!(blocks.len(), 16);
        assert_eq!(
            blocks[0],
            JournalBlock {
                index: 0,
                block: 10,
                kind: Superblock,
                tid: None,
                stale: false,
            }
        );
        for (i, b) in blocks.iter().enumerate().skip(1) {
            assert_eq!((b.index, b.block), (i as u32, 10 + i as u32));
            assert_eq!((b.kind, b.tid, b.stale), unused());
        }
    }

    #[test]
    fn a_finished_transaction_is_stale_and_the_live_one_is_not() {
        let (mut disk, j) = journal();
        log(&mut disk, &j, 1, 3, &[5, 6, 7], true);
        let mut want = vec![(Superblock, None, false)];
        want.push((Descriptor, Some(3), true));
        want.extend([5, 6, 7].map(|h| (copy(h), Some(3), true)));
        want.push((Commit, Some(3), true));
        want.extend(vec![unused(); 10]);
        assert_eq!(summary(&disk, &j), want);
        // The same blocks named by `s_start` are the live transaction.
        set_start(&mut disk, &j, 1, 3);
        let live: Vec<_> = want
            .iter()
            .map(|&(kind, tid, _)| (kind, tid, false))
            .collect();
        assert_eq!(summary(&disk, &j), live);
    }

    #[test]
    fn the_live_parse_follows_the_sequence_and_stops_at_a_mismatch() {
        let (mut disk, j) = journal();
        // Transaction 3 committed, 4 logged but not committed, then a
        // leftover descriptor of transaction 1.
        let next = log(&mut disk, &j, 1, 3, &[5], true);
        let next = log(&mut disk, &j, next, 4, &[6, 7], false);
        assert_eq!(next, 7);
        log(&mut disk, &j, next, 1, &[8], true);
        set_start(&mut disk, &j, 1, 3);
        let s = summary(&disk, &j);
        assert_eq!(
            s[1..4],
            [
                (Descriptor, Some(3), false),
                (copy(5), Some(3), false),
                (Commit, Some(3), false)
            ]
        );
        assert_eq!(
            s[4..7],
            [
                (Descriptor, Some(4), false),
                (copy(6), Some(4), false),
                (copy(7), Some(4), false)
            ]
        );
        assert_eq!(
            s[7..10],
            [
                (Descriptor, Some(1), true),
                (copy(8), Some(1), true),
                (Commit, Some(1), true)
            ]
        );
        assert!(s[10..].iter().all(|&e| e == unused()));
    }

    #[test]
    fn a_descriptor_near_the_end_claims_its_copies_across_the_wrap() {
        for live in [false, true] {
            let (mut disk, j) = journal();
            // Descriptor at 14, copies at 15, 1, 2, commit at 3.
            assert_eq!(log(&mut disk, &j, 14, 9, &[5, 6, 7], true), 4);
            if live {
                set_start(&mut disk, &j, 14, 9);
            }
            let stale = !live;
            let s = summary(&disk, &j);
            assert_eq!(s[14], (Descriptor, Some(9), stale), "live {live}");
            assert_eq!(s[15], (copy(5), Some(9), stale));
            assert_eq!(s[1], (copy(6), Some(9), stale));
            assert_eq!(s[2], (copy(7), Some(9), stale));
            assert_eq!(s[3], (Commit, Some(9), stale));
            assert!(s[4..14].iter().all(|&e| e == unused()));
        }
    }

    #[test]
    fn a_stale_descriptor_stops_claiming_at_the_live_transaction() {
        let (mut disk, j) = journal();
        // A leftover descriptor at 2 names four copies (3..=6) and its
        // commit is at 7, but the live transaction 5 has since been logged
        // at 4..=6.
        log(&mut disk, &j, 2, 4, &[20, 21, 22, 23], true);
        log(&mut disk, &j, 4, 5, &[30], true);
        set_start(&mut disk, &j, 4, 5);
        let s = summary(&disk, &j);
        assert_eq!(s[1], unused());
        assert_eq!(s[2], (Descriptor, Some(4), true));
        assert_eq!(s[3], (copy(20), Some(4), true));
        assert_eq!(s[4], (Descriptor, Some(5), false));
        assert_eq!(s[5], (copy(30), Some(5), false));
        assert_eq!(s[6], (Commit, Some(5), false));
        assert_eq!(s[7], (Commit, Some(4), true));
        assert!(s[8..].iter().all(|&e| e == unused()));
    }

    #[test]
    fn escape_flags_revoke_blocks_and_foreign_headers_are_reported() {
        let (mut disk, j) = journal();
        let tags = [Tag {
            block: 40,
            flags: TAG_ESCAPE,
        }];
        disk.write(j.offset(1), &encode_descriptor(2, &[7; 16], &tags));
        let mut revoke = [0u8; BS];
        write_header(&mut revoke, BLOCKTYPE_REVOKE, 2);
        disk.write(j.offset(3), &revoke);
        let mut foreign = [0u8; BS];
        write_header(&mut foreign, BLOCKTYPE_SUPERBLOCK_V2, 0);
        disk.write(j.offset(4), &foreign);
        let s = summary(&disk, &j);
        let escaped = JournalBlockKind::Copy {
            home: 40,
            escaped: true,
        };
        assert_eq!(s[2], (escaped, Some(2), true));
        assert_eq!(s[3], (Revoke, Some(2), true));
        assert_eq!(s[4], unused());
        // Live, a revoke block is part of the transaction.
        set_start(&mut disk, &j, 1, 2);
        let s = summary(&disk, &j);
        assert_eq!(
            s[1..4],
            [
                (Descriptor, Some(2), false),
                (escaped, Some(2), false),
                (Revoke, Some(2), false)
            ]
        );
    }

    #[test]
    fn a_broken_journal_superblock_leaves_only_stale_blocks() {
        let (mut disk, j) = journal();
        log(&mut disk, &j, 1, 3, &[5], true);
        set_start(&mut disk, &j, 1, 3);
        disk.write(j.offset(0), &[0; 4]);
        let s = summary(&disk, &j);
        assert_eq!(s[0], (Superblock, None, false));
        assert_eq!(s[1], (Descriptor, Some(3), true));
        // An `s_start` outside the log is ignored too.
        let (mut disk, j) = journal();
        log(&mut disk, &j, 1, 3, &[5], true);
        set_start(&mut disk, &j, 16, 3);
        assert_eq!(summary(&disk, &j)[1], (Descriptor, Some(3), true));
    }
}
```

In `crates/ext/src/journal/mod.rs`, after Task 5 the module doc ends with the `recovery` line and the module lines read `pub mod recovery;`, `pub mod state;`, `pub mod txn;`. Replace:

```rust
//! `recovery` scans and replays the log as a mount does (section 6).

pub mod recovery;
```

with (rustfmt keeps these lines sorted, so `inspect` goes first):

```rust
//! `recovery` scans and replays the log as a mount does (section 6);
//! `inspect` classifies every journal block and annotates it (section 7).

pub mod inspect;
pub mod recovery;
```

Run: `cargo test -p ext --lib inspect`
Expected: the build fails. The error and warning lines (the first error is at the `summary` helper, the second in `a_fresh_journal_is_its_superblock_and_unused_blocks`):

```
error[E0425]: cannot find function `classify` in this scope
error[E0425]: cannot find function `classify` in this scope
warning: unused imports: `LogBlock`, `LogEntry`, `LogWalk`, and `parse_block`
error: could not compile `ext` (lib test) due to 2 previous errors; 1 warning emitted
```

- [ ] **Step 2: Implement the classifier.** In `crates/ext/src/journal/inspect.rs`, insert this between the `JournalBlock` struct and `#[cfg(test)]` (one blank line on each side). The live parse consumes Task 5's `LogWalk`, so it reads the log exactly as `recover`'s scan does; the only difference is that `mark` shows a revoke block where the scan refuses it:

```rust
/// A journal index's classification while the walk runs.
type Slot = Option<(JournalBlockKind, Option<u32>, bool)>;

/// The classification in progress: one slot per journal index.
struct Walk<'a> {
    disk: &'a Disk,
    journal: &'a JournalState,
    slots: Vec<Slot>,
}

impl Walk<'_> {
    fn set(&mut self, index: u32, kind: JournalBlockKind, tid: Option<u32>, stale: bool) {
        self.slots[index as usize] = Some((kind, tid, stale));
    }

    fn taken(&self, index: u32) -> bool {
        self.slots[index as usize].is_some()
    }

    /// Mark a log block: a descriptor also claims the copies its tags name,
    /// as far as they are not taken yet. Returns `false` when a
    /// descriptor's tags do not parse or one of its copies was taken.
    fn mark(&mut self, entry: LogEntry, stale: bool) -> bool {
        let (index, tid) = (entry.index, Some(entry.sequence));
        match entry.block {
            LogBlock::Descriptor(tags) => {
                self.set(index, JournalBlockKind::Descriptor, tid, stale);
                let Ok(tags) = tags else {
                    return false;
                };
                for (at, tag) in tags {
                    if self.taken(at) {
                        return false;
                    }
                    let kind = JournalBlockKind::Copy {
                        home: tag.block,
                        escaped: tag.flags & TAG_ESCAPE != 0,
                    };
                    self.set(at, kind, tid, stale);
                }
            }
            LogBlock::Commit => self.set(index, JournalBlockKind::Commit, tid, stale),
            LogBlock::Revoke => self.set(index, JournalBlockKind::Revoke, tid, stale),
        }
        true
    }

    /// The live transactions from `start`: recovery's log walk (spec
    /// section 6), except that a revoke block is part of the live
    /// transaction instead of an error. The parse ends where the walk ends,
    /// at a block already classified, or at a descriptor `mark` refuses.
    fn live(&mut self, start: u32, sequence: u32) {
        for entry in LogWalk::new(self.disk, self.journal, start, sequence) {
            if self.taken(entry.index) || !self.mark(entry, false) {
                return;
            }
        }
    }

    /// Every other log block, in index order: stale descriptors (claiming
    /// their copies), commits, and revoke blocks. A header of any other
    /// type (a superblock inside the log) stays unused.
    fn stale(&mut self) {
        for index in self.journal.first..self.journal.maxlen {
            if self.taken(index) {
                continue;
            }
            if let Some(entry) = parse_block(self.disk, self.journal, index) {
                self.mark(entry, true);
            }
        }
    }
}

/// Classify every journal index in order (spec section 7): index 0 is the
/// superblock; the live transaction from `s_start`, if any, comes first
/// (`stale: false`); then every other log block is a stale leftover, a
/// stale descriptor claiming the following blocks that are not live or
/// claimed yet as its copies; the rest is unused.
pub(crate) fn classify(disk: &Disk, journal: &JournalState) -> Vec<JournalBlock> {
    let mut walk = Walk {
        disk,
        journal,
        slots: vec![None; journal.maxlen as usize],
    };
    walk.set(0, JournalBlockKind::Superblock, None, false);
    if let Ok(jsb) = journal.superblock(disk) {
        walk.live(jsb.start, jsb.sequence);
    }
    walk.stale();
    walk.slots
        .into_iter()
        .enumerate()
        .map(|(index, slot)| {
            let (kind, tid, stale) = slot.unwrap_or((JournalBlockKind::Unused, None, false));
            JournalBlock {
                index: index as u32,
                block: journal.blocks[index],
                kind,
                tid,
                stale,
            }
        })
        .collect()
}
```

Run: `cargo test -p ext --lib inspect`
Expected: `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out`. (A plain `cargo build -p ext` still warns that `BS`, `Slot`, `Walk`, its methods, and `classify` are unused: nothing outside the tests calls them until Round 2. There is no commit in between.)

#### Round 2: `journal_blocks`

- [ ] **Step 3: Write the failing `journal_blocks` tests.** In `crates/ext/tests/ext3.rs`, `mod transactions` holds the journal's first block as a private constant; share it like Task 5 shared `BOTH` and `MAGIC`. Replace:

```rust
    const JOURNAL_BLOCK_0: u32 = 82;
```

with:

```rust
    pub(super) const JOURNAL_BLOCK_0: u32 = 82;
```

Then append this module to the end of the file, after the last module (`mod recovery`, Task 5's), one blank line after its closing `}`; the module order becomes `format`, `transactions`, `recovery`, `inspect`:

```rust
/// Task 6: what every journal block holds (spec section 7).
mod inspect {
    use super::common::{ext3, pattern};
    use super::transactions::{BOTH, JOURNAL_BLOCK_0, MAGIC};
    use ext::JournalBlockKind::{Commit, Descriptor, Superblock, Unused};
    use ext::{CrashPhase, ExtFormatOptions, ExtFs, JournalBlock, JournalBlockKind, JournalMode};
    use std::collections::BTreeSet;

    /// What an ordered 3-block `create_file` on a fresh volume tags: the
    /// superblock, descriptors, both bitmaps, two inode table blocks, and
    /// the root directory's block.
    const CREATE_TAGS: [u32; 7] = [1, 2, 3, 4, 5, 6, 69];

    type Entry = (JournalBlockKind, Option<u32>, bool);

    fn copy(home: u32) -> JournalBlockKind {
        JournalBlockKind::Copy {
            home,
            escaped: false,
        }
    }

    /// `(kind, tid, stale)` per journal index.
    fn entries(fs: &ExtFs) -> Vec<Entry> {
        fs.journal_blocks()
            .into_iter()
            .map(|b| (b.kind, b.tid, b.stale))
            .collect()
    }

    const UNUSED: Entry = (Unused, None, false);

    /// Transaction `tid`'s blocks: its descriptor, one copy per home, and
    /// its commit when `commit`.
    fn transaction(tid: u32, homes: &[u32], commit: bool, stale: bool) -> Vec<Entry> {
        let mut out = vec![(Descriptor, Some(tid), stale)];
        out.extend(homes.iter().map(|&h| (copy(h), Some(tid), stale)));
        if commit {
            out.push((Commit, Some(tid), stale));
        }
        out
    }

    #[test]
    fn a_fresh_journal_is_its_superblock_then_unused_blocks() {
        for mode in BOTH {
            let fs = ext3(mode);
            let blocks = fs.journal_blocks();
            assert_eq!(blocks.len(), 1024);
            assert_eq!(
                blocks[0],
                JournalBlock {
                    index: 0,
                    block: 82,
                    kind: Superblock,
                    tid: None,
                    stale: false,
                }
            );
            for (i, b) in blocks.iter().enumerate().skip(1) {
                let want = JournalBlock {
                    index: i as u32,
                    block: JOURNAL_BLOCK_0 + i as u32,
                    kind: Unused,
                    tid: None,
                    stale: false,
                };
                assert_eq!(b, &want, "{mode:?}");
            }
        }
    }

    #[test]
    fn a_finished_transaction_stays_in_the_log_as_stale_blocks() {
        let mut fs = ext3(JournalMode::Ordered);
        fs.create_file("/f", &pattern(3000)).unwrap();
        assert_eq!(fs.journal_info().unwrap().start, 0);
        let e = entries(&fs);
        assert_eq!(e[1..10], transaction(1, &CREATE_TAGS, true, true));
        assert!(e[10..].iter().all(|&x| x == UNUSED));
        // Data mode logs the three data blocks too.
        let mut fs = ext3(JournalMode::Data);
        fs.create_file("/f", &pattern(3000)).unwrap();
        let homes = [1, 2, 3, 4, 5, 6, 69, 1111, 1112, 1113];
        let e = entries(&fs);
        assert_eq!(e[1..13], transaction(1, &homes, true, true));
        assert!(e[13..].iter().all(|&x| x == UNUSED));
    }

    #[test]
    fn a_crashed_transaction_is_live_and_the_one_before_it_stale() {
        for phase in [
            CrashPhase::BeforeCommit,
            CrashPhase::AfterCommit,
            CrashPhase::DuringCheckpoint,
        ] {
            let mut fs = ext3(JournalMode::Ordered);
            fs.create_file("/f", &pattern(3000)).unwrap();
            fs.arm_crash(phase).unwrap();
            fs.create_file("/g", &pattern(3000)).unwrap();
            // Transaction 2 is live from journal block 10, where `s_start`
            // points; `s_sequence` names it.
            let info = fs.journal_info().unwrap();
            assert_eq!((info.start, info.sequence), (10, 2), "{phase:?}");
            let e = entries(&fs);
            assert_eq!(e[1..10], transaction(1, &CREATE_TAGS, true, true));
            assert_eq!(e[info.start as usize].1, Some(info.sequence));
            let committed = phase != CrashPhase::BeforeCommit;
            let live = transaction(2, &CREATE_TAGS, committed, false);
            assert_eq!(e[10..10 + live.len()], live, "{phase:?}");
            assert!(e[10 + live.len()..].iter().all(|&x| x == UNUSED));
        }
    }

    /// An ordered volume whose log is filled with empty-file creates up to
    /// the last 8 blocks: transactions 1 to 127, the last ending at journal
    /// block 1017, so the next one starts at 1018 and must wrap.
    fn nearly_full() -> ExtFs {
        let mut fs = ext3(JournalMode::Ordered);
        let mut i = 0;
        while fs.journal_info().unwrap().head < 1024 - 8 {
            fs.create_file(&format!("/f{i:03}"), b"").unwrap();
            i += 1;
        }
        let info = fs.journal_info().unwrap();
        assert_eq!((i, info.head, info.sequence), (127, 1018, 128));
        fs
    }

    /// A `create_dir` at journal block 1018: the descriptor, five copies up
    /// to block 1023, three more at 1..=3, the commit at 4.
    const WRAPPED_TAGS: [u32; 8] = [1, 2, 3, 4, 5, 22, 1111, 1112];

    #[test]
    fn a_live_transaction_across_the_end_of_the_log_keeps_every_block() {
        let mut fs = nearly_full();
        fs.arm_crash(CrashPhase::AfterCommit).unwrap();
        fs.create_dir("/wrapped").unwrap();
        assert_eq!(fs.journal_info().unwrap().start, 1018);
        let e = entries(&fs);
        let live = transaction(128, &WRAPPED_TAGS, true, false);
        let at: Vec<usize> = (1018..1024).chain(1..5).collect();
        let got: Vec<Entry> = at.iter().map(|&i| e[i]).collect();
        assert_eq!(got, live);
        // Nothing else is live: the rest is stale or unused.
        for (i, &(kind, _, stale)) in e.iter().enumerate().skip(1) {
            if !at.contains(&i) {
                assert!(stale || kind == Unused, "index {i}: {kind:?}");
            }
        }
        // Transaction 1 lost its descriptor and first copies to the wrap
        // (its remaining copies at 5..=7 are unclaimed), but its commit at
        // 8 is still there; transactions 2 to 127 are whole.
        assert_eq!(e[5..8], [UNUSED; 3]);
        assert_eq!(e[8], (Commit, Some(1), true));
        let descriptors: BTreeSet<u32> = e
            .iter()
            .filter(|x| x.0 == Descriptor && x.2)
            .map(|x| x.1.unwrap())
            .collect();
        assert_eq!(descriptors, (2..=127).collect());
    }

    #[test]
    fn a_finished_transaction_across_the_end_of_the_log_is_claimed_whole() {
        let mut fs = nearly_full();
        fs.create_dir("/wrapped").unwrap();
        assert_eq!(fs.journal_info().unwrap().start, 0);
        let e = entries(&fs);
        let at = (1018..1024).chain(1..5);
        let got: Vec<Entry> = at.map(|i| e[i]).collect();
        assert_eq!(got, transaction(128, &WRAPPED_TAGS, true, true));
    }

    #[test]
    fn an_escaped_copy_reports_it() {
        let mut fs = ext3(JournalMode::Data);
        let mut data = pattern(1024);
        data[..4].copy_from_slice(&MAGIC);
        fs.create_file("/magic", &data).unwrap();
        let blocks = fs.journal_blocks();
        let escaped: Vec<&JournalBlock> = blocks
            .iter()
            .filter(|b| matches!(b.kind, JournalBlockKind::Copy { escaped: true, .. }))
            .collect();
        assert_eq!(
            escaped,
            [&JournalBlock {
                index: 9,
                block: 91,
                kind: JournalBlockKind::Copy {
                    home: 1111,
                    escaped: true,
                },
                tid: Some(1),
                stale: true,
            }]
        );
    }

    #[test]
    fn ext2_has_no_journal_blocks() {
        let fs = ExtFs::format(ExtFormatOptions::default()).unwrap();
        assert!(fs.journal_blocks().is_empty());
    }
}
```

Run: `cargo test -p ext --test ext3 inspect`
Expected: the build fails (the library itself still carries the five dead-code warnings of Step 2). The error lines, sorted:

```
error[E0432]: unresolved import `ext::JournalBlockKind`
error[E0432]: unresolved imports `ext::JournalBlock`, `ext::JournalBlockKind`
error[E0599]: no method named `journal_blocks` found for reference `&ExtFs` in the current scope
error[E0599]: no method named `journal_blocks` found for struct `ExtFs` in the current scope   (three times)
error: could not compile `ext` (test "ext3") due to 6 previous errors
```

- [ ] **Step 4: Add `ExtFs::journal_blocks` and the re-export.** In `crates/ext/src/fs.rs`, insert the `inspect` import above the first `use crate::journal::` line, which is `use crate::journal::recovery;` after Task 5, so the imports read `inspect`, `recovery`, `state` (the order rustfmt keeps). Replace:

```rust
use crate::journal::recovery;
```

with:

```rust
use crate::journal::inspect::{self, JournalBlock};
use crate::journal::recovery;
```

Then insert this method directly above `/// The journal mode; `None` on ext2.` (`pub fn journal_mode`), after `journal_info`:

```rust
    /// Every journal index in order with what it holds, its transaction,
    /// and whether that transaction is stale (spec section 7); empty on
    /// ext2. Reads the raw journal, so it works while the volume needs
    /// recovery or is corrupt.
    pub fn journal_blocks(&self) -> Vec<JournalBlock> {
        self.journal
            .as_ref()
            .map(|j| inspect::classify(&self.disk, j))
            .unwrap_or_default()
    }
```

In `crates/ext/src/lib.rs`, add the re-export above `pub use journal::state::{JournalInfo, JournalState};` so the two lines read:

```rust
pub use journal::inspect::{JournalBlock, JournalBlockKind};
pub use journal::state::{JournalInfo, JournalState};
```

Run: `cargo test -p ext --test ext3 inspect`
Expected: `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 54 filtered out`. One library warning is left until Step 6: ``constant `BS` is never used`` (the classifier reads blocks through Task 5's `parse_block`, so only the tests and Step 6's annotations use `BS`). There is no commit in between.

#### Round 3: the annotations

- [ ] **Step 5: Write the failing annotation tests.** In `crates/ext/tests/ext3.rs`, `mod inspect`'s imports gain `BlockRole`, `DateTime`, and `Range`. Replace:

```rust
    use ext::{CrashPhase, ExtFormatOptions, ExtFs, JournalBlock, JournalBlockKind, JournalMode};
    use std::collections::BTreeSet;
```

with:

```rust
    use ext::{
        BlockRole, CrashPhase, ExtFormatOptions, ExtFs, JournalBlock, JournalBlockKind, JournalMode,
    };
    use fs_core::DateTime;
    use std::collections::BTreeSet;
    use std::ops::Range;
```

Then add these helpers and tests at the end of `mod inspect`, after `ext2_has_no_journal_blocks` and before the module's closing `}`:

```rust
    /// `(label, value, range)` of every annotation of `block`.
    fn notes(fs: &ExtFs, block: u32) -> Vec<(String, String, Range<usize>)> {
        fs.annotate_sector(u64::from(block))
            .into_iter()
            .map(|a| (a.label, a.value, a.range))
            .collect()
    }

    fn want(list: &[(&str, &str, Range<usize>)]) -> Vec<(String, String, Range<usize>)> {
        list.iter()
            .map(|(l, v, r)| (l.to_string(), v.to_string(), r.clone()))
            .collect()
    }

    #[test]
    fn the_journal_superblock_is_annotated_field_by_field() {
        let mut fs = ext3(JournalMode::Ordered);
        let mut fields = vec![
            ("magic", "0xC03B3998", 0..4),
            ("block type", "4", 4..8),
            ("block size", "1024", 12..16),
            ("maxlen", "1024", 16..20),
            ("first", "1", 20..24),
            ("sequence", "1", 24..28),
            ("start", "0", 28..32),
            ("uuid", "e2f5ee00-2026-4923-8000-000000000001", 48..64),
            ("users", "1", 64..68),
        ];
        assert_eq!(notes(&fs, 82), want(&fields));
        // A crashed second transaction: the disk says sequence 2 from 10.
        fs.create_file("/f", &pattern(3000)).unwrap();
        fs.arm_crash(CrashPhase::AfterCommit).unwrap();
        fs.create_file("/g", &pattern(3000)).unwrap();
        fields[5].1 = "2";
        fields[6].1 = "10";
        assert_eq!(notes(&fs, 82), want(&fields));
    }

    #[test]
    fn a_descriptor_shows_its_header_and_every_tag() {
        let mut fs = ext3(JournalMode::Ordered);
        fs.create_file("/f", &pattern(3000)).unwrap();
        assert_eq!(
            notes(&fs, 83),
            want(&[
                ("magic", "0xC03B3998", 0..4),
                ("block type", "1", 4..8),
                ("sequence", "1", 8..12),
                ("tag 1", "block 1, flags none", 12..20),
                ("uuid", "e2f5ee00-2026-4923-8000-000000000001", 20..36),
                ("tag 2", "block 2, flags same_uuid", 36..44),
                ("tag 3", "block 3, flags same_uuid", 44..52),
                ("tag 4", "block 4, flags same_uuid", 52..60),
                ("tag 5", "block 5, flags same_uuid", 60..68),
                ("tag 6", "block 6, flags same_uuid", 68..76),
                ("tag 7", "block 69, flags same_uuid | last", 76..84),
            ])
        );
    }

    #[test]
    fn a_commit_block_shows_its_header_and_commit_time() {
        let mut fs = ext3(JournalMode::Ordered);
        fs.create_file("/f", &pattern(3000)).unwrap();
        let header = [
            ("magic", "0xC03B3998", 0..4),
            ("block type", "2", 4..8),
            ("sequence", "1", 8..12),
        ];
        let mut list = header.to_vec();
        list.push(("commit time", "315532800 (1980-01-01 00:00:00)", 48..56));
        assert_eq!(notes(&fs, 91), want(&list));
        // The second transaction commits at journal block 18 (block 100).
        fs.set_now(DateTime::new(2026, 9, 24, 12, 30, 5));
        fs.create_file("/g", &pattern(3000)).unwrap();
        let mut list = header.to_vec();
        list[2].1 = "2";
        list.push(("commit time", "1790253005 (2026-09-24 12:30:05)", 48..56));
        assert_eq!(notes(&fs, 100), want(&list));
    }

    #[test]
    fn a_copy_names_its_home_and_whether_it_is_stale_or_escaped() {
        let copy = |value: &str| want(&[("journal copy", value, 0..1024)]);
        let mut fs = ext3(JournalMode::Ordered);
        fs.create_file("/f", &pattern(3000)).unwrap();
        assert_eq!(
            notes(&fs, 84),
            copy("copy of block 1 for transaction 1 (stale)")
        );
        assert_eq!(
            notes(&fs, 90),
            copy("copy of block 69 for transaction 1 (stale)")
        );
        // The live transaction's copies are not stale.
        fs.arm_crash(CrashPhase::AfterCommit).unwrap();
        fs.create_file("/g", &pattern(3000)).unwrap();
        assert_eq!(notes(&fs, 93), copy("copy of block 1 for transaction 2"));
        assert_eq!(
            notes(&fs, 84),
            copy("copy of block 1 for transaction 1 (stale)")
        );
        // An escaped copy says so, and so does its tag.
        let mut fs = ext3(JournalMode::Data);
        let mut data = pattern(1024);
        data[..4].copy_from_slice(&MAGIC);
        fs.create_file("/magic", &data).unwrap();
        assert_eq!(
            notes(&fs, 91),
            copy("copy of block 1111 for transaction 1 (stale) (escaped)")
        );
        assert!(notes(&fs, 83).contains(&(
            "tag 8".to_string(),
            "block 1111, flags escape | same_uuid | last".to_string(),
            84..92
        )));
    }

    #[test]
    fn unused_revoke_and_foreign_blocks_are_annotated() {
        let mut fs = ext3(JournalMode::Ordered);
        let unused = want(&[("journal", "unused journal block", 0..1024)]);
        assert_eq!(notes(&fs, 83), unused);
        assert_eq!(notes(&fs, 1105), unused);
        // A revoke block (planted: the emulator never writes one) shows its
        // header only; a superblock header inside the log is unused.
        let header = |blocktype: u32, sequence: u32| {
            [
                MAGIC.to_vec(),
                blocktype.to_be_bytes().to_vec(),
                sequence.to_be_bytes().to_vec(),
            ]
            .concat()
        };
        fs.write_raw(87 * 1024, &header(5, 7)).unwrap();
        fs.write_raw(88 * 1024, &header(4, 0)).unwrap();
        assert!(fs.corruption().is_none());
        let blocks = fs.journal_blocks();
        assert_eq!(
            (blocks[5].kind, blocks[5].tid, blocks[5].stale),
            (JournalBlockKind::Revoke, Some(7), true)
        );
        assert_eq!(blocks[6].kind, Unused);
        assert_eq!(
            notes(&fs, 87),
            want(&[
                ("magic", "0xC03B3998", 0..4),
                ("block type", "5", 4..8),
                ("sequence", "7", 8..12),
            ])
        );
        assert_eq!(notes(&fs, 88), unused);
    }

    #[test]
    fn the_journal_pointer_blocks_keep_the_indirect_annotations() {
        let fs = ext3(JournalMode::Ordered);
        let owner = &fs.block_owners()[&1106];
        assert_eq!(
            (owner.inode, owner.path.as_str(), owner.role),
            (8, "<journal>", BlockRole::Indirect)
        );
        assert_eq!(
            notes(&fs, 1106),
            want(&[("pointer[0..255]", "blocks 94..349", 0..1024)])
        );
        assert_eq!(
            notes(&fs, 1107),
            want(&[
                ("pointer[0..2]", "blocks 1108..1110", 0..12),
                ("pointer[3..255]", "unused", 12..1024),
            ])
        );
        assert_eq!(
            notes(&fs, 1110),
            want(&[
                ("pointer[0..243]", "blocks 862..1105", 0..976),
                ("pointer[244..255]", "unused", 976..1024),
            ])
        );
    }
```

In `crates/ext/src/journal/inspect.rs`, the unit test below writes a descriptor header by hand, so `mod tests` imports `BLOCKTYPE_DESCRIPTOR` too. Replace its imports:

```rust
    use crate::journal::{
        encode_commit, encode_descriptor, write_header, JournalMode, JournalSuperblock, Tag,
        BLOCKTYPE_REVOKE, BLOCKTYPE_SUPERBLOCK_V2,
    };
```

with:

```rust
    use crate::journal::{
        encode_commit, encode_descriptor, write_header, JournalMode, JournalSuperblock, Tag,
        BLOCKTYPE_DESCRIPTOR, BLOCKTYPE_REVOKE, BLOCKTYPE_SUPERBLOCK_V2,
    };
```

and add this unit test at the end of `mod tests`, after `a_broken_journal_superblock_leaves_only_stale_blocks` and before the module's closing `}`:

```rust
    #[test]
    fn every_tag_without_same_uuid_is_followed_by_a_uuid_and_every_flag_is_named() {
        // A foreign descriptor: tag 1 (deleted) and tag 2 (an unknown bit
        // and last) both lack SAME_UUID, so a uuid follows each.
        let mut bytes = [0u8; BS];
        write_header(&mut bytes, BLOCKTYPE_DESCRIPTOR, 6);
        bytes[12..16].copy_from_slice(&7u32.to_be_bytes());
        bytes[18..20].copy_from_slice(&TAG_DELETED.to_be_bytes());
        bytes[20..36].fill(0xAB);
        bytes[36..40].copy_from_slice(&8u32.to_be_bytes());
        bytes[42..44].copy_from_slice(&(0x10 | TAG_LAST).to_be_bytes());
        bytes[44..60].fill(0xCD);
        let entry = JournalBlock {
            index: 1,
            block: 11,
            kind: Descriptor,
            tid: Some(6),
            stale: true,
        };
        let notes: Vec<(String, String, std::ops::Range<usize>)> = annotate(&bytes, &entry)
            .into_iter()
            .map(|a| (a.label, a.value, a.range))
            .collect();
        let first = "abababab-abab-abab-abab-abababababab";
        let second = "cdcdcdcd-cdcd-cdcd-cdcd-cdcdcdcdcdcd";
        assert_eq!(
            notes[3..],
            [
                ("tag 1".into(), "block 7, flags deleted".into(), 12..20),
                ("uuid".into(), first.into(), 20..36),
                (
                    "tag 2".into(),
                    "block 8, flags last | 0x0010".into(),
                    36..44
                ),
                ("uuid".into(), second.into(), 44..60),
            ]
        );
        assert_eq!(flag_names(0), "none");
        assert_eq!(flag_names(0x000F), "escape | same_uuid | deleted | last");
    }
```

Run: `cargo test -p ext --test ext3 inspect`
Expected: `test result: FAILED. 8 passed; 5 failed`. The failures are `the_journal_superblock_is_annotated_field_by_field`, `a_descriptor_shows_its_header_and_every_tag`, `a_commit_block_shows_its_header_and_commit_time`, `a_copy_names_its_home_and_whether_it_is_stale_or_escaped`, and `unused_revoke_and_foreign_blocks_are_annotated`, each at its first `assert_eq!` with `left: []` (journal data blocks are unannotated so far); `the_journal_pointer_blocks_keep_the_indirect_annotations` already passes (pointer blocks are role `Indirect`), a regression pin rather than a red test.

Run: `cargo test -p ext --lib inspect`
Expected: the build fails (the E0689 is the `0x10 | TAG_LAST` expression, whose type follows from `TAG_LAST`). The error lines, sorted:

```
error[E0425]: cannot find function `annotate` in this scope
error[E0425]: cannot find function `flag_names` in this scope   (twice)
error[E0425]: cannot find value `TAG_DELETED` in this scope
error[E0425]: cannot find value `TAG_LAST` in this scope
error[E0689]: can't call method `to_be_bytes` on ambiguous numeric type `{integer}`
error: could not compile `ext` (lib test) due to 6 previous errors
```

- [ ] **Step 6: Implement the annotations.** In `crates/ext/src/journal/inspect.rs`, replace the imports:

```rust
use super::recovery::{parse_block, LogBlock, LogEntry, LogWalk};
use super::state::JournalState;
use super::TAG_ESCAPE;
use crate::superblock::BLOCK_SIZE;
use fs_core::Disk;
```

with:

```rust
use super::recovery::{parse_block, LogBlock, LogEntry, LogWalk};
use super::state::JournalState;
use super::{
    be32_at, COMMIT_SEC_OFFSET, HEADER_LEN, TAG_BYTES, TAG_DELETED, TAG_ESCAPE, TAG_LAST,
    TAG_SAME_UUID,
};
use crate::fs::{note, uuid};
use crate::superblock::BLOCK_SIZE;
use fs_core::{Annotation, DateTime, Disk};
```

Then insert this between the end of `classify` and `#[cfg(test)]`:

```rust
/// The journal superblock fields `annotate` names: offset, length, label,
/// and whether the value shows in hex. `users` is `s_nr_users`.
const SUPERBLOCK_FIELDS: [(usize, usize, &str, bool); 9] = [
    (0x00, 4, "magic", true),
    (0x04, 4, "block type", false),
    (0x0C, 4, "block size", false),
    (0x10, 4, "maxlen", false),
    (0x14, 4, "first", false),
    (0x18, 4, "sequence", false),
    (0x1C, 4, "start", false),
    (0x30, 16, "uuid", true),
    (0x40, 4, "users", false),
];

/// The tag flags by name, in bit order.
const TAG_FLAG_NAMES: [(u16, &str); 4] = [
    (TAG_ESCAPE, "escape"),
    (TAG_SAME_UUID, "same_uuid"),
    (TAG_DELETED, "deleted"),
    (TAG_LAST, "last"),
];

/// A journal block's annotations (spec section 7), `bytes` being its 1024
/// bytes and `entry` what `classify` says it holds.
pub(crate) fn annotate(bytes: &[u8], entry: &JournalBlock) -> Vec<Annotation> {
    match entry.kind {
        JournalBlockKind::Superblock => SUPERBLOCK_FIELDS
            .iter()
            .map(|&(offset, len, label, hex)| {
                let field = &bytes[offset..offset + len];
                let value = match (len, hex) {
                    (16, _) => uuid(field),
                    (_, true) => format!("0x{:08X}", be32_at(field, 0)),
                    (_, false) => be32_at(field, 0).to_string(),
                };
                note(offset..offset + len, label, value)
            })
            .collect(),
        JournalBlockKind::Descriptor => {
            let mut notes = header_notes(bytes);
            notes.extend(tag_notes(bytes));
            notes
        }
        JournalBlockKind::Commit => {
            let mut notes = header_notes(bytes);
            let at = COMMIT_SEC_OFFSET;
            let mut sec = [0u8; 8];
            sec.copy_from_slice(&bytes[at..at + 8]);
            let secs = u64::from_be_bytes(sec);
            let date = DateTime::from_unix_seconds(i64::try_from(secs).unwrap_or(i64::MAX));
            notes.push(note(at..at + 8, "commit time", format!("{secs} ({date})")));
            notes
        }
        JournalBlockKind::Revoke => header_notes(bytes),
        JournalBlockKind::Copy { home, escaped } => {
            let tid = entry.tid.unwrap_or_default();
            let mut value = format!("copy of block {home} for transaction {tid}");
            if entry.stale {
                value.push_str(" (stale)");
            }
            if escaped {
                value.push_str(" (escaped)");
            }
            vec![note(0..BS, "journal copy", value)]
        }
        JournalBlockKind::Unused => vec![note(0..BS, "journal", "unused journal block")],
    }
}

/// The 12-byte header: magic (hex), block type, sequence.
fn header_notes(bytes: &[u8]) -> Vec<Annotation> {
    vec![
        note(0..4, "magic", format!("0x{:08X}", be32_at(bytes, 0))),
        note(4..8, "block type", be32_at(bytes, 4).to_string()),
        note(8..12, "sequence", be32_at(bytes, 8).to_string()),
    ]
}

/// A descriptor's tags as `tag k` (from 1) over their 8 bytes, each uuid
/// (after the first tag and any other tag without `SAME_UUID`) as `uuid`,
/// up to the tag with `LAST` or the end of the block.
fn tag_notes(bytes: &[u8]) -> Vec<Annotation> {
    let mut notes = Vec::new();
    let mut pos = HEADER_LEN;
    let mut k = 1;
    while pos + TAG_BYTES <= bytes.len() {
        let block = be32_at(bytes, pos);
        let flags = u16::from_be_bytes([bytes[pos + 6], bytes[pos + 7]]);
        let value = format!("block {block}, flags {}", flag_names(flags));
        notes.push(note(pos..pos + TAG_BYTES, format!("tag {k}"), value));
        pos += TAG_BYTES;
        if flags & TAG_SAME_UUID == 0 && pos + 16 <= bytes.len() {
            notes.push(note(pos..pos + 16, "uuid", uuid(&bytes[pos..pos + 16])));
            pos += 16;
        }
        if flags & TAG_LAST != 0 {
            break;
        }
        k += 1;
    }
    notes
}

/// The set flags by name joined with ` | ` (an unnamed bit in hex), or
/// `none`.
fn flag_names(flags: u16) -> String {
    let mut names: Vec<String> = TAG_FLAG_NAMES
        .iter()
        .filter(|&&(bit, _)| flags & bit != 0)
        .map(|&(_, name)| name.to_string())
        .collect();
    let other = flags & !(TAG_ESCAPE | TAG_SAME_UUID | TAG_DELETED | TAG_LAST);
    if other != 0 {
        names.push(format!("0x{other:04X}"));
    }
    if names.is_empty() {
        "none".to_string()
    } else {
        names.join(" | ")
    }
}
```

In `crates/ext/src/fs.rs`, make the two slice-2 annotation helpers crate-visible. Replace:

```rust
/// One labelled byte range of a block.
fn note(range: Range<usize>, label: impl Into<String>, value: impl Into<String>) -> Annotation {
```

with:

```rust
/// One labelled byte range of a block.
pub(crate) fn note(
    range: Range<usize>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> Annotation {
```

and replace:

```rust
/// The 16 UUID bytes in the usual 8-4-4-4-12 hex form.
fn uuid(bytes: &[u8]) -> String {
```

with:

```rust
/// The 16 UUID bytes in the usual 8-4-4-4-12 hex form.
pub(crate) fn uuid(bytes: &[u8]) -> String {
```

Still in `crates/ext/src/fs.rs`, `annotate_owned_block` (near the end of the last `impl ExtFs` block, before `fn write_changed`) currently reads:

```rust
    /// The annotations of a directory block (per entry: inode, `rec_len`,
    /// `name_len`, file type, name, slack; or `unused` for a zero inode)
    /// or of a pointer block (runs of `pointer[i]`), or `None` for any
    /// other block, which `annotate_sector` then leaves unannotated.
    pub fn annotate_owned_block(
        &self,
        sector: u64,
        owners: &BTreeMap<u32, BlockOwner>,
    ) -> Option<Vec<Annotation>> {
        let block = u32::try_from(sector).ok()?;
        let owner = owners.get(&block)?;
        let start = block as usize * BS;
        if start + BS > self.disk.len() {
            return None;
        }
        match owner.role {
            BlockRole::Data | BlockRole::Journal => None,
            BlockRole::Directory => Some(annotate_dir_block(self.disk.read(start, BS))),
            BlockRole::Indirect => Some(annotate_pointer_block(&blockmap::read_pointers(
                &self.disk, block,
            ))),
        }
    }
}
```

Replace it with (the doc comment of `annotate_journal_block` records that each call classifies the whole journal):

```rust
    /// The annotations of a directory block (per entry: inode, `rec_len`,
    /// `name_len`, file type, name, slack; or `unused` for a zero inode),
    /// of a pointer block (runs of `pointer[i]`, the journal's included), or
    /// of a journal data block (by what `journal_blocks` says it holds), or
    /// `None` for any other block, which `annotate_sector` then leaves
    /// unannotated.
    pub fn annotate_owned_block(
        &self,
        sector: u64,
        owners: &BTreeMap<u32, BlockOwner>,
    ) -> Option<Vec<Annotation>> {
        let block = u32::try_from(sector).ok()?;
        let owner = owners.get(&block)?;
        let start = block as usize * BS;
        if start + BS > self.disk.len() {
            return None;
        }
        match owner.role {
            BlockRole::Data => None,
            BlockRole::Journal => self.annotate_journal_block(block),
            BlockRole::Directory => Some(annotate_dir_block(self.disk.read(start, BS))),
            BlockRole::Indirect => Some(annotate_pointer_block(&blockmap::read_pointers(
                &self.disk, block,
            ))),
        }
    }

    /// Spec section 7: a journal data block, annotated by what it holds.
    /// `None` when `block` is not one of this volume's journal blocks.
    /// Each call classifies the whole journal (one header read per journal
    /// block) to learn what this one block holds, so a caller that needs
    /// many journal blocks should call `journal_blocks()` once instead.
    fn annotate_journal_block(&self, block: u32) -> Option<Vec<Annotation>> {
        let j = self.journal.as_ref()?;
        let index = j.blocks.iter().position(|&b| b == block)?;
        let entry = inspect::classify(&self.disk, j).swap_remove(index);
        Some(inspect::annotate(self.block_bytes(block), &entry))
    }
}
```

Run: `cargo test -p ext --lib inspect`
Expected: `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out`.

Run: `cargo test -p ext --test ext3 inspect`
Expected: `test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 54 filtered out`, and no warning.

- [ ] **Step 7: Run the gates.**

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p fs-emulator-wasm --target wasm32-unknown-unknown
```

Expected: `cargo fmt` prints nothing; clippy ends with the ``Finished `dev` profile`` line and no warning; every `test result:` line of `cargo test --workspace` is `ok` (after Tasks 5 and 6: ext lib 105 passed, `tests/ext3.rs` 67, `tests/ext2.rs` 52, `tests/e2fsprogs.rs` 21 with e2fsprogs installed, fat lib 82, `fat16` 16, fs-core 34, wasm lib 20; 0 failed); the wasm32 build ends with the ``Finished `dev` profile`` line.

- [ ] **Step 8: Commit.**

```bash
git add crates/ext/src/journal/inspect.rs crates/ext/src/journal/mod.rs crates/ext/src/lib.rs crates/ext/src/fs.rs crates/ext/tests/ext3.rs
git commit -m "feat(ext): journal block classification and annotations"
```

---

### Task 7: Wasm, web unions, and docs

**Spec:** section 8 (the wasm surface: `formatExt3` and `Ext3FormatOptions`, `fsType()`, the seven ext-only journal methods, the DTOs, `NeedsRecovery`, the README items), section 9 (the web layer sees two union additions and nothing more), section 11 (ROADMAP, the crate module docs, the spec and plan referenced like slice 2's), and the Verification section (the wasm-pack and `web/ui` gates, plus CI's `web/demo` build; the by-hand Linux mount test is recorded in the ROADMAP as not yet run). Sections 5, 6, and 7 are what the README describes: the emitted sequence and its event kinds, crash phases and `recover()`, `journal_info` and `journal_blocks`.

The wasm `Volume` gains the ext3 constructor and the journal's crash, recovery, and inspection methods; the TypeScript declarations gain the new DTOs and the `"journal"` region kind; the README and the ROADMAP describe slice 3 as landed, and the root `README.md` stops calling `crates/ext` planned. No Rust outside `crates/wasm` changes behaviour (the `ext` crate doc gains a sentence), and no file under `web/ui` changes: the `RegionKind` union the explorer uses is the one `crates/wasm/src/types.rs` emits into the generated `.d.ts` (`web/ui/src/lib/wasm.ts` only re-exports it), and there is no error-code union to extend (`FsError.code` is `string`). The FAT16 adapter's `defaultColorForRegion` (`web/ui/src/core/attribution.ts`) already falls through to its default colour for an unknown kind, and the explorer names the family in its refusal (`no adapter for ext3`, from `web/ui/src/fs/index.ts`).

Decisions later readers must know:
- `Option` fields serialise as `null`, like every existing DTO (`to_value` uses `serialize_missing_as_null`): a `JournalBlock` always carries the keys `tid`, `home`, and `escaped`, `null` when absent, and the TypeScript type says `number | null` / `boolean | null`. Spec section 8 says so ("Absent optionals serialise as `null`"); the README repeats it.
- Journal events cross the boundary through the existing `EventRecord` `{ kind, text, region }` only (spec section 8): the tid, block, and index values are in `text`, and the clean `recovery_scanned` event's `region` is `{ start: 0, end: 0 }`. No event field becomes a DTO member; the ROADMAP's deferred paragraph tells slice 4 to parse `text` or add the fields then.
- `formatExt2` and `formatExt3` share a private `fn ext_volume(opts: ext::ExtFormatOptions) -> Result<Volume, JsValue>` (the 256 MiB size check, `ExtFs::format`, the boxing); each keeps only its own DTO, `FIELDS`, and `TryFrom` → `BadArgument` mapping.
- There is no `CrashPhase` TypeScript type: `armCrash(phase: string)` and `crashPhase(): string | undefined` are typed `string`, as spec section 8 prescribes, so a union would be a dead export. The README lists the three phase strings.
- Methods that return "nothing" say so the way the contract names: `crashPhase()` returns `Option<String>` (`string | undefined`), and `journalInfo()` returns `undefined` on ext2 (not `null`).
- `armCrash` checks in this order: `NotExt` on FAT (even for a bad phase string), then `BadArgument` for a string that is not `before_commit`, `after_commit`, or `during_checkpoint` (message `crash phase "{phase}" is not "before_commit", "after_commit", or "during_checkpoint"`), then `ExtFs::arm_crash`, which is `Unsupported("this volume has no journal")` on ext2. On ext2 the other journal methods answer without error: `disarmCrash()` is a no-op, `crashPhase()` is `undefined`, `needsRecovery()` is `false`, `recover()` returns the clean record (no changes, one `recovery_scanned` event whose `region` is `{ start: 0, end: 0 }`, Task 5's `Some(0..0)`), `journalInfo()` is `undefined`, `journalBlocks()` is `[]`.
- `Ext3FormatOptions` converts through `ExtFormatOptions`' own `TryFrom` for the four shared keys (same label and uuid messages), then sets `journal: Some(JournalOptions { blocks, mode })`; an unknown `journalMode` is `BadArgument` with `journal mode "{m}" is not "ordered" or "data"`. A non-integer, negative, or non-number `journalBlocks` is refused by `serde-wasm-bindgen`'s `u32` deserialisation (`BadArgument`); a size `ExtFs::format` refuses is `InvalidGeometry`. The ext2 `TryFrom` now writes `journal: None` in place of Task 3's `journal: d.journal,` placeholder (the same value), so `formatExt2` can never format a journal; it rejects `journalBlocks` and `journalMode` through `ExtFormatOptions::FIELDS` (`unknown option "journalBlocks"`).
- The `mke2fs` recipe the README and the ROADMAP give is spec section 8's, `mke2fs -t ext3 -b 1024 -I 128 -O none,has_journal,filetype,sparse_super -J size=1`, run by hand against the loader with e2fsprogs 1.47.4: the image loads as `ext3` (a 1,024-block ordered journal whose index 0 is block 274 on a 16,384-block image), takes a `create_file`, and recovers. The leading `none,` matters: without it `mke2fs -t ext3` adds `ext_attr`, `resize_inode`, `dir_index`, and `large_file`, and `from_image` throws `unsupported: compat features are not supported: ext_attr resize_inode dir_index`.
- Node and pnpm: `node` is not on the default `PATH` here; nvm's `~/.nvm/versions/node/v26.10.0/bin` holds `node` and a global pnpm 12.6.0, and Homebrew's `/opt/homebrew/bin/pnpm` is pnpm 10.33.0. CI pins pnpm 10 (`pnpm/action-setup` `version: 10`); pnpm 11 and later ignore the `pnpm.overrides` field of `web/ui/package.json` and refuse the frozen lockfile (`ERR_PNPM_LOCKFILE_CONFIG_MISMATCH`). So every shell that runs `wasm-pack test` or a `pnpm` gate first runs `export PATH="$PATH:$HOME/.nvm/versions/node/v26.10.0/bin"` and checks that `pnpm --version` prints `10.33.0`: the nvm directory is appended, not prepended, so `node` becomes reachable while `/opt/homebrew/bin` still comes first and `pnpm` resolves to Homebrew's 10.33.0 rather than nvm's 12.6.0. `web/ui` and `web/demo` load the package through `"fs-emulator-wasm": "file:../../crates/wasm/pkg"`, so `wasm-pack build` must run before `pnpm install`.

Everything below was compiled and run on a scratch branch holding Tasks 1 to 6 (after the two fix rounds that followed their critique) plus this task: fmt, workspace clippy, `cargo test --workspace` including the e2fsprogs oracle (and again with `CI=1`), the wasm32 build, `wasm-pack test --node crates/wasm`, `wasm-pack build crates/wasm --target bundler`, the `web/ui` install, test, and build, and the `web/demo` install and build. The steps were then replayed from the base branch by a script that applied exactly the code blocks below in order and ran each command; every "Expected" is the observed output, every intermediate state passed `cargo fmt --all -- --check`, and the replayed tree was identical to the commit. Copy the code verbatim.

**Files:**
- Modify: `crates/wasm/src/dto.rs`: `journal: None,` in `TryFrom<ExtFormatOptions>` (line 333); `Ext3FormatOptions`, its `FIELDS`, and its `TryFrom` (lines 338-388); `JournalInfo` and its `From` (lines 390-420); `JournalBlock` and its `From` (lines 422-459).
- Modify: `crates/wasm/src/volume.rs`: `ext_volume` (lines 86-94); `ext_mut` (lines 171-177); `format_ext2`'s tail, now `ext_volume(opts)` (line 219); `format_ext3` (lines 222-237); the ext-specific `impl Volume` block with its doc, `block_group_count`, and the seven journal methods (lines 487-567).
- Modify: `crates/wasm/src/types.rs`: `"journal"` in `RegionKind` (line 14); `JournalMode`, `Ext3FormatOptions` (lines 19-20); `JournalInfo`, `JournalBlockKind`, `JournalBlock` (lines 21-23).
- Modify: `crates/wasm/src/lib.rs`: module doc (lines 1-4).
- Modify: `crates/wasm/README.md` (whole file, 290 lines).
- Modify: `docs/ROADMAP.md`: the ext section's lead (line 68), the slice-3 bullets (lines 89-112), the slice-4 bullet (lines 116-127), the journal's deferred items (lines 152-178).
- Modify: `crates/ext/src/lib.rs`: crate doc (lines 1-5). Spec section 11 ("`crates/ext`'s module docs describe the journal module"); the file belongs to Task 2, and only its doc comment changes. `crates/ext/src/journal/mod.rs`'s module doc already names `recovery` and `inspect` on the base (the second fix round wrote it), so it needs no edit.
- Modify: `README.md` (the repository root): the diagram's `crates/ext` line (line 26) and the `ext2, ext3` status row (line 45).
- Test: `crates/wasm/src/dto.rs`: `ext3_format_options_add_the_journal_to_the_ext2_keys`, `ext3_format_options_refuse_another_mode_and_keep_the_ext2_checks`, `journal_info_and_blocks_map_a_formatted_journal`, `journal_block_kinds_are_strings_and_only_a_copy_carries_home` (lines 983-1135).
- Test: `crates/wasm/tests/volume.rs`: helpers `fresh_ext3`, `le32`, `be32` (lines 612-626) and the two `formatExt3` tests (lines 628-720); helpers `num`, `text_of`, `event_kinds` (lines 722-739) and the four journal-method tests (lines 741-948). The 15 existing tests are unchanged.

**Interfaces:**

Consumes (from Tasks 1 to 6, on the base branch):
- `fs_core::Error::NeedsRecovery` with `code_of(..) == "NeedsRecovery"` and `region_kind_name(RegionKind::Journal) == "journal"` (Task 1, already in `crates/wasm/src/{error.rs, dto.rs}`).
- `ext::ExtFormatOptions { total_blocks, inodes_per_group, label, uuid, journal: Option<JournalOptions> }` with `ExtFormatOptions::ext3()`; `ext::JournalOptions { blocks: Option<u32>, mode: JournalMode }`; `ext::JournalMode::{parse(&str) -> Option<Self>, as_str(self) -> &'static str}` and `Default` (Ordered); `ext::CrashPhase::{parse, as_str}` (Tasks 2, 3). The `journal: d.journal,` placeholder in `crates/wasm/src/dto.rs` and `enum Inner { Fat(Box<FatFs>), Ext(Box<ExtFs>) }` in `volume.rs` (Task 3).
- `ExtFs::{fs_type, needs_recovery, journal_info() -> Option<JournalInfo>}` and `ext::JournalInfo { inode, maxlen, first_block, sequence, start, head, mode, needs_recovery, max_transaction }` (Task 3); `ExtFs::{arm_crash(CrashPhase) -> Result<()>, disarm_crash(), crash_phase() -> Option<CrashPhase>}`, the crashed record's op suffix and `crashed` event, `Err(NeedsRecovery)` from mutations (Task 4); `ExtFs::recover() -> Result<OpRecord>` and its clean record with one `RecoveryScanned` over `0..0` (Task 5); `ExtFs::journal_blocks() -> Vec<JournalBlock>`, `ext::JournalBlock { index, block, kind, tid, stale }`, `ext::JournalBlockKind::{Superblock, Descriptor, Copy { home, escaped }, Commit, Revoke, Unused}` (Task 6).

Produces (exact):

```rust
// crates/wasm/src/dto.rs
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Ext3FormatOptions {
    pub total_blocks: Option<u32>, pub inodes_per_group: Option<u32>, pub label: Option<String>,
    pub uuid: Option<String>, pub journal_blocks: Option<u32>, pub journal_mode: Option<String>,
}
impl Ext3FormatOptions { pub const FIELDS: &'static [&'static str]; } // totalBlocks, inodesPerGroup, label, uuid, journalBlocks, journalMode
impl TryFrom<Ext3FormatOptions> for ext::ExtFormatOptions { type Error = String; }
    // Err("journal mode \"{m}\" is not \"ordered\" or \"data\""), or the ext2 label/uuid messages
#[derive(Debug, Clone, PartialEq, Eq, Serialize)] #[serde(rename_all = "camelCase")]
pub struct JournalInfo { pub inode: u32, pub maxlen: u32, pub first_block: u32, pub sequence: u32, pub start: u32,
    pub head: u32, pub mode: String, pub needs_recovery: bool, pub max_transaction: u32 }
impl From<ext::JournalInfo> for JournalInfo
#[derive(Debug, Clone, PartialEq, Eq, Serialize)] #[serde(rename_all = "camelCase")]
pub struct JournalBlock { pub index: u32, pub block: u32, pub kind: String, pub tid: Option<u32>,
    pub home: Option<u32>, pub escaped: Option<bool>, pub stale: bool }
impl From<ext::JournalBlock> for JournalBlock   // kind: superblock | descriptor | copy | commit | revoke | unused

// crates/wasm/src/volume.rs
fn ext_volume(opts: ext::ExtFormatOptions) -> Result<Volume, JsValue>;      // size check, ExtFs::format, boxing
impl Volume {
    fn ext_mut(&mut self) -> Result<&mut ExtFs, JsValue>;                        // NotExt on FAT
    pub fn format_ext3(options: JsValue) -> Result<Volume, JsValue>;              // js_name formatExt3
    pub fn arm_crash(&mut self, phase: &str) -> Result<(), JsValue>;             // armCrash
    pub fn disarm_crash(&mut self) -> Result<(), JsValue>;                      // disarmCrash
    pub fn crash_phase(&self) -> Result<Option<String>, JsValue>;               // crashPhase
    pub fn needs_recovery(&self) -> Result<bool, JsValue>;                      // needsRecovery
    pub fn recover(&mut self) -> Result<JsValue, JsValue>;                      // recover: OpRecord
    pub fn journal_info(&self) -> Result<JsValue, JsValue>;                     // journalInfo: JournalInfo | undefined
    pub fn journal_blocks(&self) -> Result<JsValue, JsValue>;                   // journalBlocks: JournalBlock[]
}
```

```ts
// the generated .d.ts (crates/wasm/src/types.rs and the wasm-bindgen signatures)
export type RegionKind = "boot" | "metadata" | "allocationTable" | "directory" | "data" | "journal" | "reserved" | "other";
export type JournalMode = "ordered" | "data";
export interface Ext3FormatOptions extends ExtFormatOptions { journalBlocks?: number; journalMode?: JournalMode; }
export interface JournalInfo { inode: number; maxlen: number; firstBlock: number; sequence: number; start: number; head: number; mode: JournalMode; needsRecovery: boolean; maxTransaction: number; }
export type JournalBlockKind = "superblock" | "descriptor" | "copy" | "commit" | "revoke" | "unused";
export interface JournalBlock { index: number; block: number; kind: JournalBlockKind; tid: number | null; home: number | null; escaped: boolean | null; stale: boolean; }
static formatExt3(options: Ext3FormatOptions | undefined): Volume;
armCrash(phase: string): void;
disarmCrash(): void;
crashPhase(): string | undefined;
needsRecovery(): boolean;
recover(): OpRecord;
journalInfo(): JournalInfo | undefined;
journalBlocks(): JournalBlock[];
```

Gate commands used throughout (run from the repository root). `wasm-pack test` needs `node` and the web gates need pnpm 10, so each shell starts with the first two lines, and `pnpm --version` must print `10.33.0`: the nvm directory is appended so that Homebrew's pnpm 10 at `/opt/homebrew/bin` still wins over nvm's pnpm 12, which refuses the frozen lockfile.

```
export PATH="$PATH:$HOME/.nvm/versions/node/v26.10.0/bin"
pnpm --version
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
CI=1 cargo test -p ext --test e2fsprogs
cargo build -p fs-emulator-wasm --target wasm32-unknown-unknown
wasm-pack test --node crates/wasm
wasm-pack build crates/wasm --target bundler
(cd web/ui && pnpm install --frozen-lockfile && pnpm test && pnpm build)
(cd web/demo && pnpm install --frozen-lockfile && pnpm build)
```

---

#### Round 1: the ext3 options, the journal DTOs, and `formatExt3`

- [ ] **Step 1: Write the failing DTO tests.** In `crates/wasm/src/dto.rs`, inside `mod tests`, insert these four tests immediately before `#[test] fn ext_uuids_parse_bare_or_hyphenated_and_nothing_else`:

```rust
    #[test]
    fn ext3_format_options_add_the_journal_to_the_ext2_keys() {
        let core = ext::ExtFormatOptions::try_from(Ext3FormatOptions::default()).unwrap();
        assert_eq!(core, ext::ExtFormatOptions::ext3());
        let dto = Ext3FormatOptions {
            total_blocks: Some(8192),
            inodes_per_group: Some(64),
            label: Some("journal".into()),
            uuid: Some("0123456789abcdef0123456789abcdef".into()),
            journal_blocks: Some(2048),
            journal_mode: Some("data".into()),
        };
        let core = ext::ExtFormatOptions::try_from(dto).unwrap();
        assert_eq!(core.total_blocks, 8192);
        assert_eq!(core.inodes_per_group, Some(64));
        assert_eq!(&core.label, b"journal\0\0\0\0\0\0\0\0\0");
        assert_eq!(core.uuid[..2], [0x01, 0x23]);
        assert_eq!(
            core.journal,
            Some(ext::JournalOptions {
                blocks: Some(2048),
                mode: ext::JournalMode::Data
            })
        );
        let ordered = Ext3FormatOptions {
            journal_mode: Some("ordered".into()),
            ..Default::default()
        };
        assert_eq!(
            ext::ExtFormatOptions::try_from(ordered).unwrap().journal,
            Some(ext::JournalOptions::default())
        );
        assert_eq!(
            Ext3FormatOptions::FIELDS,
            &[
                "totalBlocks",
                "inodesPerGroup",
                "label",
                "uuid",
                "journalBlocks",
                "journalMode"
            ]
        );
        // The ext2 options never carry a journal.
        let ext2 = ext::ExtFormatOptions::try_from(ExtFormatOptions::default()).unwrap();
        assert_eq!(ext2.journal, None);
    }

    #[test]
    fn ext3_format_options_refuse_another_mode_and_keep_the_ext2_checks() {
        for mode in ["writeback", "Ordered", "journal_data", ""] {
            let err = ext::ExtFormatOptions::try_from(Ext3FormatOptions {
                journal_mode: Some(mode.into()),
                ..Default::default()
            })
            .unwrap_err();
            assert_eq!(
                err,
                format!("journal mode \"{mode}\" is not \"ordered\" or \"data\"")
            );
        }
        let err = ext::ExtFormatOptions::try_from(Ext3FormatOptions {
            uuid: Some("xyz".into()),
            ..Default::default()
        })
        .unwrap_err();
        assert_eq!(
            err,
            "uuid \"xyz\" is not 32 hex digits (optionally hyphenated 8-4-4-4-12)"
        );
        assert!(ext::ExtFormatOptions::try_from(Ext3FormatOptions {
            label: Some("x".repeat(17)),
            ..Default::default()
        })
        .is_err());
    }

    #[test]
    fn journal_info_and_blocks_map_a_formatted_journal() {
        let fs = ext::ExtFs::format(ext::ExtFormatOptions::ext3()).unwrap();
        assert_eq!(
            JournalInfo::from(fs.journal_info().unwrap()),
            JournalInfo {
                inode: 8,
                maxlen: 1024,
                first_block: 82,
                sequence: 1,
                start: 0,
                head: 1,
                mode: "ordered".into(),
                needs_recovery: false,
                max_transaction: 256,
            }
        );
        let blocks: Vec<JournalBlock> = fs.journal_blocks().into_iter().map(Into::into).collect();
        assert_eq!(blocks.len(), 1024);
        assert_eq!(
            blocks[0],
            JournalBlock {
                index: 0,
                block: 82,
                kind: "superblock".into(),
                tid: None,
                home: None,
                escaped: None,
                stale: false,
            }
        );
        assert!(blocks[1..].iter().all(|b| b.kind == "unused"));
        assert_eq!(blocks[1023].block, 1105);
    }

    #[test]
    fn journal_block_kinds_are_strings_and_only_a_copy_carries_home() {
        use ext::JournalBlockKind as K;
        let core = |kind, tid| ext::JournalBlock {
            index: 3,
            block: 85,
            kind,
            tid,
            stale: true,
        };
        assert_eq!(
            JournalBlock::from(core(
                K::Copy {
                    home: 69,
                    escaped: true
                },
                Some(2)
            )),
            JournalBlock {
                index: 3,
                block: 85,
                kind: "copy".into(),
                tid: Some(2),
                home: Some(69),
                escaped: Some(true),
                stale: true,
            }
        );
        for (kind, name) in [
            (K::Superblock, "superblock"),
            (K::Descriptor, "descriptor"),
            (K::Commit, "commit"),
            (K::Revoke, "revoke"),
            (K::Unused, "unused"),
        ] {
            let dto = JournalBlock::from(core(kind, Some(7)));
            assert_eq!(dto.kind, name);
            assert_eq!(dto.tid, Some(7));
            assert_eq!((dto.home, dto.escaped), (None, None));
        }
    }
```

Run: `cargo test -p fs-emulator-wasm --lib`
Expected: the test build fails with `error[E0422]: cannot find struct, variant or union type `Ext3FormatOptions` in this scope` (and the same for `JournalInfo` and `JournalBlock`), ending `error: could not compile `fs-emulator-wasm` (lib test) due to 14 previous errors`.

- [ ] **Step 2: Add the DTOs.** In `crates/wasm/src/dto.rs`, in `impl TryFrom<ExtFormatOptions> for ext::ExtFormatOptions`, replace Task 3's placeholder line

```rust
            journal: d.journal,
```

with

```rust
            journal: None,
```

and insert this block after that impl's closing `}` (between it and `fn text(bytes: &[u8]) -> String`), separated by one blank line on each side:

```rust
/// `formatExt3`'s options: `ExtFormatOptions`' four keys plus the
/// journal's. Absent fields take `ext::ExtFormatOptions::ext3()` (mke2fs's
/// journal size for the volume, ordered mode).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Ext3FormatOptions {
    pub total_blocks: Option<u32>,
    pub inodes_per_group: Option<u32>,
    pub label: Option<String>,
    pub uuid: Option<String>,
    pub journal_blocks: Option<u32>,
    pub journal_mode: Option<String>,
}

impl Ext3FormatOptions {
    /// The camelCase keys a JS caller may pass; see `FormatOptions::FIELDS`
    /// for why the boundary checks them by hand.
    pub const FIELDS: &'static [&'static str] = &[
        "totalBlocks",
        "inodesPerGroup",
        "label",
        "uuid",
        "journalBlocks",
        "journalMode",
    ];
}

impl TryFrom<Ext3FormatOptions> for ext::ExtFormatOptions {
    type Error = String;

    fn try_from(o: Ext3FormatOptions) -> Result<Self, String> {
        let mode = match o.journal_mode {
            Some(m) => ext::JournalMode::parse(&m)
                .ok_or_else(|| format!("journal mode \"{m}\" is not \"ordered\" or \"data\""))?,
            None => ext::JournalMode::default(),
        };
        let ext2 = ext::ExtFormatOptions::try_from(ExtFormatOptions {
            total_blocks: o.total_blocks,
            inodes_per_group: o.inodes_per_group,
            label: o.label,
            uuid: o.uuid,
        })?;
        Ok(ext::ExtFormatOptions {
            journal: Some(ext::JournalOptions {
                blocks: o.journal_blocks,
                mode,
            }),
            ..ext2
        })
    }
}

/// `journalInfo()`: the journal's shape and state (spec section 7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalInfo {
    pub inode: u32,
    pub maxlen: u32,
    pub first_block: u32,
    pub sequence: u32,
    pub start: u32,
    pub head: u32,
    /// `"ordered"` or `"data"`.
    pub mode: String,
    pub needs_recovery: bool,
    pub max_transaction: u32,
}

impl From<ext::JournalInfo> for JournalInfo {
    fn from(i: ext::JournalInfo) -> Self {
        JournalInfo {
            inode: i.inode,
            maxlen: i.maxlen,
            first_block: i.first_block,
            sequence: i.sequence,
            start: i.start,
            head: i.head,
            mode: i.mode.as_str().to_string(),
            needs_recovery: i.needs_recovery,
            max_transaction: i.max_transaction,
        }
    }
}

/// One entry of `journalBlocks()`. `kind` is `superblock`, `descriptor`,
/// `copy`, `commit`, `revoke`, or `unused`; `home` and `escaped` are set
/// only for a `copy` and are `null` otherwise, like `tid` on a block that
/// belongs to no transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalBlock {
    pub index: u32,
    pub block: u32,
    pub kind: String,
    pub tid: Option<u32>,
    pub home: Option<u32>,
    pub escaped: Option<bool>,
    pub stale: bool,
}

impl From<ext::JournalBlock> for JournalBlock {
    fn from(b: ext::JournalBlock) -> Self {
        use ext::JournalBlockKind as K;
        let (kind, home, escaped) = match b.kind {
            K::Superblock => ("superblock", None, None),
            K::Descriptor => ("descriptor", None, None),
            K::Copy { home, escaped } => ("copy", Some(home), Some(escaped)),
            K::Commit => ("commit", None, None),
            K::Revoke => ("revoke", None, None),
            K::Unused => ("unused", None, None),
        };
        JournalBlock {
            index: b.index,
            block: b.block,
            kind: kind.to_string(),
            tid: b.tid,
            home,
            escaped,
            stale: b.stale,
        }
    }
}
```

Run: `cargo test -p fs-emulator-wasm --lib`
Expected: `test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` (the 20 existing and the 4 new).

- [ ] **Step 3: Write the failing `formatExt3` boundary tests.** Append to `crates/wasm/tests/volume.rs`, after `format_ext2_options_reach_the_superblock_and_bad_ones_throw`, with one blank line before:

```rust
fn fresh_ext3() -> Volume {
    Volume::format_ext3(JsValue::UNDEFINED).unwrap()
}

/// The little-endian `u32` at a byte offset (the ext superblock's order).
fn le32(v: &Volume, offset: u32) -> u32 {
    let b = v.read_raw(offset, 4).unwrap();
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// The big-endian `u32` at a byte offset (the journal's order).
fn be32(v: &Volume, offset: u32) -> u32 {
    let b = v.read_raw(offset, 4).unwrap();
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

#[wasm_bindgen_test]
fn format_ext3_defaults_and_options_reach_the_disk() {
    let v = fresh_ext3();
    assert_eq!(v.fs_type(), "ext3");
    assert_eq!(v.sector_count(), 16384);
    assert_eq!(v.history_length(), 0);
    assert_eq!(names(&v.list_dir("/").unwrap()), vec!["lost+found"]);
    assert_eq!(le32(&v, 1024 + 0x5C), 0x0004); // s_feature_compat: has_journal
    assert_eq!(le32(&v, 1024 + 0xE0), 8); // s_journal_inum
    assert_eq!(le32(&v, 1024 + 0x100), 0x0040); // s_default_mount_opts: ordered
    assert_eq!(be32(&v, 82 * 1024), 0xC03B_3998); // the journal superblock's magic
    assert_eq!(be32(&v, 82 * 1024 + 0x10), 1024); // s_maxlen
    assert_eq!(
        Volume::format_ext3(JsValue::NULL).unwrap().fs_type(),
        "ext3"
    );

    let custom = obj(&[
        ("totalBlocks", JsValue::from(8192u32)),
        ("label", JsValue::from_str("journal")),
        ("journalBlocks", JsValue::from(2048u32)),
        ("journalMode", JsValue::from_str("data")),
    ]);
    let d = Volume::format_ext3(custom).unwrap();
    assert_eq!(d.fs_type(), "ext3");
    assert_eq!(d.sector_count(), 8192);
    assert_eq!(d.read_raw(1024 + 120, 7).unwrap(), b"journal".to_vec());
    assert_eq!(le32(&d, 1024 + 0x100), 0x0020); // data mode
    assert_eq!(be32(&d, 82 * 1024 + 0x10), 2048);
    let ordered = obj(&[("journalMode", JsValue::from_str("ordered"))]);
    assert_eq!(
        le32(&Volume::format_ext3(ordered).unwrap(), 1024 + 0x100),
        0x0040
    );
    let again = Volume::from_image(d.image()).unwrap();
    assert_eq!(again.fs_type(), "ext3");
    assert_eq!(le32(&again, 1024 + 0x100), 0x0020);
}

#[wasm_bindgen_test]
fn format_ext3_bad_options_throw_and_format_ext2_refuses_the_journal_keys() {
    let fails = |key: &str, value: JsValue| Volume::format_ext3(obj(&[(key, value)])).unwrap_err();
    for (key, value) in [
        ("journalBlocks", JsValue::from(1024.5)),
        ("journalBlocks", JsValue::from(-1)),
        ("journalBlocks", JsValue::from_str("1024")),
        ("journalMode", JsValue::from(1)),
        ("uuid", JsValue::from_str("not-a-uuid")),
        ("label", JsValue::from_str("seventeen-bytes!!")),
    ] {
        assert_eq!(
            code(fails(key, value.clone())),
            "BadArgument",
            "{key}: {value:?}"
        );
    }
    let err = fails("journalMode", JsValue::from_str("writeback"));
    assert_eq!(code(err.clone()), "BadArgument");
    assert_eq!(
        message(&err),
        "journal mode \"writeback\" is not \"ordered\" or \"data\""
    );
    let err = fails("journal", JsValue::TRUE);
    assert_eq!(code(err.clone()), "BadArgument");
    assert_eq!(message(&err), "unknown option \"journal\"");
    let err = fails("totalBlocks", JsValue::from(262_145u32));
    assert_eq!(
        message(&err),
        "volume of 268436480 bytes exceeds the 268435456-byte limit"
    );
    // Sizes the format itself refuses.
    let err = fails("journalBlocks", JsValue::from(1000u32));
    assert_eq!(code(err.clone()), "InvalidGeometry");
    assert_eq!(
        message(&err),
        "invalid geometry: journal size must be at least 1024 blocks"
    );
    let err = fails("totalBlocks", JsValue::from(2047u32));
    assert_eq!(code(err.clone()), "InvalidGeometry");
    assert_eq!(
        message(&err),
        "invalid geometry: a journal needs at least 2048 blocks"
    );
    // formatExt2 knows no journal keys.
    for (key, value) in [
        ("journalBlocks", JsValue::from(1024u32)),
        ("journalMode", JsValue::from_str("ordered")),
    ] {
        let err = Volume::format_ext2(obj(&[(key, value)])).unwrap_err();
        assert_eq!(code(err.clone()), "BadArgument");
        assert_eq!(message(&err), format!("unknown option \"{key}\""));
    }
}
```

Run (in a shell that has run `export PATH="$PATH:$HOME/.nvm/versions/node/v26.10.0/bin"`, as for every `wasm-pack test` below): `wasm-pack test --node crates/wasm`
Expected: the test build fails with `error[E0599]: no function or associated item named `format_ext3` found for struct `Volume` in the current scope` (five times), `error: could not compile `fs-emulator-wasm` (test "volume") due to 5 previous errors`.

- [ ] **Step 4: Add `formatExt3`, the shared `ext_volume`, and the TypeScript options.** In `crates/wasm/src/volume.rs`, insert this function after `fn check_volume_size` (between its closing `}` and `#[wasm_bindgen]` `pub struct Volume {`), with a blank line on each side:

```rust
/// The ext volume `formatExt2` and `formatExt3` return: the size check,
/// then `ExtFs::format` with the converted options.
fn ext_volume(opts: ext::ExtFormatOptions) -> Result<Volume, JsValue> {
    check_volume_size(u64::from(opts.total_blocks) * u64::from(ext::BLOCK_SIZE))?;
    let fs = ExtFs::format(opts).map_err(to_js)?;
    Ok(Volume {
        inner: Inner::Ext(Box::new(fs)),
    })
}
```

In `format_ext2`, replace its last five lines before the closing `}`

```rust
        check_volume_size(u64::from(opts.total_blocks) * u64::from(ext::BLOCK_SIZE))?;
        let fs = ExtFs::format(opts).map_err(to_js)?;
        Ok(Volume {
            inner: Inner::Ext(Box::new(fs)),
        })
```

with

```rust
        ext_volume(opts)
```

Then, in the generic `#[wasm_bindgen] impl Volume` block, insert after `format_ext2` (before `/// Load an image of any family `detect` recognises`), with a blank line on each side:

```rust
    /// Format a fresh ext3 volume: the ext2 layout plus a journal on inode 8.
    /// `options` may be `undefined`.
    #[wasm_bindgen(js_name = formatExt3)]
    pub fn format_ext3(
        #[wasm_bindgen(unchecked_param_type = "Ext3FormatOptions | undefined")] options: JsValue,
    ) -> Result<Volume, JsValue> {
        let opts: dto::Ext3FormatOptions = if options.is_undefined() || options.is_null() {
            dto::Ext3FormatOptions::default()
        } else {
            reject_unknown_keys(&options, dto::Ext3FormatOptions::FIELDS)?;
            from_value(options)?
        };
        let opts =
            ext::ExtFormatOptions::try_from(opts).map_err(|msg| js_error(BAD_ARGUMENT, &msg))?;
        ext_volume(opts)
    }
```

In `crates/wasm/src/types.rs`, insert two lines inside the `TYPES` string right after the line

```rust
export interface ExtFormatOptions { totalBlocks?: number; inodesPerGroup?: number; label?: string; uuid?: string; }
```

namely:

```rust
export type JournalMode = "ordered" | "data";
export interface Ext3FormatOptions extends ExtFormatOptions { journalBlocks?: number; journalMode?: JournalMode; }
```

Run: `wasm-pack test --node crates/wasm`
Expected: `test result: ok. 17 passed; 0 failed; 0 ignored; 0 filtered out` (the 15 existing, `format_ext3_defaults_and_options_reach_the_disk`, and `format_ext3_bad_options_throw_and_format_ext2_refuses_the_journal_keys`).

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: ends with the ``Finished `dev` profile`` line, no warning.

#### Round 2: the journal methods

- [ ] **Step 5: Write the failing journal-method tests.** Append to `crates/wasm/tests/volume.rs`, after `format_ext3_bad_options_throw_and_format_ext2_refuses_the_journal_keys`, with one blank line before:

```rust
fn num(v: &JsValue, key: &str) -> f64 {
    get(v, key)
        .as_f64()
        .unwrap_or_else(|| panic!("{key} is a number"))
}

fn text_of(v: &JsValue, key: &str) -> String {
    get(v, key)
        .as_string()
        .unwrap_or_else(|| panic!("{key} is a string"))
}

fn event_kinds(record: &JsValue) -> Vec<String> {
    Array::from(&get(record, "events"))
        .iter()
        .map(|e| text_of(&e, "kind"))
        .collect()
}

#[wasm_bindgen_test]
fn journal_info_blocks_and_layout_describe_the_formatted_journal() {
    let v = fresh_ext3();
    let info = v.journal_info().unwrap();
    for (key, want) in [
        ("inode", 8.0),
        ("maxlen", 1024.0),
        ("firstBlock", 82.0),
        ("sequence", 1.0),
        ("start", 0.0),
        ("head", 1.0),
        ("maxTransaction", 256.0),
    ] {
        assert_eq!(num(&info, key), want, "{key}");
    }
    assert_eq!(text_of(&info, "mode"), "ordered");
    assert_eq!(get(&info, "needsRecovery").as_bool(), Some(false));
    assert!(!v.needs_recovery().unwrap());

    let data = Volume::format_ext3(obj(&[
        ("journalMode", JsValue::from_str("data")),
        ("journalBlocks", JsValue::from(2048u32)),
    ]))
    .unwrap();
    let info = data.journal_info().unwrap();
    assert_eq!(text_of(&info, "mode"), "data");
    assert_eq!(num(&info, "maxlen"), 2048.0);
    assert_eq!(num(&info, "maxTransaction"), 512.0);

    let journal: Vec<JsValue> = Array::from(&v.layout().unwrap())
        .iter()
        .filter(|r| text_of(r, "kind") == "journal")
        .collect();
    assert_eq!(journal.len(), 1);
    assert_eq!(text_of(&journal[0], "name"), "journal");
    let sectors = get(&journal[0], "sectors");
    assert_eq!(
        (num(&sectors, "start"), num(&sectors, "end")),
        (82.0, 1106.0)
    );

    let blocks = Array::from(&v.journal_blocks().unwrap());
    assert_eq!(blocks.length(), 1024);
    let first = blocks.get(0);
    assert_eq!(text_of(&first, "kind"), "superblock");
    assert_eq!((num(&first, "index"), num(&first, "block")), (0.0, 82.0));
    let unused = blocks.get(1023);
    assert_eq!(text_of(&unused, "kind"), "unused");
    assert_eq!(num(&unused, "block"), 1105.0);
    for key in ["tid", "home", "escaped"] {
        assert!(get(&unused, key).is_null(), "{key} is null");
    }
    assert_eq!(get(&unused, "stale").as_bool(), Some(false));

    // ext2 has no journal.
    let e = fresh_ext();
    assert!(e.journal_info().unwrap().is_undefined());
    assert_eq!(Array::from(&e.journal_blocks().unwrap()).length(), 0);
    assert!(!e.needs_recovery().unwrap());
    assert!(!names(&e.layout().unwrap()).contains(&"journal".to_string()));
}

#[wasm_bindgen_test]
fn a_crash_after_commit_needs_recovery_and_recover_replays_it() {
    let mut v = fresh_ext3();
    v.arm_crash("after_commit").unwrap();
    let rec = v.create_file("/a.txt", b"hello").unwrap();
    assert_eq!(
        text_of(&rec, "op"),
        "create_file /a.txt (crashed after commit)"
    );
    let kinds = event_kinds(&rec);
    assert_eq!(kinds.last().unwrap(), "crashed");
    assert!(kinds.contains(&"journal_block_written".to_string()));
    assert!(!kinds.contains(&"checkpointed".to_string()));
    let events = Array::from(&get(&rec, "events"));
    assert_eq!(
        text_of(&events.get(events.length() - 1), "text"),
        "crashed after commit"
    );
    assert!(v.needs_recovery().unwrap());
    assert_eq!(v.crash_phase().unwrap(), None);
    let info = v.journal_info().unwrap();
    assert_eq!(get(&info, "needsRecovery").as_bool(), Some(true));
    assert_eq!(num(&info, "start"), 1.0);

    let err = v.create_file("/b.txt", b"x").unwrap_err();
    assert_eq!(code(err.clone()), "NeedsRecovery");
    assert_eq!(message(&err), "needs recovery");
    assert_eq!(v.history_length(), 1);

    let rec = v.recover().unwrap();
    assert_eq!(text_of(&rec, "op"), "recover");
    let kinds = event_kinds(&rec);
    let replayed = kinds.iter().filter(|k| *k == "replayed").count();
    assert_eq!(replayed, 7);
    let mut want = vec!["recovery_scanned".to_string()];
    want.extend(std::iter::repeat_n("replayed".to_string(), replayed));
    want.extend([
        "journal_emptied".to_string(),
        "recovery_flag_cleared".to_string(),
    ]);
    assert_eq!(kinds, want);
    assert!(!v.needs_recovery().unwrap());
    assert_eq!(v.read_file("/a.txt").unwrap(), b"hello");
    let info = v.journal_info().unwrap();
    assert_eq!((num(&info, "sequence"), num(&info, "start")), (3.0, 0.0));
    assert_eq!(v.history_length(), 2);

    // The recovered journal still holds transaction 1, now stale.
    let blocks = Array::from(&v.journal_blocks().unwrap());
    let kinds: Vec<String> = blocks.iter().map(|b| text_of(&b, "kind")).collect();
    let mut want = vec!["superblock".to_string(), "descriptor".to_string()];
    want.extend(std::iter::repeat_n("copy".to_string(), replayed));
    want.push("commit".to_string());
    want.extend(std::iter::repeat_n(
        "unused".to_string(),
        1024 - 3 - replayed,
    ));
    assert_eq!(kinds, want);
    let homes: Vec<f64> = (2..2 + replayed as u32)
        .map(|i| num(&blocks.get(i), "home"))
        .collect();
    assert_eq!(homes, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 69.0]);
    let copy = blocks.get(2);
    assert_eq!(get(&copy, "escaped").as_bool(), Some(false));
    assert_eq!(num(&copy, "tid"), 1.0);
    let descriptor = blocks.get(1);
    assert_eq!(num(&descriptor, "tid"), 1.0);
    assert_eq!(get(&descriptor, "stale").as_bool(), Some(true));
    assert!(get(&descriptor, "home").is_null());
    assert!(get(&descriptor, "escaped").is_null());

    // Mutations work again.
    let rec = v.create_file("/b.txt", b"x").unwrap();
    assert_eq!(text_of(&rec, "op"), "create_file /b.txt");
    assert_eq!(
        names(&v.list_dir("/").unwrap()),
        vec!["lost+found", "a.txt", "b.txt"]
    );
}

#[wasm_bindgen_test]
fn crash_phase_round_trips_and_ext2_refuses_to_arm() {
    let mut v = fresh_ext3();
    assert_eq!(v.crash_phase().unwrap(), None);
    for phase in ["before_commit", "after_commit", "during_checkpoint"] {
        v.arm_crash(phase).unwrap();
        assert_eq!(v.crash_phase().unwrap().as_deref(), Some(phase));
    }
    v.disarm_crash().unwrap();
    assert_eq!(v.crash_phase().unwrap(), None);
    let rec = v.create_file("/a", b"a").unwrap();
    assert_eq!(text_of(&rec, "op"), "create_file /a");
    assert_eq!(event_kinds(&rec).last().unwrap(), "recovery_flag_cleared");

    let err = v.arm_crash("after commit").unwrap_err();
    assert_eq!(code(err.clone()), "BadArgument");
    assert_eq!(
        message(&err),
        "crash phase \"after commit\" is not \"before_commit\", \"after_commit\", or \"during_checkpoint\""
    );
    assert_eq!(v.crash_phase().unwrap(), None);

    // recover() on a clean journal records one event and changes nothing.
    let rec = v.recover().unwrap();
    assert_eq!(text_of(&rec, "op"), "recover");
    assert_eq!(Array::from(&get(&rec, "changes")).length(), 0);
    let events = Array::from(&get(&rec, "events"));
    assert_eq!(events.length(), 1);
    let scanned = events.get(0);
    assert_eq!(text_of(&scanned, "kind"), "recovery_scanned");
    assert_eq!(
        text_of(&scanned, "text"),
        "journal is clean; nothing to replay"
    );
    let region = get(&scanned, "region");
    assert_eq!((num(&region, "start"), num(&region, "end")), (0.0, 0.0));

    let mut e = fresh_ext();
    let err = e.arm_crash("after_commit").unwrap_err();
    assert_eq!(code(err.clone()), "Unsupported");
    assert_eq!(message(&err), "unsupported: this volume has no journal");
    e.disarm_crash().unwrap();
    assert_eq!(e.crash_phase().unwrap(), None);
    let rec = e.recover().unwrap();
    assert_eq!(event_kinds(&rec), vec!["recovery_scanned"]);
}

#[wasm_bindgen_test]
fn journal_methods_throw_not_ext_on_fat() {
    let mut f = fresh();
    let errors = [
        f.arm_crash("after_commit").unwrap_err(),
        f.arm_crash("bogus").unwrap_err(),
        f.disarm_crash().unwrap_err(),
        f.crash_phase().unwrap_err(),
        f.needs_recovery().unwrap_err(),
        f.recover().unwrap_err(),
        f.journal_info().unwrap_err(),
        f.journal_blocks().unwrap_err(),
    ];
    for err in errors {
        assert_eq!(code(err.clone()), "NotExt");
        assert_eq!(message(&err), "not an ext volume");
    }
    assert_eq!(f.history_length(), 0);
}
```

Run: `wasm-pack test --node crates/wasm`
Expected: the test build fails with `error[E0599]: no method named `arm_crash` found for struct `Volume` in the current scope`, and the same for `crash_phase`, `disarm_crash`, `journal_blocks`, `journal_info`, `needs_recovery`, and `recover`; `error: could not compile `fs-emulator-wasm` (test "volume") due to 35 previous errors`.

- [ ] **Step 6: Add the journal methods.** In `crates/wasm/src/volume.rs`, in the private `impl Volume` block, insert after `fn ext(&self)` (before `fn op(&mut self, ...)`), with a blank line on each side:

```rust
    /// The ext volume for a mutation, or `NotExt` on any other family.
    fn ext_mut(&mut self) -> Result<&mut ExtFs, JsValue> {
        match &mut self.inner {
            Inner::Ext(e) => Ok(e),
            Inner::Fat(_) => Err(js_error(NOT_EXT, "not an ext volume")),
        }
    }
```

Replace the whole ext-specific block, which today reads

```rust
/// ext-specific inspection. Each throws `code === "NotExt"` on a non-ext volume.
/// The superblock, inode, and block-ownership DTOs arrive with the explorer's
/// ext panels; the group count is the one method the spec names as ext-only
/// for this slice (decision 3, section 8).
#[wasm_bindgen]
impl Volume {
    /// How many block groups the volume has (2 on the default 16 MiB disk).
    #[wasm_bindgen(js_name = blockGroupCount)]
    pub fn block_group_count(&self) -> Result<u32, JsValue> {
        Ok(self.ext()?.geometry().groups)
    }
}
```

with

```rust
/// ext-specific methods. Each throws `code === "NotExt"` on a non-ext volume.
/// The superblock, inode, and block-ownership DTOs arrive with the explorer's
/// ext panels; for now there are the group count (slice 2) and the ext3
/// journal's crash, recovery, and inspection methods (slice 3, section 8).
/// The journal methods answer on ext2 too: no journal, no armed crash,
/// nothing to recover.
#[wasm_bindgen]
impl Volume {
    /// How many block groups the volume has (2 on the default 16 MiB disk).
    #[wasm_bindgen(js_name = blockGroupCount)]
    pub fn block_group_count(&self) -> Result<u32, JsValue> {
        Ok(self.ext()?.geometry().groups)
    }

    /// Arm a crash for the next journaled mutation: `"before_commit"`,
    /// `"after_commit"`, or `"during_checkpoint"` (`BadArgument` otherwise).
    /// Throws `Unsupported` on ext2, which has no journal.
    #[wasm_bindgen(js_name = armCrash)]
    pub fn arm_crash(&mut self, phase: &str) -> Result<(), JsValue> {
        let fs = self.ext_mut()?;
        let parsed = ext::CrashPhase::parse(phase).ok_or_else(|| {
            js_error(
                BAD_ARGUMENT,
                &format!(
                    "crash phase \"{phase}\" is not \"before_commit\", \"after_commit\", or \"during_checkpoint\""
                ),
            )
        })?;
        fs.arm_crash(parsed).map_err(to_js)
    }

    /// Clear an armed crash; a no-op when none is armed.
    #[wasm_bindgen(js_name = disarmCrash)]
    pub fn disarm_crash(&mut self) -> Result<(), JsValue> {
        self.ext_mut()?.disarm_crash();
        Ok(())
    }

    /// The armed phase, as `armCrash` takes it, or `undefined`.
    #[wasm_bindgen(js_name = crashPhase)]
    pub fn crash_phase(&self) -> Result<Option<String>, JsValue> {
        Ok(self.ext()?.crash_phase().map(|p| p.as_str().to_string()))
    }

    /// Whether a crash left a transaction for `recover()`; path mutations
    /// throw `NeedsRecovery` until then. Always `false` on ext2.
    #[wasm_bindgen(js_name = needsRecovery)]
    pub fn needs_recovery(&self) -> Result<bool, JsValue> {
        Ok(self.ext()?.needs_recovery())
    }

    /// Replay or discard what the journal holds, as a mount does, recorded
    /// as the operation `recover`. On a clean journal (and on ext2) the
    /// record has no changes and one `recovery_scanned` event.
    #[wasm_bindgen(unchecked_return_type = "OpRecord")]
    pub fn recover(&mut self) -> Result<JsValue, JsValue> {
        let r = self.ext_mut()?.recover();
        self.op(r)
    }

    /// The journal's shape and state, or `undefined` on ext2.
    #[wasm_bindgen(js_name = journalInfo, unchecked_return_type = "JournalInfo | undefined")]
    pub fn journal_info(&self) -> Result<JsValue, JsValue> {
        match self.ext()?.journal_info() {
            Some(info) => to_value(&dto::JournalInfo::from(info)),
            None => Ok(JsValue::UNDEFINED),
        }
    }

    /// Every journal block in index order with what it holds; empty on ext2.
    #[wasm_bindgen(js_name = journalBlocks, unchecked_return_type = "JournalBlock[]")]
    pub fn journal_blocks(&self) -> Result<JsValue, JsValue> {
        let list: Vec<dto::JournalBlock> = self
            .ext()?
            .journal_blocks()
            .into_iter()
            .map(Into::into)
            .collect();
        to_value(&list)
    }
}
```

In `crates/wasm/src/types.rs`, insert three lines inside the `TYPES` string right after the `Ext3FormatOptions` line added in Step 4 (no `CrashPhase` type: both phase methods are typed `string`):

```rust
export interface JournalInfo { inode: number; maxlen: number; firstBlock: number; sequence: number; start: number; head: number; mode: JournalMode; needsRecovery: boolean; maxTransaction: number; }
export type JournalBlockKind = "superblock" | "descriptor" | "copy" | "commit" | "revoke" | "unused";
export interface JournalBlock { index: number; block: number; kind: JournalBlockKind; tid: number | null; home: number | null; escaped: boolean | null; stale: boolean; }
```

Run: `wasm-pack test --node crates/wasm`
Expected: `test result: ok. 21 passed; 0 failed; 0 ignored; 0 filtered out` (adds `journal_info_blocks_and_layout_describe_the_formatted_journal`, `a_crash_after_commit_needs_recovery_and_recover_replays_it`, `crash_phase_round_trips_and_ext2_refuses_to_arm`, `journal_methods_throw_not_ext_on_fat`).

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: ends with the ``Finished `dev` profile`` line, no warning.

#### Round 3: the `journal` region kind and the web gates

- [ ] **Step 7: Add `"journal"` to the `RegionKind` union.** In `crates/wasm/src/types.rs`, replace

```rust
export type RegionKind = "boot" | "metadata" | "allocationTable" | "directory" | "data" | "reserved" | "other";
```

with

```rust
export type RegionKind = "boot" | "metadata" | "allocationTable" | "directory" | "data" | "journal" | "reserved" | "other";
```

This is the union `web/ui` uses: `web/ui/src/lib/wasm.ts` re-exports `RegionKind` from `fs-emulator-wasm`, and nothing under `web/ui/src` declares its own. There is no error-code union anywhere (`FsError` is `interface FsError extends Error { code: string; }`), so `"NeedsRecovery"` needs no type change. Nothing under `web/ui` is edited.

- [ ] **Step 8: Build the package and run the web gates.**

Run: `wasm-pack build crates/wasm --target bundler`
Expected: ends `[INFO]: 📦   Your wasm pkg is ready to publish at crates/wasm/pkg.`

Run: `grep -E '^export (type|interface) (RegionKind|JournalMode|Ext3FormatOptions|JournalInfo|JournalBlockKind|JournalBlock) |^    (static )?(formatExt3|armCrash|disarmCrash|crashPhase|needsRecovery|recover|journalInfo|journalBlocks)\(' crates/wasm/pkg/fs_emulator_wasm.d.ts`
Expected, exactly these 14 lines (the six type declarations, then the eight method signatures in the `.d.ts`'s alphabetical order; the pattern is anchored to declarations, so no doc-comment line matches):

```
export type RegionKind = "boot" | "metadata" | "allocationTable" | "directory" | "data" | "journal" | "reserved" | "other";
export type JournalMode = "ordered" | "data";
export interface Ext3FormatOptions extends ExtFormatOptions { journalBlocks?: number; journalMode?: JournalMode; }
export interface JournalInfo { inode: number; maxlen: number; firstBlock: number; sequence: number; start: number; head: number; mode: JournalMode; needsRecovery: boolean; maxTransaction: number; }
export type JournalBlockKind = "superblock" | "descriptor" | "copy" | "commit" | "revoke" | "unused";
export interface JournalBlock { index: number; block: number; kind: JournalBlockKind; tid: number | null; home: number | null; escaped: boolean | null; stale: boolean; }
    armCrash(phase: string): void;
    crashPhase(): string | undefined;
    disarmCrash(): void;
    static formatExt3(options: Ext3FormatOptions | undefined): Volume;
    journalBlocks(): JournalBlock[];
    journalInfo(): JournalInfo | undefined;
    needsRecovery(): boolean;
    recover(): OpRecord;
```

Run: `grep -c CrashPhase crates/wasm/pkg/fs_emulator_wasm.d.ts`
Expected: `0`.

Run: `export PATH="$PATH:$HOME/.nvm/versions/node/v26.10.0/bin" && pnpm --version`
Expected: `10.33.0` (Homebrew's pnpm; the appended nvm directory only supplies `node`, see the decisions above).

Run: `(cd web/ui && pnpm install --frozen-lockfile && pnpm test && pnpm build)`
Expected: the install ends `Done in ... using pnpm v10.33.0`; `pnpm test` reports ` Test Files  39 passed (39)` and `      Tests  315 passed (315)`; `pnpm build` runs `svelte-check` to `COMPLETED 418 FILES 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS` and ends with vite's `✓ built in ...`.

Run: `(cd web/demo && pnpm install --frozen-lockfile && pnpm build)`
Expected: the install ends `Done in ... using pnpm v10.33.0`; the build (`tsc --noEmit && vite build`) ends with vite's `✓ built in ...`.

Run: `git status --short web/ui`
Expected: no output (no test expectation or source under `web/ui` changed; `crates/wasm/pkg` and `web/ui/node_modules` are ignored).

#### Round 4: the README, the ROADMAP, the crate docs, and the root README

- [ ] **Step 9: Rewrite `crates/wasm/README.md`.** Replace the whole file with:

````markdown
# fs-emulator-wasm

Browser bindings for the emulator. One class, `Volume`, exposes the
filesystem-agnostic operations plus family-specific inspection; results are
plain JS objects with TypeScript types in the generated `.d.ts`.

```ts
import { Volume } from "fs-emulator-wasm";

const vol = Volume.formatFat16(undefined);
const record = vol.createFile("/Hello world.txt", new TextEncoder().encode("hi"));
for (const e of record.events) console.log(e.kind, e.text);
console.log(vol.listDir("/"));

const ext = Volume.formatExt2({ label: "teach" });
console.log(ext.fsType(), ext.listDir("/")); // "ext2", [lost+found]

const ext3 = Volume.formatExt3({ journalMode: "data" });
ext3.armCrash("after_commit");
ext3.createFile("/a.txt", new TextEncoder().encode("hi")); // op "create_file /a.txt (crashed after commit)"
ext3.recover(); // replays the committed transaction
```

Errors are `Error` objects with a `code` property; see Error codes.

## Constructors

| Constructor | Options | Volume |
|---|---|---|
| `formatFat16(options)` | `FormatOptions` | FAT16 |
| `formatExt2(options)` | `ExtFormatOptions` | minimal ext2 revision 1, 1 KiB blocks |
| `formatExt3(options)` | `Ext3FormatOptions` | the same ext2 layout plus a JBD2 journal on inode 8 (`fsType()` is `"ext3"`) |
| `fromImage(bytes)` | | whatever family the image is; see Detection |

Every `format*` constructor accepts `undefined` (or `null`) for the defaults.

```ts
interface ExtFormatOptions {
  totalBlocks?: number;    // default 16384 (16 MiB, two block groups); 64 to 262144
  inodesPerGroup?: number; // default one inode per 16 KiB; a multiple of 8, 16 to 8192
  label?: string;          // at most 16 bytes of UTF-8; default empty
  uuid?: string;           // 32 hex digits, bare or hyphenated 8-4-4-4-12;
                           // default e2f5ee00-2026-4923-8000-000000000001
}

interface Ext3FormatOptions extends ExtFormatOptions {
  journalBlocks?: number;         // default mke2fs's size for the volume: 1024 below
                                  // 32,768 blocks, 4096 below 262,144, else 8192
  journalMode?: "ordered" | "data"; // default "ordered"
}
```

On an ext volume the disk's sector is the block, so `sectorSize()` is 1024
and sector N is block N. An unknown key (including `journalBlocks` or
`journalMode` given to `formatExt2`), a value of the wrong type (a
`journalBlocks` that is not a non-negative integer, a `journalMode` other than
the two strings), a malformed `uuid`, a label longer than 16 bytes, or a
volume over the 256 MiB limit (`totalBlocks * 1024`, like FAT's
`totalSectors * bytesPerSector`) throws `BadArgument`; options the format
itself rejects (fewer than 64 blocks, an `inodesPerGroup` that is not a
multiple of 8; for ext3 a volume below 2,048 blocks, a journal below 1,024
blocks, or one above half the volume's free blocks) throw `InvalidGeometry`.
On the default disk the journal occupies blocks 82 to 1105.

## Generic and filesystem-specific methods

`Volume` wraps the `FileSystem` trait from `fs-core`, so `createFile`,
`writeFile`, `readFile`, `deleteFile`, `createDir`, `removeDir`, `listDir`,
`stat`, `setNow`, `layout`, `annotateSector`, `historyLength`, `historyAt`,
`sectorSize`, `sectorCount`, `sector`, `writeRaw`, `readRaw`, `corruption`,
and `image` work on every filesystem the crate holds. `fsType()` names the
one inside (`"FAT16"`, `"ext2"`, or `"ext3"`).

`writeRaw(offset, bytes)` writes anywhere on the disk and returns an
`OpRecord` like every other mutating call, so the timeline and diff cover it;
a range past the end throws `OutOfBounds` and writes nothing. A write that
touches the family's on-disk metadata re-parses it (FAT: the first 512
bytes, the boot sector; ext: the primary superblock in block 1 and the
primary group descriptor table, and on ext3 also the journal superblock,
inode 8, and the journal's indirect blocks): if it still parses and fits the
disk, the new geometry is adopted (the disk is the truth); if not, the path
methods (`createFile`, `writeFile`, `readFile`, `deleteFile`, `createDir`,
`removeDir`, `listDir`, `stat`, FAT's `rawDirEntries`, and ext's `recover`)
throw `CorruptImage` with a message naming what no longer parses (`boot
sector no longer parses after a raw write` on FAT, `superblock or group
descriptors no longer parse after a raw write` on ext) until a later
`writeRaw` repairs it. On ext3 a broken journal superblock, inode 8, or
journal indirect block gives that same ext prefix, followed by `: ` and the
cause, even though the superblock and descriptors still parse.
`corruption()` returns that `CorruptImage` message while the gate is up, or
`null` while the volume is mounted. `layout`, `annotateSector`, `sector`,
`readRaw`, `image`, and `writeRaw` keep working meanwhile, and FAT's
`bootSector()`/`geometry()` return the last good values. `readRaw(offset,
len)` returns a copy of the bytes and throws `BadArgument` past the end. Both
take byte offsets, not sector numbers, as non-negative integers (wasm-bindgen
does not validate them).

`bootSector`, `geometry`, `fatEntries`, `clusterChain`, `rawDirEntries`,
`clusterOwners`, and `annotateSectorWith` are FAT-only and throw an error
with code `NotFat` (message `not a FAT volume`) on any other volume; FAT32
will reuse them. The methods below are ext-only and throw `NotExt` (message
`not an ext volume`) on any other volume; the rest of the ext inspection
(superblock, inodes, block ownership) arrives with the explorer's ext panels.
A UI should branch on `fsType()` before calling the specific ones. The plan
is in `docs/ROADMAP.md`.

- `blockGroupCount()`: how many block groups the volume has (2 on the
  default disk).
- `armCrash(phase)`: make the next path mutation stop at `"before_commit"`
  (after the last journal copy), `"after_commit"` (after the commit block),
  or `"during_checkpoint"` (after the first block written home); any other
  string throws `BadArgument`, and on ext2 it throws `Unsupported`
  (`unsupported: this volume has no journal`).
- `disarmCrash()`: clear the armed phase (a no-op when none is armed, and on
  ext2).
- `crashPhase()`: the armed phase as `armCrash` takes it, or `undefined`.
  Firing clears it. Both methods type the phase as `string`; its values are
  exactly `"before_commit"`, `"after_commit"`, and `"during_checkpoint"`.
- `needsRecovery()`: whether a crash left the journal for `recover()`;
  always `false` on ext2.
- `recover()`: replay the committed transactions and discard an uncommitted
  one, as a mount does, recorded as the operation `recover` and returned as
  its `OpRecord`. On a clean journal, and on ext2, the record has no changes
  and one `recovery_scanned` event (`journal is clean; nothing to replay`)
  whose `region` is `{ start: 0, end: 0 }`.
- `journalInfo()`: a `JournalInfo`, or `undefined` on ext2.
- `journalBlocks()`: one `JournalBlock` per journal index, in order; `[]` on
  ext2.

```ts
interface JournalInfo {
  inode: number;          // always 8
  maxlen: number;         // journal blocks, the journal superblock included
  firstBlock: number;     // physical block of journal index 0
  sequence: number;       // the tid the next transaction takes (while
                          // needsRecovery, the crashed transaction's tid)
  start: number;          // s_start on disk: 0 when the journal is empty
  head: number;           // the journal index the next transaction starts at
  mode: "ordered" | "data";
  needsRecovery: boolean;
  maxTransaction: number; // maxlen / 4, the most blocks one transaction may tag
}

interface JournalBlock {
  index: number;          // journal index (0 is the journal superblock)
  block: number;          // physical block
  kind: "superblock" | "descriptor" | "copy" | "commit" | "revoke" | "unused";
  tid: number | null;     // the transaction; null for superblock and unused
  home: number | null;    // copy only: the block it is a copy of
  escaped: boolean | null; // copy only: its first four bytes were the magic
  stale: boolean;         // a leftover of a finished transaction
}
```

Like every `Option` these DTOs carry, an absent `tid`, `home`, or `escaped`
is `null`, never `undefined` or a missing key. On an ext3 volume every path
mutation runs as one journal transaction, so its record shows the whole
write order: first the body's ext2 events (`inode_allocated`,
`data_written`, and the rest, whose regions name bytes that reach the disk
only with the data home or the checkpoint), then the journal's:
`recovery_flag_set`, the data home (ordered mode),
`transaction_started`, `journal_block_written` for the descriptor, each
copy, and the commit, `checkpointed` per block, `journal_emptied`, and
`recovery_flag_cleared`; a crashed record stops at the armed phase, ends with
a `crashed` event, and its `op` gains ` (crashed before commit)`,
` (crashed after commit)`, or ` (crashed during checkpoint)`. `recover()`
records `recovery_scanned`, `replayed` per block or
`transaction_discarded`, `journal_emptied`, and `recovery_flag_cleared`.
While `needsRecovery()` is true the path mutations throw `NeedsRecovery`;
reads, `layout`, `annotateSector`, and `writeRaw` see the raw on-disk state.
`layout()` carries a region of kind `journal` named `journal` over the
journal's data blocks only: its indirect blocks stay in the data region,
which is split around the journal, and a foreign image whose journal is
fragmented gets one `journal (part k)` region per run, so match on the kind,
not the name. `annotateSector` of a journal block explains it (its fields,
its tags, or the block it copies). A transaction that would tag more than
`maxTransaction` blocks throws `Unsupported` and changes nothing: on the
default disk only data-mode writes above roughly 250 KiB reach it.

## Error codes

| Code | Raised when |
|---|---|
| `NotFound`, `AlreadyExists`, `InvalidPath`, `InvalidName` | a path or name does not resolve, already exists, or is malformed |
| `DiskFull`, `DirectoryFull`, `FileTooLarge` | no room for the data, the entry, or the file size |
| `NotADirectory`, `IsADirectory`, `DirectoryNotEmpty` | the path names the wrong kind of entry |
| `InvalidGeometry` | format options the filesystem cannot lay out |
| `CorruptImage` | on-disk metadata that does not parse (see `corruption()`) |
| `Unsupported` | a feature the emulator does not implement, `armCrash` on ext2, or an image with no recognisable signature |
| `OutOfBounds` | `writeRaw` past the end of the disk |
| `NeedsRecovery` | a path mutation on an ext3 volume whose journal needs `recover()` (message `needs recovery`) |
| `BadArgument` | a malformed argument from JS (the wrapper's own) |
| `NotFat`, `NotExt` | a family-specific method on another family (the wrapper's own) |

## Detection

`fromImage(bytes)` names the family from the image's signature before it
parses anything, checking in this order:

1. ext: the image is at least 1,082 bytes long and the `u16le` at offset
   1080 (the superblock's `s_magic`) is `0xEF53`;
2. FAT: the image is at least 512 bytes long and bytes 510..512 are `55 AA`.

The ext magic covers ext2 and ext3 alike; the ext parser tells them apart by
the `has_journal` feature, and `fsType()` then says `"ext2"` or `"ext3"`.
ext goes first because a bootable ext image can also carry `55 AA` at 510.
When an image carries the ext magic but does not parse as ext and also carries
`55 AA` at 510 (a FAT volume can hold `53 EF` at 1080 in its FAT), it is
handed to the FAT parser, and FAT's result, error included, is what
`fromImage` returns.
Whether the metadata behind a signature parses is then that family's parser's
job, so a recognised image can still throw `CorruptImage` (or `Unsupported`
for an ext feature the emulator does not implement). An image that matches
neither signature throws `Unsupported` with the message `unsupported: no
recognisable filesystem signature`; that includes images shorter than 512
bytes and FAT images whose `55 AA` is gone, which used to reach the FAT parser
and throw `CorruptImage`. An image over 256 MiB throws `BadArgument` before
detection.

### Loading an ext2 image

The ext2 loader takes revision 1 images with 1 KiB blocks, 128-byte inodes,
exactly the `filetype` and `sparse_super` features, and no compat features
other than ext3's `has_journal`; anything else there throws `Unsupported`
naming the reason. It also needs 8,192 blocks per group (otherwise
`CorruptImage`).
The intended `mke2fs` form for an image it can load (not yet run against this
crate by hand; CI's e2fsprogs checks go the other way, judging the
emulator's images) is:

```
mke2fs -t ext2 -b 1024 -I 128 -O none,filetype,sparse_super <image>
```

The last group must hold its own metadata (a backup superblock and
descriptor table when it has them, both bitmaps, and its inode table). With
512 inodes per group (the default 16 MiB volume's), group 1 needs 68 blocks,
so `formatExt2` with `totalBlocks` from 8,194 to 8,260 throws
`InvalidGeometry` (`group 1 has N blocks but its metadata needs 68`), and
`fromImage` throws `CorruptImage` with the same text for an image whose
`s_blocks_count` falls there; with the default inode ratio at that size (256
inodes per group, 36 metadata blocks) the range is 8,194 to 8,229. Every
group boundary has the same shape: a last group of 1 block up to one short of
its metadata. `mke2fs` never writes such an image, because it drops a last
group that small from the block count.

### Loading an ext3 image

An ext3 image loads when it meets the ext2 rules above plus the journal's:
an internal journal on inode 8 of at least 1,024 blocks, a version-2 journal
superblock with 1 KiB blocks, no journal features, and ordered or data
journaling (writeback throws `Unsupported`). The `mke2fs` form is:

```
mke2fs -t ext3 -b 1024 -I 128 -O none,has_journal,filetype,sparse_super -J size=1 <image>
```

The leading `none,` matters: without it `mke2fs -t ext3` adds its default
`ext_attr`, `resize_inode`, `dir_index`, and `large_file` features, which
throw `Unsupported`. (Checked by hand with e2fsprogs 1.47.4 on a 16,384-block
image: it loads as `"ext3"` with a 1,024-block ordered journal, takes a
`createFile`, and recovers.) Images from other tools must not carry journal
checksums (`journal_checksum_v2`, `journal_checksum_v3`) or 64-bit tags
(`journal_64bit`); those, like `journal_async_commit` and an external
journal, throw `Unsupported` at load, and `recover()` throws `Unsupported`
for a log that holds revoke records. Loading never replays
the journal: an image whose superblock has `needs_recovery` set (a crashed
volume, or a real one that was not unmounted) loads with `needsRecovery()`
true, its path mutations throw `NeedsRecovery`, and its reads show the
on-disk state until `recover()` replays it, as `e2fsck -fy` would. A loaded
journal starts its next transaction at journal block 1, as the kernel does.

## Build and test

```
wasm-pack build crates/wasm --target bundler   # writes crates/wasm/pkg
wasm-pack test --node crates/wasm              # boundary tests
cargo test -p fs-emulator-wasm                 # native DTO and detection tests
```

## Demo

Build the wasm package first: `wasm-pack build crates/wasm --target bundler`.
The demo is written against FAT16: it formats FAT16 only, and an ext image
loaded into it reports `not a FAT volume` (its file listing calls
`clusterOwners`).

```
cd web/demo && pnpm install && pnpm dev
```
````

- [ ] **Step 10: Update `docs/ROADMAP.md`.** Three edits in the ext sections.

Replace

```markdown
`docs/superpowers/specs/`. Slice 1 has landed; slice 2 is in review on
`feat/ext2`:
```

with

```markdown
`docs/superpowers/specs/`. Slices 1 to 3 have landed:
```

Replace (the end of the slice-2 wasm bullet, the old slice-3 bullet, and the slice-4 bullet)

```markdown
  explorer refuses an ext image with the status `no adapter for ext2`.

Still to do:

- **Slice 3, the ext3 journal:** `has_journal` on the reserved inode 8,
  `formatExt3`, and journaled operations the timeline can replay.
- **Slice 4, the explorer:** an ext adapter under `web/ui/src/fs/`, ext-only
  wasm DTOs (superblock, inodes, block owners), a block-group map that
  replaces the FAT map through `PANELS`, an inode inspector, the terminal's
  ext vocabulary, and scenarios that show indirect blocks and, for ext3, a
  journaled write replaying. It inherits the `web/ui` seams listed under
  "Deferred, by area", decides whether an ext volume's labels say "block"
  instead of "sector", and gives `Unsupported` from `fromImage` its own
  status text (an unrecognised image now shows the generic "That isn't
  supported yet.").
```

with

```markdown
  explorer refuses an ext image with the status `no adapter for ext2`.
- **Slice 3, the ext3 journal** (`2026-09-24-ext3-journal-design.md`, plan
  `docs/superpowers/plans/2026-09-24-ext3-journal.md`): a JBD2 version-2
  journal on the reserved inode 8 (`crates/ext/src/journal/`), in ordered or
  full-data mode chosen at format and kept in `s_default_mount_opts`. Every
  path mutation on an ext3 volume is one transaction whose `OpRecord` shows
  the real write order (needs-recovery flag, data home in ordered mode,
  journal superblock, descriptor, copies, commit, checkpoint, journal
  emptied, flag cleared), so the timeline replays it byte by byte; after
  every operation the image is a cleanly unmounted volume. A crash armed at
  a phase stops the next mutation there as a successful truncated record;
  mutations then throw `NeedsRecovery` until `recover()`, whose result equals
  `e2fsck -fy`'s replay byte for byte outside the superblock copies.
  `layout()` carries a `journal` region, `block_owners` lists the journal as
  `<journal>` on inode 8, and `annotate_sector` explains journal blocks.
  `e2fsck`, `dumpe2fs`, and `debugfs logdump` check the images in CI.
- **Slice 3, wasm:** `Volume.formatExt3` with `Ext3FormatOptions` (the ext2
  keys plus `journalBlocks` and `journalMode`), `fsType()` of `"ext3"`,
  `armCrash`, `disarmCrash`, `crashPhase`, `needsRecovery`, `recover`,
  `journalInfo` (`JournalInfo`), and `journalBlocks` (`JournalBlock[]`), all
  ext-only (`NotExt` elsewhere); the `NeedsRecovery` error code; the
  `journal` region kind in the `RegionKind` union. `fromImage` detection is
  unchanged, with the FAT fallback for an image that carries the ext magic
  but only parses as FAT. The explorer refuses an ext3 image with `no
  adapter for ext3`.

Still to do:

- **Slice 4, the explorer:** an ext adapter under `web/ui/src/fs/`, ext-only
  wasm DTOs (superblock, inodes, block owners), a block-group map that
  replaces the FAT map through `PANELS`, an inode inspector, the terminal's
  ext vocabulary, and scenarios that show indirect blocks and, for ext3, a
  journaled write replaying. For ext3 it adds a journal panel over
  `journalInfo()` and `journalBlocks()`, the `journal` layout region, the
  `<journal>` owner of the journal's blocks, and the crash and recovery
  methods (`armCrash`, `disarmCrash`, `crashPhase`, `needsRecovery`,
  `recover`). It inherits the `web/ui` seams listed under "Deferred, by
  area", decides whether an ext volume's labels say "block" instead of
  "sector", and gives `Unsupported` from `fromImage` its own status text (an
  unrecognised image now shows the generic "That isn't supported yet.").
```

Under "Deferred, by area", after the `**`crates/ext`**` paragraph that ends

```markdown
loadable image, which has not yet been run against the loader.
```

insert, separated by a blank line on each side:

```markdown
**`crates/ext`, the journal** (slice 3): `annotate_sector` on a journal
block classifies the whole journal on every call (about a thousand header
reads on the default disk), so slice 4's panel should call `journalBlocks()`
once and reuse it rather than annotating block by block. One mutation is
one transaction and is never split: a transaction that would tag more than
`maxlen / 4` blocks (256 on the default 1,024-block journal) throws
`Unsupported`, so in data mode a single write above roughly 250 KiB fails on
the default disk (ordered mode tags only metadata). The journal head is not
persisted: a loaded image starts its next transaction at journal block 1, as
a kernel without `journal_cycle_record` does. Revoke records, journal
checksums, 64-bit tags, and writeback mode are `Unsupported`; fast commit,
async commit, and an external journal device are not implemented either.
One transaction is in the journal at a time and there is no lazy
checkpointing: every operation checkpoints immediately and leaves a cleanly
unmounted volume, the largest departure from a kernel, which a slice-4
lesson has to explain. Mounts are not modelled (mount count, orphan list,
`s_state`). Event fields reach JS only inside `text`: the wasm
`EventRecord` is `{ kind, text, region }`, and the clean `recovery_scanned`
event has the empty region `{ start: 0, end: 0 }`, so slice 4 either parses
`text` or adds the event fields to `EventRecord`. `crates/wasm/README.md`
gives the `mke2fs -t ext3 -b 1024 -I 128 -O
none,has_journal,filetype,sparse_super -J size=1` recipe for a loadable ext3
image, checked by hand against the loader; the leading `none,` matters,
since without it mke2fs adds `ext_attr`, `resize_inode`, `dir_index`, and
`large_file`, which the loader refuses. The ignored ext3 mount test in
`crates/ext/tests/mount_linux.rs`
(`linux_replays_a_crashed_ext3_image_like_recover`) has not been run.
```

- [ ] **Step 11: Update the crate docs.** In `crates/wasm/src/lib.rs`, replace

```rust
//! `FileSystem` trait (FAT16 and ext2 today) plus family-specific
//! inspection, with plain JS objects crossing the boundary.
```

with

```rust
//! `FileSystem` trait (FAT16, ext2, and ext3 today) plus family-specific
//! inspection and the ext3 journal's crash and recovery methods, with plain
//! JS objects crossing the boundary.
```

In `crates/ext/src/lib.rs`, replace

```rust
//! Byte-accurate ext2 (revision 1, 1 KiB blocks) on an in-memory disk. The
//! on-disk codecs live here; `ExtFs`, the filesystem itself, joins them in
//! `fs.rs`.
```

with

```rust
//! Byte-accurate ext2 (revision 1, 1 KiB blocks) on an in-memory disk, and
//! ext3: the same volume with a JBD2 journal on inode 8. The on-disk codecs
//! live here; `ExtFs`, the filesystem itself, joins them in `fs.rs`, and
//! `journal` holds the journal: its codecs, format and load, the transaction
//! every mutation runs as, crash recovery, and inspection.
```

`crates/ext/src/journal/mod.rs` needs no edit: on the base its module doc already ends with the `txn`, `recovery`, and `inspect` sentences.

- [ ] **Step 12: Update the root `README.md`.** Two lines still call `crates/ext` planned. In the diagram under "How it fits together", replace

```
crates/fat   crates/ext (planned)      one crate per filesystem family
```

with

```
crates/fat   crates/ext                one crate per filesystem family
```

and in the "Status" table replace

```markdown
| ext2, ext3 (`crates/ext`) | Planned |
```

with

```markdown
| ext2, ext3 (`crates/ext`) | Implemented: ext2, and ext3 with its journal, crash points, and recovery; the explorer does not load ext volumes yet |
```

- [ ] **Step 13: Run the gates.**

```
export PATH="$PATH:$HOME/.nvm/versions/node/v26.10.0/bin"
pnpm --version
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
CI=1 cargo test -p ext --test e2fsprogs
cargo build -p fs-emulator-wasm --target wasm32-unknown-unknown
wasm-pack test --node crates/wasm
wasm-pack build crates/wasm --target bundler
(cd web/ui && pnpm install --frozen-lockfile && pnpm test && pnpm build)
(cd web/demo && pnpm install --frozen-lockfile && pnpm build)
git status --short web/ui
```

Expected: `pnpm --version` prints `10.33.0`; `cargo fmt` prints nothing; clippy ends with the ``Finished `dev` profile`` line and no warning; every `test result:` line of `cargo test --workspace` is `ok` (401 passed, 0 failed, 1 ignored in total with e2fsprogs installed: ext lib 105, `tests/e2fsprogs.rs` 21, `tests/ext2.rs` 52, `tests/ext3.rs` 67, fat lib 82, `tests/fat16.rs` 16, fs-core 34, `fs-emulator-wasm` lib 24; `mount_macos.rs` 1 ignored; `mount_linux.rs` holds no tests off Linux); `CI=1 cargo test -p ext --test e2fsprogs` reports `test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`; the wasm32 build ends with the ``Finished `dev` profile`` line; `wasm-pack test` reports `test result: ok. 21 passed; 0 failed; 0 ignored; 0 filtered out`; `wasm-pack build` ends `Your wasm pkg is ready to publish at crates/wasm/pkg.`; `web/ui` reports 39 test files and 315 tests passed, `svelte-check` 0 errors and 0 warnings, and `✓ built`; `web/demo` ends with `✓ built`; `git status --short web/ui` prints nothing.

- [ ] **Step 14: Commit.**

```bash
git add crates/wasm/src/dto.rs crates/wasm/src/volume.rs crates/wasm/src/types.rs crates/wasm/src/lib.rs crates/wasm/tests/volume.rs crates/wasm/README.md docs/ROADMAP.md crates/ext/src/lib.rs README.md
git commit -m "feat(wasm): ext3 formatting, crash and recovery, and journal inspection"
```
