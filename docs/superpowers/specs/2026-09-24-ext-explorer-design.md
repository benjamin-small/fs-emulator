# ext explorer design (slice 4 of the ext work)

The fs explorer learns the ext family: one adapter under `web/ui/src/fs/ext/`
that binds to ext2 and ext3 volumes, a block-group map in place of the FAT
map, an inode-aware Inspector, a Journal panel with the crash and recovery
controls, the terminal's ext vocabulary with `crash`, `recover`, and
`mkfs --type`, three ext3 lessons, and the ext-only wasm DTOs they need. FAT16
keeps every string, colour, and behaviour it has today, and stays the app's
default volume.

Binding for the plan and the implementers. Where a later plan or task text
disagrees with this spec, the spec wins. The slice-1 spec
(`docs/superpowers/specs/2026-09-23-fs-adapter-design.md`) stays binding for
the seam except where section 2 amends it; the slice-2 and slice-3 specs stay
binding for the crates.

## Decisions (2026-09-24, with the user)

1. **Lesson scope: the core set.** Three lessons, all on ext3: a tour of the
   on-disk structure, a journaled write, and crash and recover. ext versions
   of the small FAT lessons come later.
2. **Vocabulary: "block" everywhere on ext.** The disk's sector is the
   block, so the chrome names it once: Block 69, `b:69`, "first block 1111".
   FAT keeps sector and cluster unchanged.
3. **Crash controls in the Journal panel and the terminal.** The panel arms
   a phase and recovers; the terminal gets `crash` and `recover`; the Actions
   panel is unchanged.
4. **One `ext` family with a journal capability.** A single adapter for ext2
   and ext3; an optional `journal` capability on the adapter drives the
   Journal panel and the two commands, so the seam stays family-neutral.

## Outcome

- Format an ext2 or ext3 volume from the Actions panel's Format details
  (with a Filesystem select) or with `mkfs --type ext3`, load an mke2fs
  image, and the tree, ribbon, dump, block-group map, Inspector, Journal
  panel, strings, timeline, diff, and terminal all work on it.
- On ext the chrome says block, the Inspector explains inodes, indirect
  blocks, and journal blocks, `stat` and `df` print ext facts, and
  `seek i:12` jumps to an inode.
- On ext3 the Journal panel shows the ring with live and stale transactions,
  arms a crash phase, and recovers; the terminal does the same with `crash`
  and `recover`; the status line explains `NeedsRecovery`.
- Three lessons run end to end on ext3 and pin their numbers by test.
- Every existing FAT test passes with no expectation edited, with exactly
  one sanctioned change: the `mkfs --label` description pinned in
  `tests/shell/help-text.test.ts` becomes `volume label (fat16: up to 11
  characters; ext: up to 16 bytes)` because the flag is shared by both
  families (section 6). The FAT chrome is unchanged pixel for pixel except
  for the new Filesystem select inside the Format details.

## Non-goals

- No ext remnants view (deleted directory entries are merged, not marked).
- No bitmap or inode editor, no ext versions of the small FAT lessons, no
  FAT32, no overlap check in the ext loader (recorded in the ROADMAP).
- No change to the wasm event record: journal event fields stay in `text`.
- No new browser-terminal version.

## Vocabulary

The **family** is the adapter and its panels (`fat16`, `ext`); the **type**
is `Volume.fsType()` (`FAT16`, `ext2`, `ext3`). The **sector noun** is what
the chrome calls one disk sector (`sector` on FAT, `block` on ext); the
**unit noun** is the allocation unit (`cluster`, `block`). On ext the two
coincide and the chrome shows one name. The **journal capability** is the
optional part of the adapter that exposes the ext3 journal.

## 1. Family and registry

- `FsFamilyId = "fat16" | "ext"`.
- `FsFamily` gains `readonly fsTypes: readonly string[]` (`["FAT16"]`,
  `["ext2", "ext3"]`). `familyIdOf(fsType)` returns the family whose
  `fsTypes` includes the string and throws `no adapter for ${fsType}`
  otherwise. `FsFamily.name` stays the family's display name (`FAT16`,
  `ext`); `FsAdapter.name` becomes the bound volume's type string (`FAT16`,
  `ext2`, `ext3`), which is what the status line and the tree note print.
- `ExtFamilyOptions` (in `src/fs/ext/format.ts`):
  `{ variant?: "ext2" | "ext3"; totalBlocks?: number; inodesPerGroup?: number;
  label?: string; uuid?: string; journalBlocks?: number; journalMode?:
  "ordered" | "data" }`. `ext.format(options)` calls `Volume.formatExt2` for
  `ext2` (throwing `Error("journalBlocks and journalMode need variant ext3")`
  when either journal key is present) and `Volume.formatExt3` for `ext3`,
  the default. `ext.format()` with no options is ext3, ordered, 16,384
  blocks: the disk every ext lesson runs on.
- `DEFAULT_FAMILY` stays `fat16`; the app still opens on FAT16.
- `PANELS` gains `extras: Component[]`, rendered by `App.svelte` under the
  map panel in order (`fat16: []`, `ext: [JournalPanel]`).

## 2. Seam amendments (family-neutral)

- `UnitSpace` gains `readonly sector: AddrVocab` where `AddrVocab = { singular:
  string; plural: string; letter: string }`: FAT `{ "sector", "sectors", "s" }`,
  ext `{ "block", "blocks", "b" }`. `unitIsSector(space)` (a helper in
  `fs/adapter.ts`) is `space.unit.singular === space.sector.singular`.
- `UnitVocab` gains `fileParts: string`, the Lesson card's clause after a
  path: FAT `"its entry, chain, and clusters"`, ext `"its inode, block map,
  and blocks"`. `describeFocus` prints `Files: ${path}, ${fileParts}`; for
  FAT that is the string it prints today. The unit clause keeps "in the data
  region".
- `UnitOwner` gains `role?: "data" | "directory" | "indirect"`; FAT rows leave
  it undefined. `buildAttribution` colours an indirect row with its path's
  hue like a data row; `Attr` gains `role?` copied from the owner.
- `FsAdapter` gains `readonly needsRecovery: boolean` (FAT and ext2: always
  false) and `readonly journal?: JournalCapability`:

  ```ts
  export type CrashPhase = "before_commit" | "after_commit" | "during_checkpoint";
  export const CRASH_PHASES: readonly CrashPhase[];              // in that order
  export const CRASH_PHASE_LABELS: Record<CrashPhase, string>;  // "before commit", "after commit", "during checkpoint"
  export interface JournalState { mode: "ordered" | "data"; sequence: number; head: number; start: number; maxlen: number; firstBlock: number; maxTransaction: number; needsRecovery: boolean }
  export interface JournalRingBlock { index: number; block: number; kind: "superblock" | "descriptor" | "copy" | "commit" | "revoke" | "unused"; tid: number | null; home: number | null; escaped: boolean | null; stale: boolean }
  export interface JournalCapability {
    state(): JournalState;                 // from the adapter's cache (refreshed with it)
    blocks(): JournalRingBlock[];          // reads the volume; callers memoise per epoch
    arm(phase: CrashPhase): void;
    disarm(): void;
    phase(): CrashPhase | null;
    recover(): OpRecord;                   // the volume op; callers run it through volume.run / host.run
  }
  ```

  These types are declared in `fs/adapter.ts` (structural twins of the
  wasm DTOs) so nothing outside `src/fs/ext/` imports an ext-only wasm type.
- `MkfsSpec` loses `done`; the shell composes `formatted /dev/hda as
  ${host.vol.fsType()}; the timeline was cleared`, which for FAT is the
  string it prints today. `MkfsFlag` is unchanged.
- `FsAdapter` gains `readonly extraAddrHelp?: string` (ext: `", i:11 (inode)"`),
  `AddrSpace` picks it up beside `parseAddr`, and `addrHelp` composes `addresses: 0x1f (hex), 512 (decimal),
  ${sector.letter}:65 (${sector.singular})` + (unit distinct from sector ?
  `, ${unit.letter}:3 (${unit.singular})` : ``) + (extraAddrHelp ?? ``).
  FAT's help is byte-identical to today's. `parseAddr` accepts, in order:
  `s:N` (always, a sector), hex, decimal, `${sector.letter}:N` when the
  letter is not `s`, `${unit.letter}:N`, then the family's `parseAddr`.
- `VolumeStore` gains `needsRecovery = $state(false)`, set in `refreshMeta`
  from the adapter.
- The palette gains `COLOR_JOURNAL = 11` with `--own-11` in both themes (a
  warm amber distinct from every file hue and from the table violet), and
  `defaultColorForRegion` maps kind `journal` to it.

## 3. Wasm additions (ext-only)

All on `Volume`, `NotExt` on FAT, DTOs camelCase, absent optionals `null`,
each with a wasm-pack test and a README row. Numbers are plain JS numbers.

| Method | Returns |
|---|---|
| `extGeometry()` | `ExtGeometry { blockSize, totalBlocks, firstDataBlock, blocksPerGroup, inodesPerGroup, inodesCount, inodeSize, inodeTableBlocks, descriptorBlocks, groups: ExtGroup[] }`; `ExtGroup { index, firstBlock, blockCount, superblockBlock: number \| null, descriptorsBlock: number \| null, blockBitmap, inodeBitmap, inodeTable, firstData, freeBlocks, freeInodes, usedDirs }` (layout from `Geometry`, counts from the primary descriptors) |
| `extSuperblock()` | `ExtSuperblock { inodesCount, blocksCount, reservedBlocks, freeBlocks, freeInodes, firstDataBlock, logBlockSize, blocksPerGroup, inodesPerGroup, magic, state, revLevel, firstIno, inodeSize, featureCompat, featureIncompat, featureRoCompat, uuid (hyphenated 8-4-4-4-12), label (NUL-trimmed), journalInum, defaultMountOpts, mtime, wtime, mntCount }` |
| `blockOwners()` | `ExtBlockOwner[] { block, inode, path, role: "data" \| "directory" \| "indirect" \| "journal" }`, ascending by block; the journal's rows carry `path: "<journal>"` |
| `inodeNumber(path)` | the inode number (`NotFound` / `CorruptImage` as `lookup`) |
| `extInode(ino)` | `ExtInode { ino, mode, uid, gid, size, links, blocks, flags, atime, ctime, mtime, dtime, block: number[15], slot: { block, offset } }` where `slot.offset` is the absolute byte offset of the 128-byte slot; `NotFound` for `ino == 0` or past `inodesCount` |
| `dirEntries(path)` | `ExtDirEntry[] { block, offset, inode, recLen, nameLen, fileType, name }` in block then offset order, `offset` absolute, `name` lossy UTF-8, including `.` and `..` and any zero-inode entry |
| `fileBlocks(path)` | `ExtFileBlocks { data: number[] (logical order), indirect: { block, level }[] }` |

`blockGroupCount()` stays. The `types.ts` string gains the interfaces above
plus `ExtBlockOwnerRole`.

## 4. The ext adapter (`web/ui/src/fs/ext/`)

Files: `geometry.ts` (`EXT_UNIT`, `EXT_SECTOR`, `extSpace(geo)`,
`extColorForRegion`), `adapter.ts` (`ExtAdapter`, `ext: FsFamily<ExtFamilyOptions>`),
`format.ts` (`SIZES`, `DEFAULTS`, `checkFormat`, `defaultInodesPerGroup`,
`MKFS`), `journal.ts` (`ExtJournal implements JournalCapability`, the
phase mapping), `metadata.ts` (`CORRUPT_NOTE`, `NOTES`, `touchesMetadata`),
`index.ts` (exports, `asExt(fs)`), `BlockGroupMap.svelte`,
`JournalPanel.svelte`, `ExtFormatForm.svelte`.

- `EXT_UNIT = { singular: "block", plural: "blocks", letter: "b", first: 1,
  fileParts: "its inode, block map, and blocks" }`, `EXT_SECTOR = { singular:
  "block", plural: "blocks", letter: "b" }`.
- `extSpace(geo: ExtGeometry)`: `unitCount = totalBlocks − 1`, `unitSize =
  blockSize`, `sectorSize = blockSize`, `totalSectors = totalBlocks`;
  `unitOfSector(s)` is `s` for `firstDataBlock ≤ s < totalBlocks`, else
  undefined; `unitByteRange(b) = [b·1024, (b+1)·1024)`; `unitOfOffset` divides;
  `unitStartsAt(s)` is `unitOfSector(s) !== undefined`; `colorForRegion` gives
  `journal` regions `COLOR_JOURNAL` and everything else the default.
- `ExtAdapter.refresh()` reads `extGeometry()`, `extSuperblock()`,
  `blockOwners()`, and on ext3 `journalInfo()` and `needsRecovery()`, into
  plain fields. `owners` are the non-journal rows mapped to `UnitOwner {
  unit: block, path, isDir: role === "directory", firstUnit, role }` where
  `firstUnit` is the path's first `data` (or `directory`) block; journal
  rows are kept in a separate `journalBlocks: number[]` field and never
  become units (journal blocks lie in `journal` regions, which attribution
  does not treat as units).
- `chain(path)`: `fileBlocks(path).data` (memoised per refresh by path).
  `entrySlots(path)`: the inode's 128-byte slot. `dataStart("/")`: the root
  directory's first block; a file: its first data block; a directory: its
  first block. `regionStart(kind)`: as FAT.
- `trace(path)` rows, in order: the directory entry (`directory entry in
  ${parent} (block B + 0xNN)`, offset from `dirEntries`; the root has none),
  the inode (`inode ${ino} (block B + 0xNN)`), one row per indirect block
  (`single-indirect block N`, `double-indirect block N`, `indirect block N
  (level 1, under the double)` for second-level pointer blocks), then `data
  block N (first of K)` or the muted `no data blocks`.
- `stat(path)`: `{ inode, inodeOffset: hex, mode: octal string such as
  "0100644", links, blocks512, dataBlocks: number[], indirectBlocks:
  number[], dirEntryOffset: hex or "" }`.
- `df()`: `{ unitSize: blockSize, units: blocksCount, used: blocksCount −
  freeBlocks, free: freeBlocks }` from the cached superblock; the shell
  prints `blockSize` and `blocks`.
- `describeUnit(b)`: `data block ${i} of ${path}` (i the logical index),
  `directory block ${i} of ${path}`, `single-indirect block of ${path}`,
  `double-indirect block of ${path}`, `indirect block of ${path} (level 1)`,
  or `free`; null outside the data area.
- `annotateSector(s)`: `vol.annotateSector(s)`.
- `parseAddr(v)`: `i:N` → `extInode(N).slot.offset`, undefined otherwise;
  `extraAddrHelp = ", i:11 (inode)"`. `namesMatch` is `a === b`.
- `touchesMetadata(changes)`: true when any change starts in a block that is
  below its group's `firstData`, or in one of the journal's blocks, or in an
  indirect block owned by any path.
- `remnants` absent. `needsRecovery` from the cache. `journal` is an
  `ExtJournal` on ext3, undefined on ext2. `CORRUPT_NOTE` and `NOTES` are the
  FAT ones' shape in ext words (the plan pins the strings).
- `asExt(fs)` throws `not an ext adapter: ${fs.id}` for another family.

## 5. Panels and chrome

- **`BlockGroupMap.svelte`** (`PANELS.ext.map`). One canvas, a band per
  block group with a header row of text drawn on the canvas: `group ${i} ·
  blocks ${first}–${last} · ${free} free`. Cells 4 px with a 1 px gap,
  wrapping to the panel width, the whole map scrolling inside the FAT map's
  260 px maximum. Colours: metadata blocks by region kind through
  `attrAtSector` (boot, tables, metadata), journal blocks `COLOR_JOURNAL`,
  owned data and directory blocks the owner's hue, free blocks the hairline;
  an indirect block gets a 2 px dot like the FAT map's end-of-chain mark; the
  selected file's `layers.chain` outlined and joined; blocks overlapping
  `layers.diff` outlined in the diff colour; the hovered dump byte's block
  dashed. Hover caption: `block ${b} · ${regionName}` plus ` · ${ownerPath}`
  when owned, ` · free` for a free data block; click selects the owner (if
  any) and jumps to the block. Heading `Block groups · ${groups} groups ·
  ${totalBlocks} blocks`, with the FAT map's "latest state" note when rewound.
- **`JournalPanel.svelte`** (`PANELS.ext.extras[0]`). On ext2 a single
  muted line `This volume has no journal.` On ext3: heading `Journal ·
  ${mode} mode`; facts `sequence`, `head`, `start`, `blocks` (maxlen), and,
  when `needsRecovery`, the line `Needs recovery: the journal holds an
  unfinished transaction.`; a ring strip canvas of `journal.blocks()`
  (memoised per `volume.epoch`), one cell per journal index in reading order
  wrapping to the panel width, coloured by kind (superblock `COLOR_BOOT`,
  descriptor `COLOR_TABLE`, copy the ink colour, commit the focus colour,
  revoke the diff colour, unused the hairline), stale blocks at 35 % alpha,
  live ones full; hover caption `journal block ${index} (block ${block}) ·
  ${kind}` + ` · copy of block ${home}` + ` · transaction ${tid}` + ` ·
  stale` / ` · live`; click jumps to the block. Controls: a `<select>` of the
  three phases labelled with `CRASH_PHASE_LABELS`, an `Arm` button that
  calls `journal.arm`, replaced while armed by `Armed: the next change
  stops ${label}` and a `Disarm` button; a `Recover` button, enabled only
  while `needsRecovery`, that runs `volume.run(() => journal.recover())`. The controls are
  disabled while `!volume.atLatest`.
- **`ExtFormatForm.svelte`** (`PANELS.ext.format`): Variant (ext2 / ext3,
  default ext3), Size select (`4 MB` 4096, `16 MB` 16384, `64 MB` 65536,
  `256 MB` 262144 blocks), Inodes per group (number input, placeholder the
  derived default from `defaultInodesPerGroup(totalBlocks)`, blank means
  default), Label (up to 16 bytes), and for ext3 only: Journal mode
  (ordered / data) and Journal blocks (placeholder the mke2fs default for
  the size). A `checkFormat` line names the problem and disables the
  button: `too few blocks (minimum 64)`, `too many blocks (maximum 262,144)`,
  `a journal needs at least 2048 blocks`, `the journal must be at least 1024
  blocks`, `label is longer than 16 bytes`. `Format disk` calls
  `volume.format("ext", options)` and clears the selection.
- **`ActionsPanel.svelte`**: inside the Format details, a `Filesystem`
  select listing every family by `name` (`FAT16`, `ext`), initialised to the
  mounted family and re-synced when the mounted family changes; the form
  shown is `PANELS[selected].format`.
- **`App.svelte`** renders `PANELS[id].extras` after the map.
- **`StatusLine.svelte`**: `NeedsRecovery: "This volume needs recovery: the
  journal holds an unfinished transaction. Recover it from the Journal panel
  or with recover."`; `Unsupported` leaves the table so the wasm text shows
  (`no recognisable filesystem signature`, `journal feature journal_64bit`,
  and so on); while `volume.needsRecovery` a line `Volume needs recovery`
  shows beside the corruption line.
- **Inspector**: the address row's label is `cap(sector.singular)`; the unit
  row is omitted when `unitIsSector`, and `describeUnit`'s note joins the
  address row instead (`Block 1111 · data (group 0) · data block 0 of
  /hello.txt`); the owner row and annotations are unchanged.
- **HexView**: row labels use the sector noun: `${unit.singular} ${unit} ·
  ${owner}` on unit starts as today, `${sector.singular} ${sector} ·
  ${region}` otherwise; the `g` prompt names both nouns only when distinct.
- **TreeNodeView**: `first ${unit.singular} N` (already generic).
- **Ribbon**: free label unchanged in shape.
- **LessonPanel / lesson.ts**: `fileParts` as in section 2.
- **ScenarioPanel**: the select groups options in `<optgroup>`s by family
  display name in registry order (`FAT16`, then `ext`), each family's
  scenarios in their `all` order.

## 6. Shell

- `crash` (registered only when `host.adapter.journal` exists): flags
  `--at before-commit|after-commit|during-checkpoint` (default
  `after-commit`) and `--off`. Output `armed: the next change to /dev/hda
  stops ${label}` or `disarmed`. An unknown `--at` is a `ShellError` with
  help `phases: before-commit, after-commit, during-checkpoint`. Arming is not
  a recorded operation and works while rewound (with the rewound warning).
- `recover` (same condition): runs `host.run(() => host.adapter.journal!.recover())`
  and prints one line per event text; a clean journal prints its single event
  line. Errors go through `fsCall` like every mutation.
- `mkfs`: gains `--type fat16|ext2|ext3` (default: the mounted volume's own
  type, `host.vol.fsType()` lower-cased: `FAT16` is `fat16`), and its flag list is the union of the families'
  `mkfs.flags` deduplicated by `long`; a flag shared by two families merges
  descriptions as `${base} (fat16: ${restA}; ext: ${restB})` where each
  family's description is split at its first comma; for `--label` that is
  `volume label (fat16: up to 11 characters; ext: up to 16 bytes)`, the one
  sanctioned FAT expectation change. At run time a flag that
  the chosen family does not list is `ShellError("--${long} is not a ${type}
  option")`. ext's `MKFS.flags`: `--blocks` (`totalBlocks`, "total 1 KiB
  blocks (default 16384 = 16 MB)"), `--inodes-per-group`, `--label` ("volume
  label, up to 16 bytes"), `--uuid` ("32 hex digits, hyphens optional"),
  `--journal-blocks` ("journal size in blocks, ext3 only"), `--journal-mode`
  ("ordered or data, ext3 only"). `mkfs --type ext2 --journal-mode data` is
  the family's `Error` from section 1 through `fsCall`. Summary: `Format
  /dev/hda (clears the timeline); --type picks fat16, ext2, or ext3`. Done
  line composed as in section 2.
- `df` prints `blockSize`/`blocks`; `stat` spreads the ext facts; `seek`,
  `write --at`, `xxd --offset` accept `b:N` and `i:N` through `parseAddr`.
- **Re-registration**: `TerminalPanel` keeps the registered command names;
  an effect on `volume.adapter.id` unregisters them all and registers
  `createCommands(host, vfs)` again with the same `vfs` (so the working
  directory and prompt survive), then re-sets the prompt.

## 7. Lessons (all `family: "ext"`, all run on the default ext3 disk)

Numbers below are the default disk's; each lesson's test pins every quoted
number against a freshly formatted volume, as `tests/fundamentals.test.ts`
does for FAT. Steps use `fs` and, for ext-only offsets, `asExt(fs)`.

**`ext-fundamentals`, "The fundamentals (ext)".** Creates `/hello.txt`
(`Hello, ext3!`, 12 bytes) and `/bigger.txt` (13 blocks of numbered lines,
13,312 bytes) in its first step, then tours: the boot block (block 0, all
zero), the superblock (block 1: magic `0xEF53` at +0x38, 16,384 blocks, 1,024
inodes, the free counts), the group descriptors (block 2: two groups, each
descriptor naming its bitmaps and inode table), the block bitmap (block 3:
bits 1..1110 set for metadata and the journal, then the two files), the
inode bitmap (block 4: inodes 1..11 and the two files), the inode table
(blocks 5..68; inode 2 at block 5 + 0x80 is the root directory, inode 11 at
block 6 + 0x100 is lost+found), the root directory (block 69: `.`, `..`,
`lost+found`, `hello.txt`, `bigger.txt` with their `rec_len`s), hello's inode
(block 6 + 0x180: size 12, one direct pointer to block 1111), bigger's
single-indirect block (the 13th data block's pointer lives outside the
inode), the journal (blocks 82..1105, a hidden file on inode 8, with its
superblock at block 82), and group 1's backup superblock (block 8193).

**`journaled-write`, "A journaled write".** Step 1 creates `/notes.txt`
(3,000 bytes, three blocks) on the ordered-mode disk and focuses the
superblock's incompat word (block 1 + 0x60, which read 0x06 while the
transaction ran). The following steps only move the dump, in the order the
bytes were written: the data blocks (1111..1113, written before anything
touches the journal), the journal superblock (block 82 + 0x18: `s_start`
pointed at the transaction), the descriptor (block 83: one tag per metadata
block, ascending), the first copy (block 84, the superblock's image with the
flag set), the commit block (after the last copy), a checkpointed home
block (the root directory, block 69, which now names the file), the journal
superblock again (`s_start` back to 0), and the flag word (back to 0x02).
The Journal panel shows transaction 1, now stale, in the ring.

**`crash-recover`, "Crash and recover".** Step 1 arms `after_commit` and
creates `/crash.txt` (two blocks); the volume needs recovery; focus the flag
word (0x06). Step 2 focuses the live descriptor in the journal. Step 3
focuses the root directory block: no entry for the file yet. Step 4 runs
`recover()` and focuses the root directory: the entry is there. Step 5 arms
`before_commit` and creates `/lost.txt` (one block, ordered mode); focus its
data block: the bytes are on disk. Step 6 runs `recover()` (the transaction
is discarded) and focuses the block bitmap: the block is free. Step 7
focuses the same data block: the bytes are still there, owned by nothing.

`scenarios/index.ts` appends the three after the FAT lessons.

## 8. Tests

- `tests/fixtures/extGeometry.ts`: the default ext3 disk's `ExtGeometry` and
  `layout` as plain data, and `space = extSpace(geo)`.
- `tests/fs/ext.test.ts`: over real `ext.format()` volumes (ext2 and ext3):
  `fsTypes`/`familyIdOf`, `name`, the space arithmetic, owners with roles
  and `firstUnit`, no journal rows in `owners`, `chain`, `entrySlots`,
  `dataStart`, `trace` strings, `stat`, `df`, `describeUnit` strings,
  `parseAddr("i:2")`, `touchesMetadata` on a data-only change versus an inode
  change, `needsRecovery`, and the journal capability: `state()`, `blocks()`
  kinds after a create, `arm`/`phase`/`disarm`, a crash making
  `needsRecovery` true, `recover()` returning a record and clearing it;
  `ext.format({ variant: "ext2", journalMode: "data" })` throws the section-1
  error.
- `tests/fs/ext-format.test.ts`: `checkFormat` problems and
  `defaultInodesPerGroup`.
- `tests/fs/registry.test.ts`: both families, both ext types.
- `tests/attribution.test.ts`: the ext fixture (journal regions never units;
  indirect rows coloured).
- `tests/lesson.test.ts`: `describeFocus` with the ext vocabulary and the FAT
  strings unchanged.
- `tests/shell/*`: `addr.test.ts` for `b:N`, `s:N`, `i:N` and the ext help
  string; `mutations.test.ts` for `mkfs --type ext3`, `--type ext2` with a
  journal flag, a foreign flag error, and the done line for each type;
  `read-commands.test.ts` for ext `df`/`stat`; a new `journal.test.ts` for
  `crash`/`recover` output, the rewound warning, and their absence on FAT
  (`createCommands` on a FAT host has neither).
- `tests/scenarios.test.ts` and three new pinning tests
  (`ext-fundamentals.test.ts`, `journaled-write.test.ts`,
  `crash-recover.test.ts`) that run each lesson the way the runner does and
  check every number and focus against the volume.
- `tests/adapterBoundary.test.ts`: the ext-only wasm calls (`extGeometry`,
  `extSuperblock`, `blockOwners`, `inodeNumber`, `extInode`, `dirEntries`,
  `fileBlocks`, `blockGroupCount`, `journalInfo`, `journalBlocks`, `armCrash`,
  `disarmCrash`, `crashPhase`, `needsRecovery`, `recover`, `formatExt2`,
  `formatExt3`) and types (`ExtGeometry`, `ExtGroup`, `ExtSuperblock`,
  `ExtBlockOwner`, `ExtInode`, `ExtDirEntry`, `ExtFileBlocks`,
  `ExtFormatOptions`, `Ext3FormatOptions`, `JournalInfo`, `JournalBlock`) are
  allowed only under `src/fs/ext/` and in `src/lib/wasm.ts`; imports from
  `fs/ext` only from `fs/index.ts`, `fs/panels.ts`, and `scenarios/**`.
- `tests/layout.test.ts`: the guard covers the two new panels' styles.
- wasm: one wasm-pack test per new method (ext3 and FAT `NotExt`).
- Browser pass (spec Verification): format ext3 from the form and from
  `mkfs --type ext3`; add, overwrite, delete a file and a folder while
  watching the tree, ribbon, dump, map, Inspector, and Journal panel; `stat`,
  `df`, `seek i:12`, `xxd --offset b:69`; crash and recover from the panel
  and from the terminal in both phases; run the three lessons end to end;
  load the mke2fs image from the wasm README recipe; switch back to FAT16
  and confirm the FAT chrome and the FAT lessons are unchanged.

## 9. Docs

`web/ui/README.md`: "What is filesystem-specific" describes `src/fs/ext/`,
the journal capability, and the vocabulary rule; the Panes section gains the
block-group map and the Journal panel; the Terminal section gains `crash`,
`recover`, and `mkfs --type`. `crates/wasm/README.md`: the new methods and
DTOs. `docs/ROADMAP.md`: slice 4 landed; the deferred list keeps the loader
overlap check and the event-field note.

## 10. Rulings

- FAT16 stays the default volume; the ext lessons format ext3 themselves.
- One family, two types: `fsTypes` on the family; the adapter's `name` is the
  type string. This is the one registry contract change.
- The chrome's sector noun comes from the adapter; on ext the unit row
  collapses into the address row because they are the same block.
- Journal blocks are never units. Attribution keys units off `data` regions
  only, and the journal has its own region kind and colour.
- Indirect blocks are owner rows with `role: "indirect"` so the map and the
  Inspector can name them without a second table.
- `mkfs --type` chooses the family and variant; the union flag list keeps
  one command, and misuse names the type in the error.
- The terminal re-registers on family change rather than reading the
  adapter lazily, because command summaries and flag lists are captured at
  registration.
- `Unsupported` shows the wasm text: the loader's messages are already
  specific, and a fixed string would hide which feature was refused.
- The Journal panel memoises `journal.blocks()` per epoch (slice 3 recorded
  that `annotateSector` on a journal block classifies the whole journal).

## Verification

- Rust: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
  -- -D warnings`, `cargo test --workspace`, `wasm-pack test --node
  crates/wasm`, `wasm-pack build crates/wasm --target bundler`.
- Web: `pnpm install --frozen-lockfile && pnpm test && pnpm build` in
  `web/ui`, `pnpm build` in `web/demo`, with no FAT expectation edited.
- The browser pass of section 8.
