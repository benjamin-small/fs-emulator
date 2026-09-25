# fs explorer UI

A Svelte 5 (runes) app for exploring a filesystem byte by byte, FAT16, ext2,
and ext3: a whole-disk hex dump with ASCII and a strings overlay, a disk
ribbon and a FAT cluster map or ext block-group map for seeing where files
land, an ext3 journal panel with crash and recovery controls, an operation
timeline with byte-diff replay, twelve guided scenarios, and a terminal
drawer that mounts the volume at `/mnt` and the raw disk at `/dev/hda`. The
app opens on a FAT16 disk. It runs entirely in the
browser against the `fs-emulator-wasm` package — nothing is sent over the
network, and there is no server component. The build from `main` is
published at https://benjamin-small.github.io/fs-emulator/ by
`.github/workflows/pages.yml`, which sets `VITE_BASE=/fs-emulator/` so
asset URLs resolve under the repository path.

## Prerequisite

Build the wasm package first:

```
wasm-pack build crates/wasm --target bundler
```

Then, from `web/ui`:

```
pnpm install
pnpm dev       # dev server with HMR
pnpm test      # vitest over src/core, src/fs and src/shell (loads the real wasm package)
pnpm coverage  # the same suite with a local V8 coverage report
pnpm build     # svelte-check, then vite build (the CI gate)
pnpm preview   # serve the production build
```

## Panes

Layout: the current step runs as a strip under the top bar, next to the
other controls; a disk ribbon runs full width above a three-column grid —
files, the mounted family's map (the FAT map, or the block-group map and, on
ext, the Journal panel under it), and actions on the left; the hex dump in
the center; the lesson card, strings, and the byte inspector on the right —
with an operation timeline along the bottom.

- **Ribbon** — the entire disk as one strip, one column per pixel of its
  width, colored by owning file or region (or a hairline for free space),
  with region-boundary ticks and a bracket showing the dump's current
  viewport. Click or drag to seek; with the ribbon focused, the arrow keys
  move one column at a time and `Home`/`End` jump to the ends of the disk.
  Hovering shows the sector and its owner. During replay, the columns whose bytes changed flash briefly. The
  legend below lists the boot/FAT/root regions and every file, each in its
  hue (click a swatch to select it), plus free space.
- **Files** — the directory tree. Selecting an entry highlights its
  directory slot, its FAT chain, and its clusters everywhere else in the
  UI. A "Show remnants" toggle marks deleted entries and the data they left
  behind.
- **FAT map** — every cluster as a small cell: free, owned (in its
  file's hue), end-of-chain, or bad. Hover for the cluster number, FAT
  value, and owner; the selected file's chain draws as connected arrows.
- **Block groups** (ext, in place of the FAT map) — one band per block
  group headed `group 0 · blocks 1–8192 · 7,082 free`, every block a 4 px
  cell: metadata in its region's colour, the journal in amber, owned data and
  directory blocks in their file's hue, free blocks as a hairline, and a dot
  on an indirect block. The selected file's blocks are outlined, the last
  step's changes outlined in the diff colour, and the block under the dump's
  hovered byte dashed. Hover for `block N · region · owner`; click to select
  the owner and jump to the block.
- **Journal** (ext, under the block-group map) — on ext3 the journal's mode
  and header facts (sequence, head, start, size), a ring strip with one cell
  per journal block coloured by kind (superblock, descriptor, copy, commit,
  revoke, unused) with checkpointed transactions faded, and the crash
  controls: pick a phase (before commit, after commit, during checkpoint),
  **Arm** it so the next change stops there, and **Recover**, enabled while
  the volume needs recovery, which replays a committed transaction or
  discards an uncommitted one as a timeline step. While recovery is needed
  the panel and the status line say so, and changes throw `NeedsRecovery`.
  On ext2 the panel is one line: "This volume has no journal."
- **Actions** — add, overwrite, or delete a file; make or remove a
  folder; format a fresh disk (a Filesystem select picks FAT16, with a size
  and cluster size, or ext, with ext2 or ext3, a size, inodes per group, a
  label, and for ext3 the journal's mode and size); load or export a raw
  image. Every action records a step in the timeline.
- **Dump** — the virtualized whole-disk hex view: offset, 16 hex bytes,
  and a 16-character ASCII gutter per row. Bytes are tinted by owner, runs
  of zero sectors collapse into a single clickable row, and the header
  changes at sector and cluster boundaries.
- **Inspector** — facts about the byte under the cursor (offset, sector,
  cluster, owner) plus the sector's decoded annotations — directory
  entries, FAT chain, boot sector fields — with the byte's range
  highlighted. On ext the address row is `Block`, with the block's own line
  (`data block 0 of /hello.txt`, `single-indirect block of /bigger.txt`)
  folded into it, since a block is both the sector and the unit; the
  annotations decode the superblock, the group descriptors, the bitmaps,
  inodes, directory entries, pointer blocks, and journal blocks, and the
  selected file's trace runs from its directory entry through its inode and
  pointer blocks to its first data block. Offsets and integer fields are in
  hex to match the dump; hover one for the decimal.
- **Strings** — printable runs (4+ bytes) in the visible window, or the
  whole disk on request, each with its offset and owner; click one to jump
  to it.
- **Step** (strip under the top bar) **/ Timeline** (bottom) — the current
  operation's plain-language events and changed sectors, with Prev/Next/Play
  scrubbing through history. Replay
  reconstructs the disk as it was after any step and highlights what that
  step changed, in amber, in both the dump and the ribbon. Running a command
  never moves the dump: it highlights the changed bytes and leaves the view
  where you left it. Any explicit navigation does move it — a timeline step
  (a step button, Prev/Next/Play, the slider, `[`/`]`), the ribbon, a tree
  node or a FAT-map cluster, an inspector, strings, or step-panel link, the
  dump's own keys, or `seek`.
- **Learning scenarios** (top bar) — guided walkthroughs: the fundamentals
  (a tour of the regions, the allocation table, the root directory, and how
  a file's slot, chain, and clusters link together, on a disk that already
  holds a small file and a three-cluster one), format an empty
  disk, add a small file, add a long-named file (LFN entries), overwrite
  with a larger file (chain grows), delete and see what remains, fill the
  disk, make a directory, and work from the shell (the same operations typed
  as commands, plus a raw-sector read and a raw patch of the volume label);
  and, on ext3, the fundamentals (ext) (a tour of the block groups, the
  superblock, descriptors, bitmaps, and inode table, and how a name reaches
  its blocks through an inode, including a single-indirect block), a
  journaled write (one create followed through the needs-recovery flag, the
  data, the descriptor, the copies, the commit, and the checkpoint), and
  crash and recover (a crash after the commit that recovery replays and one
  before it that recovery discards, leaving bytes nobody owns). The picker
  groups them by filesystem, FAT16 first.
  Pick one in the top bar and press **Start**; it formats a fresh disk of the
  lesson's filesystem (the default FAT16 or ext3 disk) and a
  **Lesson** card floats over the page, at the top right to begin with. Drag
  it by its bar to wherever it is out of the way of the bytes it talks about,
  or focus the ⋮⋮ handle and use the arrow keys (Shift for bigger steps); it
  remembers where you left it, and `Escape` inside it closes it. The − / +
  button in its bar minimizes it to just the bar and the Prev / Next / Close
  row (still floating and movable) and expands it again; that is remembered
  too. Under 760px it docks to the bottom of the screen instead. The
  card holds the scenario title, the step number, the step's title and text,
  a "Look at:" line naming what the step pointed the UI at (a file, a
  cluster, a sector, an offset and its region, the remnant hatching, the
  strings overlay), and **Prev** / **Next** (**Finish** on the last step) /
  **Close**; `n` and `p` do Next and Prev from anywhere outside a text
  field. It is a non-modal dialog: nothing is dimmed or made inert, every
  other pane stays live while a lesson runs, and the card's title takes focus on
  each step so a keyboard or screen-reader user lands on the new text. Each
  step runs
  at most once: **Next** runs a step the first time you reach it, but once
  it has run, Prev and Next replay it through the timeline — the disk is
  rewound or fast-forwarded to the state that step left behind, so stepping
  back and forth never repeats an operation or adds duplicate timeline
  entries. A step that formats the disk throws the old history away, so
  **Prev** stops there rather than rewinding past it.

## Terminal

The **Terminal** button in the top bar (or the backtick key, from anywhere
that is not a text field) opens the terminal, running a shell
from `@benjamin-small/browser-terminal`, pinned at 0.3.0. The volume is
mounted at `/mnt` and the raw disk is `/dev/hda`; `/dev/zero` and `/dev/null`
exist too. Every write is an ordinary journaled operation, so it lands in
the timeline, the dump, the ribbon, and the tree exactly like a form action,
and it rewinds the same way. On viewports 1600px and wider the terminal is
a column down the right side, resized by dragging its left edge (320px to
half the window); narrower viewports get a drawer along the bottom, resized
by dragging its bar (120px to 60% of the window). Both sizes persist.
`Escape` on the bar, the Close button, or `exit` close it, and `help` or `<command> --help`
describe every command.

The prompt shows the working directory: the shell hands it to the terminal on
startup and after every `cd` or `mkfs`, so it reads `/mnt/DOCS ❯`.

The command set follows the mounted volume: when a format, a load, or a
lesson changes the family (or swaps ext3 for ext2), the terminal re-registers
its commands. The address help follows the new family (`s:65 (sector), c:3
(cluster)` on FAT, `b:65 (block), i:11 (inode)` on ext), so does `df`'s
summary (cluster or block usage), and `crash` and `recover` exist only while
the volume has a journal. `mkfs`'s flags do not change: every host lists
every family's. The new commands share the old ones' working-directory
state, and the prompt is set again.

| Command | Does |
|---|---|
| `ls [path] [-l]` / `dir` | List a directory as a table of name, type, size (`-l` adds the modified time); entries keep their on-disk order. `ls /` shows `dev` and `mnt`; `ls /dev` shows `hda`, `zero`, `null` |
| `cd [path]`, `pwd` | Change or print the working directory; `.` and `..` resolve client-side, `cd` alone returns to `/mnt`, and the stored path takes the on-disk case |
| `cat <path> [--bytes]` | Print a file as UTF-8 text, or as raw bytes for pipes; refuses files over 1 MiB either way (use `dd`) and warns on binary content |
| `write <path> [--append] [--at <addr>]` | Write the piped input, creating or overwriting the file (`echo hi \| write /mnt/A.TXT`). `--append` reads, concatenates, and rewrites; `write /dev/hda --at <addr>` patches the disk |
| `dd if=<src> of=<dst> bs=N count=N skip=N seek=N` | Copy bytes between files and the raw disk, in the classic operand form; `--if=<src>` and the rest work as flags too. At most 1 MiB per invocation; `/dev/zero` needs `count=` |
| `xxd [path] [--offset --len --cols]` / `hexdump` | Hex dump of a path, piped bytes, or piped text; on `/dev/hda` one sector at absolute addresses that match the dump |
| `mkdir`, `rmdir`, `rm`, `touch`, `cp` | The usual; `cp` into an existing directory keeps the source name |
| `stat <path>` | Name, type, size, timestamps, then the family's facts: on FAT first cluster, chain, entry offset, FAT entry offset, data offset; on ext inode, inode offset, mode, links, 512-byte blocks, data blocks, indirect blocks, directory-entry offset. `stat /dev/hda` reports the sector size and count |
| `df`, `mount` | Cluster usage on FAT, block usage (`blockSize`, `blocks`) on ext; device, mount point, type, and `ok` or `corrupt` |
| `seek <addr>` | Move the hex dump (`0x200`, `512`, `s:1`, `c:2` on FAT; `b:69` for a block and `i:12` for an inode's slot on ext) |
| `select [path]` | Select a file in every pane, or clear the selection |
| `mkfs [--type fat16\|ext2\|ext3] [flags]` | Format a fresh disk; the timeline is cleared. `--type` defaults to the mounted volume's type; FAT16 takes `--sectors --spc --label --root-entries --fats --reserved`, ext `--blocks --inodes-per-group --label --uuid --journal-blocks --journal-mode` (the last two ext3 only); a flag the type does not take is an error that names the type |
| `crash [--at before-commit\|after-commit\|during-checkpoint] [--off]` | ext3 only: arm a crash so the next change to `/dev/hda` stops at that phase (default `after-commit`), or disarm it |
| `recover` | ext3 only: replay the journal's committed transaction or discard an uncommitted one, as one timeline step, printing its events |
| `exit` | Close the drawer |

`>`, `>>`, and `<` resolve through the same tree:

```
echo hi > /mnt/A.TXT            # create, or overwrite
cat /mnt/A.TXT >> /mnt/LOG.TXT  # read, concatenate, rewrite the whole file
str upcase < /mnt/A.TXT         # feed a file to the first command
```

A `>` or `>>` is the same journaled write `write` performs — one timeline
step, the file selected afterwards — and `<` reads the file as UTF-8 text
under the same 1 MiB cap as `cat`. `/dev/null` discards, and `/dev/hda` and
`/dev/zero` are refused in both directions: a redirect cannot say *where* on
the disk to put the bytes, so use `dd of=/dev/hda seek=<blocks>` to write and
`dd if=/dev/hda | xxd` to read.

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
dd if=/dev/hda bs=512 count=1 | xxd
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
/dev/hda`, and `dd` keep working. Outside the terminal, the Files panel shows
"Boot sector does not parse; the tree is unavailable until a raw write
repairs it." and the status line shows "Volume not mounted: …" with the same
message; `mount` reports `corrupt` rather than `ok`. The wipe is one
journaled step, so **Prev** on the timeline shows the disk as it was; writing
a parsable boot sector back to `/dev/hda` (or `mkfs`) clears the corruption.

Things to know:

- Quotes are only needed for a value with a space in it
  (`dd 'of=/mnt/MY FILE.BIN'`); `dd if=/dev/hda count=1` needs none.
- Strings cross pipes as UTF-8 text; binary data crosses as raw bytes, which
  the terminal shows as `<N bytes>`, `length` counts, and `to json` encodes as
  hex. `cat --bytes`, `dd`, `xxd`, and `write` all speak them — so
  `cat --bytes /mnt/A.TXT | xxd` prints what `<13 bytes>` was hiding. `echo a b`
  produces a list, which `write` joins with one space.
- A command with nothing piped into it receives an empty list from
  browser-terminal, not `null`; the shell treats both as "no input" (`write`,
  `dd`, and `xxd` all check this the same way). A redirect does not: `>` and
  `>>` always write, so an empty pipeline leaves an empty file where `write`
  would have refused with "nothing to write".
- While the timeline is rewound, reads show the latest state and print one
  warning each; any write snaps the timeline back to now first. A `<` redirect
  reads the latest state too, but silently: a redirect hook has no channel to
  warn on.
- `dd` and `cat` refuse more than 1 MiB per invocation: every byte is
  journaled twice in Rust and again in the UI's history.
- The working directory is one value per page, not per shell session: the
  prompt prefix is engine-wide, so two sessions could not show different
  directories anyway. Anything that replaces the volume — `mkfs`, the Actions
  panel's Format, starting a learning scenario, or loading a raw image —
  returns the shell to `/mnt`, since the directory it was in no longer exists.
- `echo` is the shell's own builtin and behaves as in browser-terminal.
- The terminal and its wasm load on first open, so the initial page load is
  unchanged.

## What is filesystem-specific

The app is the fs explorer: one explorer for every filesystem family the wasm
package can hold, FAT16 and ext (ext2 and ext3) today. The hex dump, ribbon, byte attribution,
zero-run collapsing, strings overlay, timeline, diff replay, and terminal read
`layout()` regions and the operation journal from `fs-emulator-wasm`, so a
second family gets all of them unchanged. Everything the UI knows about one
family lives behind the adapter in `src/fs/`:

- `src/fs/adapter.ts` declares the seam. `UnitSpace` is pure allocation-unit
  arithmetic over one geometry: the unit noun, plural, and shell letter
  (`cluster`, `clusters`, `c` for FAT), the unit count and size, sector to
  unit and unit to byte range, the dump's row-label rule, and the region
  colours. `FsAdapter` is bound to one `Volume` and adds what needs the disk:
  owners, chains, entry slots, remnants, `stat` and `df` facts, the ribbon's
  free count (`freeUnits`), the Inspector's trace, sector annotations, extra address forms, name matching,
  the "this write may have moved regions" trigger, and the notes the tree and
  the shell print. Its caches are plain fields that the store refreshes after
  every operation, so a `$derived` that calls an adapter method reads
  `volume.epoch` first.
- `src/fs/fat16/` is the FAT16 implementation: `Fat16Adapter`, the cluster
  geometry, the FAT chain walk, the directory-entry and remnant scans, the
  boot-sector trigger, the Format model (`clusterCountFor`, `checkFormat`,
  the `mkfs` flags), `FatMap.svelte`, and `FormatForm.svelte`. It is the only
  place, besides the type facade `src/lib/wasm.ts`, that calls the FAT-only
  wasm methods (`geometry`, `fatEntries`, `clusterOwners`,
  `annotateSectorWith`, `rawDirEntries`, `bootSector`, `clusterChain`,
  `formatFat16`) or names their types.
- `src/fs/ext/` is the ext implementation, one adapter for ext2 and ext3:
  `ExtAdapter` over the ext-only DTOs (`extGeometry`, `extSuperblock`,
  `blockOwners`, `inodeNumber`, `extInode`, `dirEntries`, `fileBlocks`), the
  block geometry (`extSpace`), the Format model (`checkFormat`, the size and
  journal defaults, the `mkfs` flags), `ExtJournal`, the metadata trigger,
  `BlockGroupMap.svelte`, `JournalPanel.svelte`, and `ExtFormatForm.svelte`.
  Journal blocks are never allocation units (the journal has its own region
  kind and amber colour; its pointer blocks show as owned by `<journal>`),
  and a file's indirect blocks are owner rows with `role: "indirect"`, so the
  map, the dump, and the Inspector name them without a second table. It is
  the only place, besides `src/lib/wasm.ts`, that calls the ext-only wasm
  methods (those seven, the journal's `journalInfo`, `journalBlocks`,
  `armCrash`, `disarmCrash`, `crashPhase`, `needsRecovery`, and `recover`,
  and `formatExt2`/`formatExt3`) or names their types.
- The **journal capability** is the one optional part of the seam:
  `FsAdapter.journal?: JournalCapability` (`state`, `blocks`, `arm`,
  `disarm`, `phase`, `recover`), present on ext3 and absent on FAT16 and
  ext2, with `FsAdapter.needsRecovery` beside it. Its types live in
  `fs/adapter.ts`, so the Journal panel, the status line, and the `crash` and
  `recover` commands depend on the capability, never on the ext family, and
  appear whenever the mounted adapter has one. `recover()` returns the
  volume's op, which the caller runs through the timeline like any change.
- **Vocabulary:** every noun the chrome prints comes from the adapter. The
  sector noun (`UnitSpace.sector`) names one disk sector and the unit noun
  (`UnitSpace.unit`) the allocation unit: FAT says sector and cluster
  (`s:65`, `c:2`), ext says block for both, because its disk sector is the
  block (`b:69`, plus `i:12` for an inode), and where the two nouns coincide
  the chrome shows one name (the Inspector folds the unit row into the
  address row, and the dump's `g` prompt names one noun). The Lesson card's
  file clause is `unit.fileParts` ("its entry, chain, and clusters" on FAT,
  "its inode, block map, and blocks" on ext).
- `src/fs/index.ts` is the registry: `FAMILIES` (`fat16`, then `ext`),
  `DEFAULT_FAMILY` (`fat16`), `familyIdOf(fsType)`, which returns the family
  whose `fsTypes` lists the string (`FAT16`; `ext2` and `ext3`), and
  `adapterFor(vol)`. `src/fs/panels.ts` maps a family id to its map and
  Format panels and its `extras` (the Journal panel for ext), and `App.svelte`
  and `ActionsPanel.svelte` render whichever the mounted family names. The
  panels sit in that table rather than on the adapter because the node tests
  have no Svelte plugin and the adapter must stay importable from them.
- Each scenario declares its `family`; `start()` formats that family's
  default disk. A step reaches generic facts through the adapter it is
  handed and family-only ones through `asFat16(fs)` or `asExt(fs)`.

`tests/adapterBoundary.test.ts` keeps it that way: it scans
`src/**/*.{ts,svelte}` and fails on a FAT-only wasm call or type outside
`src/fs/fat16/` and `src/lib/wasm.ts`, on an ext-only one outside
`src/fs/ext/` and `src/lib/wasm.ts`, and on an import from `fs/fat16` or
`fs/ext` anywhere but `src/fs/index.ts`, `src/fs/panels.ts`, and
`src/scenarios/`. Adding a family means extending `FsFamilyId` in
`fs/adapter.ts`, an adapter under `src/fs/<family>/`, an entry in
`fs/index.ts` and `fs/panels.ts`, a map panel in its own words ("FAT map",
"Block groups"), and scenarios that teach what is different about it.
`docs/ROADMAP.md` has the checklist.

## Keyboard shortcuts

| Key | Effect |
|---|---|
| `[` / `]` | Step the timeline back / forward |
| `n` / `p` | Next / previous step on the Lesson card (while a lesson is running) |
| `Escape` (inside the Lesson card) | Close the lesson |
| `/` | Focus the path field in Actions |
| Arrow keys | Move the dump's byte cursor, or seek one ribbon column (ribbon focused) |
| `Page Up` / `Page Down` | Scroll the dump by a page |
| `Home` / `End` | Jump to the start / end of the disk |
| `g` | Jump to an offset, sector, or cluster, or on ext an offset or block (dump focused) |
| `s` | Toggle string highlighting (dump focused) |
| `` ` `` | Toggle the terminal |

All shortcuts except the dump's own (which need the dump focused) work from
anywhere that isn't a text field, so typing a path or file content in
Actions is never hijacked. The terminal counts as a text field: keys typed
into it, including `[`, `]`, `/`, `n`, `p`, and the backtick, never reach the
app's shortcuts, so close the drawer with `exit`, the Close button, or
`Escape` on its bar.

Dark by default, whatever the OS prefers: the **Dark mode** switch at the
right of the top bar flips to light, remembers the choice across reloads, and
the terminal follows. Respects `prefers-reduced-motion` (the diff fade, the
ribbon's flash, and the timeline's Play speed all back off).
