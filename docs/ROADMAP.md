# Roadmap

What fs-emulator is growing into, what is deliberately unfinished, and the
rules that keep the pieces fitting. Dates are when an item was decided, not
when it is due.

## What stays fixed

These hold for every filesystem the project adds.

- **The disk is the truth.** A filesystem's state is the bytes on the `Disk`;
  nothing is cached beside it that could disagree. Exported images must mount
  on a real OS.
- **Every operation journals its bytes.** An `OpRecord` carries each changed
  byte's offset, before value, and after value, plus plain-language events.
  The UI's timeline, diff replay, and rewind are built only on that journal,
  so a new filesystem gets them for free.
- **`fs-core` stays filesystem-agnostic** with zero external dependencies and
  no I/O, clock, or threads. The `FileSystem` trait, `Region`, and
  `Annotation` are what the wasm layer and the UI program against.
- **One crate per filesystem family**, and one `Inner` variant per family in
  the wasm `Volume`. Filesystem-specific inspection is exposed as extra
  methods that throw a family-specific error code (`NotFat` today) on other
  volumes; the UI branches on `fsType()`.
- **The UI is region-driven.** The hex dump, disk ribbon, byte attribution,
  strings overlay, and timeline read `layout()` regions and the journal, not
  FAT structures. Filesystem-specific panels (the FAT map, the entry and chain
  trace, the scenarios) sit beside them.

## Next: FAT32 in `crates/fat`

Decided 2026-09-21. Already dispatched on `FatVariant`: cluster numbers are
`u32` everywhere, `Geometry::fat_entry_offset` and the entry codec in `table`
match on the variant, and `format` / `from_image` reject anything but FAT16
with `Unsupported`. Still to do:

- `BootSector::parse` reads the FAT16 EBPB at offset 36 unconditionally; FAT32
  needs the common 36 bytes, 28 extra fields, then the EBPB at 64, plus the
  FSInfo sector.
- `dir::slots(DirLocation::Root)` assumes the fixed root region; FAT32 keeps
  the root directory in a cluster chain.
- End-of-chain and bad-cluster markers are FAT16 constants in `decode16`; FAT32
  entries are 28-bit.
- Split `crates/fat/src/fs.rs` (about 1,900 lines) into inspect and dir-scan
  modules while touching it.
- `annotate_sector_with` walks one directory's chain per sector; revisit when
  the root can be a chain.
- The wasm `Volume` gains `formatFat32`; `fromImage` detects the variant from
  the boot sector.
- UI: the FAT map and chain tracing work unchanged; the Format panel needs a
  FAT32 option, and its JavaScript copy of the cluster-count formula must
  follow the Rust one (or be replaced by a wasm call).

## Then: `crates/ext`

ext2 first (superblock, block groups, inodes, bitmaps, directory blocks), then
ext3 as ext2 plus a journal. Expected shape:

- `ExtFs` implementing `FileSystem`; regions for the superblock, group
  descriptors, block and inode bitmaps, inode tables, and data blocks.
- Inspection methods of the same kind as FAT's: inode table entries, block
  ownership, the journal's transactions for ext3.
- Wasm: `Inner::Ext`, `formatExt2` / `formatExt3`, ext-only methods throwing
  `NotExt`.
- UI: a block-group map beside the FAT map, an inode inspector, and scenarios
  that show indirect blocks and, for ext3, a journaled write replaying.

## Core API additions

Out of scope so far, in rough order: rename, append, truncate to size, and
undo derived from `ByteChange.before` (the journal already stores it).

## Deferred, by area

Known and accepted; not bugs.

**`crates/fat`**: NT lowercase bits in short entries are not set.

**`crates/wasm`**: `init` is exported by wasm-bindgen despite being private.
`vite-plugin-top-level-await` and its `@swc/core` pin may be removable from
both web apps with `build.target: esnext`.

**`web/ui`**: the right column should become tabs under 1100px; history
memory is uncapped; the FAT map chain has no arrowheads; `[` and `]` are not
scenario-aware; canvas captions are not live regions; the `prompt()` used for
jump-to-offset should be guarded in browsers that block it; the ribbon has no
minimum region width, so tiny regions can vanish at narrow widths.

## Adding a filesystem: checklist

1. New crate under `crates/`, depending only on `fs-core`, zero external
   dependencies, no I/O. Implement `FileSystem`; report `layout()` regions and
   `annotate_sector()` decoded fields.
2. A spec under `docs/superpowers/specs/` first, then a plan under
   `docs/superpowers/plans/`. Specs are binding; plans record the build.
3. Mount test behind `--ignored`, like `crates/fat/tests/mount_macos.rs`.
4. Wasm: an `Inner` variant, a `formatXxx` constructor, `fromImage`
   detection, family-specific inspection methods with a `NotXxx` error code,
   TypeScript types for any new DTOs.
5. UI: extend `attribution` with the new region kinds and colors, add a
   family-specific map panel, extend the inspector and the Format panel, and
   write scenarios that teach what is different about this filesystem.
6. CI already builds every crate for `wasm32-unknown-unknown` and runs the
   web builds; nothing to add unless the crate needs a new tool.
