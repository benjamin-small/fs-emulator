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
  methods that throw a family-specific error code (`NotFat`, `NotExt`) on
  other volumes; the UI branches on `fsType()`.
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

## Landed: `crates/ext` and the ext explorer

Decided 2026-09-23, in four slices, each with its spec under
`docs/superpowers/specs/`. All four have landed:

- **Slice 1, the adapter seam** (`2026-09-23-fs-adapter-design.md`): every
  FAT assumption in `web/ui` sits behind `FsAdapter`; see "What stays
  fixed".
- **Slice 2, `crates/ext`** (`2026-09-23-ext2-design.md`): `ExtFs`
  implements `FileSystem` over a minimal ext2 revision 1 disk (1 KiB blocks,
  128-byte inodes, the `filetype` and `sparse_super` features, direct,
  single-, and double-indirect blocks, goal-based allocation by block group,
  overwrite in place). `layout()` names each group's superblock or backup,
  descriptors, bitmaps, inode table, and data; `annotate_sector()` decodes
  the superblock, descriptors, bitmaps, inodes, directory entries, and
  indirect pointers; a raw write that breaks the primary superblock or
  descriptors raises the `CorruptImage` gate. `e2fsck -fn`, `dumpe2fs`, and
  `debugfs` check its images in CI.
- **Slice 2, generic wasm support:** `Inner::Ext`, `Volume.formatExt2` with
  `ExtFormatOptions`, `fromImage` detection by the ext magic before FAT's
  signature (an image with neither throws `Unsupported`), `fsType()` of
  `"ext2"`, and the `NotFat` / `NotExt` codes (`blockGroupCount`, the one
  ext-only method the spec names for this slice: decision 3, section 8). The
  explorer refused an ext image with the status `no adapter for ext2`
  until slice 4.
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
  but only parses as FAT.
- **Slice 4, the explorer** (`2026-09-24-ext-explorer-design.md`): seven
  ext-only wasm DTO methods (`extGeometry`, `extSuperblock`, `blockOwners`,
  `inodeNumber`, `extInode`, `dirEntries`, `fileBlocks`); one `ext` family
  under `web/ui/src/fs/ext/` binding both ext2 and ext3 (`fsTypes` on the
  family, the adapter's `name` the volume's type), with journal blocks never
  units and indirect blocks as owner rows; the seam's sector noun beside the
  unit noun (ext says block for both: `b:69`, and `i:12` for an inode), the
  Lesson card's `fileParts`, owner roles and fixed colours, `extras` panels,
  and the optional journal capability (`FsAdapter.journal`,
  `needsRecovery`). The explorer mounts ext2 and ext3 volumes, formatted from
  the Actions panel's Filesystem select or `mkfs --type ext2|ext3`, or loaded
  from an mke2fs image: a block-group map replaces the FAT map, the Inspector
  explains inodes, indirect blocks, and journal blocks, and `stat`, `df`, and
  `seek i:N` speak ext. On ext3 a Journal panel shows the ring with live and
  stale transactions, arms a crash phase, and recovers; the terminal's
  `crash` and `recover` do the same, registered only while the volume has a
  journal (the terminal re-registers its commands when the family changes);
  the status line explains `NeedsRecovery` and shows `Unsupported`'s own
  wasm text. Three ext3 lessons (the fundamentals (ext), a journaled write,
  crash and recover) pin their numbers by test, and the lesson picker groups
  lessons by filesystem. FAT16 is unchanged and still the default volume.

## Core API additions

Out of scope so far, in rough order: rename (the shell's `mv` waits on it),
append and truncate to size (the shell's `write --append` rewrites the whole
file and says so), and undo derived from `ByteChange.before` (the journal
already stores it).

## Deferred, by area

Known and accepted; not bugs.

**`crates/fat`**: NT lowercase bits in short entries are not set.

**`crates/ext`**: `s_wtime` (every operation) and the zeroed tails of the
pointer blocks a shrinking `write_file` keeps change bytes with no event, so
the explorer's attribution sees bytes no event explains. `fs_core::run_op` has
no caller yet (FAT and ext keep their own `run_op`). A last block group too
small for its metadata throws `InvalidGeometry` from `formatExt2` and
`CorruptImage` from `fromImage` (for example 8,194 to 8,260 blocks with 512
inodes per group); `crates/wasm/README.md` gives the range and the
`mke2fs -t ext2 -b 1024 -I 128 -O none,filetype,sparse_super` recipe for a
loadable image, checked against the loader and loaded into the explorer by
the slice-4 browser pass (e2fsprogs 1.47.4, 16,384 blocks).

**`crates/ext`, the journal** (slice 3): `annotate_sector` on a journal
block classifies the whole journal on every call (about a thousand header
reads on the default disk); the Journal panel calls `journalBlocks()` once
per `volume.epoch` and reuses it, but the Inspector still annotates a journal
block this way. One mutation is
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
unmounted volume, the largest departure from a kernel, which the "A
journaled write" lesson explains at its checkpoint step. Mounts are not
modelled (mount count, orphan list, `s_state`). Event fields reach JS only
inside `text`: the wasm `EventRecord` is `{ kind, text, region }`, and the
clean `recovery_scanned` event has the empty region `{ start: 0, end: 0 }`.
Slice 4 kept it that way (a spec non-goal): the explorer shows the `text`,
and the lesson tests parse it to pin journal positions. Adding the event
fields to `EventRecord` is still open. `crates/wasm/README.md`
gives the `mke2fs -t ext3 -b 1024 -I 128 -O
none,has_journal,filetype,sparse_super -J size=1` recipe for a loadable ext3
image, checked by hand against the loader; the leading `none,` matters,
since without it mke2fs adds `ext_attr`, `resize_inode`, `dir_index`, and
`large_file`, which the loader refuses. `JournalState::open` does not yet
reject a journal that overlaps group metadata or maps a block twice; a
crafted foreign image would then have transactions write over metadata.
The explorer now loads foreign images, and the check (`CorruptImage`, as
e2fsck reports) is still to add; slice 4 left the loader alone (a spec
non-goal). The ignored ext3 mount tests in
`crates/ext/tests/mount_linux.rs` were run on 2026-09-25 (Linux 7.0 in a
privileged Docker container; `docs/testing.md` has the command): the kernel
reads the image back and replays the crashed image byte for byte like
`recover()`, apart from `s_overhead_clusters`, which a read-write mount
fills in when it is zero and the comparison now ignores.

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

**`web/ui`, after the ext slice** (the slice-4 browser pass): the `.warn`
text (the Format check line) is hard to read in dark mode in both Format
forms. By design, clicking one of the journal's pointer blocks (`<journal>`,
1106 to 1110 on the default disk) on the block-group map only jumps the dump:
`<journal>` is a pseudo-owner (`isPseudoOwner`), not a tree path, so there is
nothing to select.

**`web/ui`, seams the ext slice inherited**: (a) the free-space model in
generic code assumes allocation units exist only in `data` regions and that
an unowned unit is free. Slice 4 keeps journal blocks out of the units and
owns the indirect and journal pointer blocks, so the map and the Inspector
are right, and the ribbon's free label reads the family's `freeUnits()`: FAT
keeps its count of the clusters no owner row claims, and ext answers `df()`'s
free count (a fresh ext3 disk reads `14.8 MB free`, 15,205 free blocks, where
counting unowned blocks would say `16.0 MB free`). (b) Resolved: the terminal re-registers its commands
when the family changes. (c) `entrySlots(path)`
stays one `Interval` (on ext the inode's slot); the Inspector's trace names
the directory entry. (d) Resolved: `extraAddrHelp` feeds `addrHelp` and the
dump's `g` prompt. (e) `Ribbon.metaLabel` maps a `directory` region to
"root", true only for FAT; ext has no such region, and its legend shows
`boot` for the boot block, the superblock, and the backup superblock alike.
(f) Hex formatting is still repeated: `fs/base.ts`'s `hexAddr` serves the
adapters and the shell, while `components/StringsPanel.svelte`,
`components/Inspector.svelte`, and `core/lesson.ts` each keep a `hex` of their
own.

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
4. Wasm: an `Inner` variant, a `formatXxx` constructor, the family's
   signature in `detect` (`crates/wasm/src/volume.rs`) and a `fromImage` arm
   for it, family-specific inspection methods with a `NotXxx` error code,
   TypeScript types for any new DTOs. `detect` checks the more specific
   signature first: ext's magic at byte 1080 runs before FAT's `55 AA` at
   510, which a bootable ext image can also carry, and an image matching no
   signature throws `Unsupported`.
5. UI: extend `FsFamilyId` in `web/ui/src/fs/adapter.ts`, implement
   `FsAdapter` under `web/ui/src/fs/<family>/`, register it in `fs/index.ts`
   (where `familyIdOf` maps the `fsType()` string to it) and `fs/panels.ts`,
   and write scenarios that teach what is different about this filesystem.
   The UI does no signature detection of its own; `fromImage` does it.
6. CI already builds every crate for `wasm32-unknown-unknown` and runs the
   web builds; nothing to add unless the crate needs a new tool.
