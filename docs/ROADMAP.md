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
  This includes raw writes (`FileSystem::write_raw`), which journal like any
  other operation and are never capped in Rust; a filesystem re-parses its
  on-disk metadata when a raw write touches it and reports `CorruptImage`
  from path operations until that metadata parses again. The UI's timeline,
  diff replay, rewind, and terminal are built only on that journal, so a new
  filesystem gets them for free.
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
- **Per-family UI knowledge lives behind `FsAdapter`.** What the explorer
  knows about one family (its allocation-unit noun, byte ownership, chain
  tracing, `stat` facts, the Format form, the map panel) is one adapter under
  `web/ui/src/fs/<family>/`, chosen from `fsType()` and guarded by a
  source-scan test (`web/ui/tests/adapterBoundary.test.ts`) that keeps family-specific wasm calls out of the rest of
  `web/ui/src`.

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
  FAT32 option, and `clusterCountFor` in `web/ui/src/fs/fat16/format.ts`, the
  JavaScript copy of the cluster-count formula, must follow the Rust one (or
  be replaced by a wasm call).

## Then: `crates/ext`

ext2 first (superblock, block groups, inodes, bitmaps, directory blocks), then
ext3 as ext2 plus a journal. Expected shape:

- `ExtFs` implementing `FileSystem`; regions for the superblock, group
  descriptors, block and inode bitmaps, inode tables, and data blocks.
- Inspection methods of the same kind as FAT's: inode table entries, block
  ownership, the journal's transactions for ext3.
- Wasm: `Inner::Ext`, `formatExt2` / `formatExt3`, ext-only methods throwing
  `NotExt`.
- UI: a block-group map that replaces the FAT map through `PANELS` when an
  ext volume is mounted, an inode inspector, and scenarios that show indirect
  blocks and, for ext3, a journaled write replaying.

## Core API additions

Out of scope so far, in rough order: rename (the shell's `mv` waits on it),
append and truncate to size (the shell's `write --append` rewrites the whole
file and says so), and undo derived from `ByteChange.before` (the journal
already stores it).

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
minimum region width, so tiny regions can vanish at narrow widths;
`src/fs/fat16/direntry.ts` and `src/fs/fat16/remnants.ts` keep their
pre-existing blanket `catch` around `rawDirEntries` on purpose, so a corrupt
volume falls back to "no range" or "skip this entry" instead of throwing.

**`web/ui`, seams the ext slice inherits**: (a) the free-space model in
generic code assumes allocation units exist only in `data` regions and that
an unowned unit is free, at `web/ui/src/core/attribution.ts` (the
`region.kind !== "data"` early return and the `free` marking),
`web/ui/src/components/Inspector.svelte` (the "free" owner fact) and
`web/ui/src/components/Ribbon.svelte` (`freeLabel` counting unowned units);
ext blocks span metadata regions and allocated-but-unowned blocks exist (the
journal, indirect blocks), so the ext slice needs an adapter `isFree(unit)`
and a `df()`-based free label; (b) the terminal's command help and the
`mkfs` flags are captured once in `createCommands`, so a family change must
re-register commands (the spec's non-goal); (c) `entrySlots(path)` returns
one `Interval`, while an ext path has a directory entry and an inode, so
slice 4 widens it to `Interval[]`; (d) `addrHelp` and the dump's `g` prompt
cannot advertise a family's extra address forms such as `i:N`; (e)
`Ribbon.metaLabel` maps a `directory` region to "root", true only for FAT;
(f) `hexAddr`/`hex` formatting is duplicated in `fs/fat16/adapter.ts`,
`shell/commands.ts`, `components/Inspector.svelte`, and `core/lesson.ts` (a
`core/hex.ts` would serve all four).

**`web/ui` terminal** (decided 2026-09-22): the working directory is one
value per page, not per shell session, because `setPrompt` is engine-wide and
two sessions could not show different directories in their prompts anyway;
reads while the timeline is rewound show the latest state and warn (an
`--at-step` flag reading the cached image is the follow-up), except through a
`<` redirect, which has no channel to warn on; `dd` and `cat` are capped at
1 MiB per invocation in the shell, not in Rust; no `mv`, true append, or
truncate until the core has them; the library and its wasm load lazily on
first open; `Escape` closes the drawer only from its bar, since xterm cancels
the key inside the terminal; no tab completion of `/mnt` paths. Deferred:
under 760px the drawer track is capped at 40vh while the store's height can
be 60%, so the first drag on a phone-width window jumps; `dd`'s volume-file
sink bounds `--seek` by the disk size, not a sane file size, so a huge seek
allocates up to the disk size before `DiskFull`; `rm` and `rmdir` clear the
selection even when a different file was selected; on keyboard layouts where
backtick is a dead key only the Terminal button toggles the drawer; `readRaw`
recomputes its display sum; `planWindow`'s message for an absent count on
`/dev/zero` is unreachable through the runner; `select` warns before
validating its target; `flagGiven` and the range message are repeated
between `commands.ts` and `dd.ts`; `commands.ts` should get a second module
before the next command group; the loading and error
notes in the drawer are not live regions; `TerminalStore` is only exercised
manually; `>>` and `write --append` read the whole existing file with no cap
before rewriting it, so appending one byte to a file larger than 1 MiB
journals more than the `dd`/`cat` rule allows in a single step; and the test
harness's `callErr` swallows harness errors.

### browser-terminal follow-ups

All seven shipped in `@benjamin-small/browser-terminal` 0.3.0, and the
explorer adopted them on 2026-09-22. Each workaround they replaced is gone:

1. A prompt prefix (`setPrompt`), so the prompt shows the working directory:
   https://github.com/benjamin-small/browser-terminal/issues/12. The explorer
   was already calling `host.setPrompt(promptFor(vfs.cwd))` on startup and
   after every `cd` and `mkfs`; the call reaches the terminal now instead of
   an optional-chained no-op.
2. `>`, `>>`, and `<` redirection with a host-pluggable file hook:
   https://github.com/benjamin-small/browser-terminal/issues/13. The explorer
   registers `src/shell/redirect.ts` over the same `/mnt` and `/dev` resolver
   its commands use, so a redirect is an ordinary journaled write.
3. `key=value` barewords, so `dd if=/dev/hda count=1` lexes without quotes:
   https://github.com/benjamin-small/browser-terminal/issues/14. `parseDd`
   already accepted that shape; the docs now lead with it.
4. A bytes `Value`, replacing the `{ bytes: "<hex>", length }` blob record:
   https://github.com/benjamin-small/browser-terminal/issues/15. `cat
   --bytes`, `dd` and `xxd` carry `Uint8Array` now, and the terminal shows it
   as `<N bytes>`.
5. A session or pane id on `ctx`, so each shell could keep its own working
   directory: https://github.com/benjamin-small/browser-terminal/issues/16.
   `ctx.session` and `ctx.pane` are available, but the working directory
   stays one per page: `setPrompt` is engine-wide, so per-session directories
   could not be shown in their prompts. The ids are there for whenever that
   changes.
6. `CreateOptions.terminal` (theme, font family, font size) and `setTheme`,
   replacing the `!important` CSS overrides:
   https://github.com/benjamin-small/browser-terminal/issues/17.
   `src/core/terminalTheme.ts` maps the design tokens onto an `ITheme`, and
   the panel re-pushes it when the theme switch flips.
7. A public `focus()`, replacing the `.xterm-helper-textarea` query:
   https://github.com/benjamin-small/browser-terminal/issues/18.

The explorer pins the package exactly (0.3.0) so a minor release cannot move
any of this underneath it.

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
5. UI: extend `FsFamilyId` in `web/ui/src/fs/adapter.ts`, implement
   `FsAdapter` under `web/ui/src/fs/<family>/`, register it in `fs/index.ts`
   and `fs/panels.ts`, add its signature to `detect`, and write scenarios
   that teach what is different about this filesystem.
6. CI already builds every crate for `wasm32-unknown-unknown` and runs the
   web builds; nothing to add unless the crate needs a new tool.
