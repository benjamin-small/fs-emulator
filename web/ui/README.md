# FAT explorer UI

A Svelte 5 (runes) app for exploring a FAT16 volume byte by byte: a
whole-disk hex dump with ASCII and a strings overlay, a disk ribbon and FAT
cluster map for seeing where files land, an operation timeline with
byte-diff replay, eight guided scenarios, and a terminal drawer that mounts
the volume at `/mnt` and the raw disk at `/dev/hda`. It runs entirely in the
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
pnpm test      # vitest over src/core and src/shell (loads the real wasm package)
pnpm build     # svelte-check, then vite build (the CI gate)
pnpm preview   # serve the production build
```

## Panes

Layout: a disk ribbon runs full width above a three-column grid — files,
FAT map, and actions on the left; the hex dump in the center; byte
inspector, strings, and the current step on the right — with an operation
timeline along the bottom.

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
- **Actions** — add, overwrite, or delete a file; make or remove a
  folder; format a fresh disk with a chosen size and cluster size; load or
  export a raw image. Every action records a step in the timeline.
- **Dump** — the virtualized whole-disk hex view: offset, 16 hex bytes,
  and a 16-character ASCII gutter per row. Bytes are tinted by owner, runs
  of zero sectors collapse into a single clickable row, and the header
  changes at sector and cluster boundaries.
- **Inspector** — facts about the byte under the cursor (offset, sector,
  cluster, owner) plus the sector's decoded annotations — directory
  entries, FAT chain, boot sector fields — with the byte's range
  highlighted.
- **Strings** — printable runs (4+ bytes) in the visible window, or the
  whole disk on request, each with its offset and owner; click one to jump
  to it.
- **Step / Timeline** — the current operation's plain-language events and
  changed sectors, with Prev/Next/Play scrubbing through history. Replay
  reconstructs the disk as it was after any step and highlights what that
  step changed, in amber, in both the dump and the ribbon. Running a command
  never moves the dump: it highlights the changed bytes and leaves the view
  where you left it. Any explicit navigation does move it — a timeline step
  (a step button, Prev/Next/Play, the slider, `[`/`]`), the ribbon, a tree
  node or a FAT-map cluster, an inspector, strings, or step-panel link, the
  dump's own keys, or `seek`.
- **Learning scenarios** (top bar) — guided walkthroughs: format an empty
  disk, add a small file, add a long-named file (LFN entries), overwrite
  with a larger file (chain grows), delete and see what remains, fill the
  disk, make a directory, and work from the shell (the same operations typed
  as commands, plus a raw-sector read and a raw patch of the volume label).
  Pick one in the top bar and press **Start**; it formats a fresh disk and a
  **Lesson** card appears at the top of the right column, above Step. The
  card holds the scenario title, the step number, the step's title and text,
  a "Look at:" line naming what the step pointed the UI at (a file, a
  cluster, a sector, an offset and its region, the remnant hatching, the
  strings overlay), and **Prev** / **Next** (**Finish** on the last step) /
  **Close**; `n` and `p` do Next and Prev from anywhere outside a text
  field. It is an ordinary panel: nothing is covered or dimmed, every other
  pane stays live while a lesson runs, and the card's title takes focus on
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
that is not a text field) opens a drawer along the bottom running a shell
from `@benjamin-small/browser-terminal`, pinned at 0.3.0. The volume is
mounted at `/mnt` and the raw disk is `/dev/hda`; `/dev/zero` and `/dev/null`
exist too. Every write is an ordinary journaled operation, so it lands in
the timeline, the dump, the ribbon, and the tree exactly like a form action,
and it rewinds the same way. The drawer bar can be dragged to resize
(120px to 60% of the window; the height persists), `Escape` on the bar,
the Close button, or `exit` close it, and `help` or `<command> --help`
describe every command.

The prompt shows the working directory: the shell hands it to the terminal on
startup and after every `cd` or `mkfs`, so it reads `/mnt/DOCS ❯`.

| Command | Does |
|---|---|
| `ls [path] [-l]` / `dir` | List a directory as a table of name, type, size (`-l` adds the modified time); entries keep their on-disk order. `ls /` shows `dev` and `mnt`; `ls /dev` shows `hda`, `zero`, `null` |
| `cd [path]`, `pwd` | Change or print the working directory; `.` and `..` resolve client-side, `cd` alone returns to `/mnt`, and the stored path takes the on-disk case |
| `cat <path> [--bytes]` | Print a file as UTF-8 text, or as raw bytes for pipes; refuses files over 1 MiB either way (use `dd`) and warns on binary content |
| `write <path> [--append] [--at <addr>]` | Write the piped input, creating or overwriting the file (`echo hi \| write /mnt/A.TXT`). `--append` reads, concatenates, and rewrites; `write /dev/hda --at <addr>` patches the disk |
| `dd if=<src> of=<dst> bs=N count=N skip=N seek=N` | Copy bytes between files and the raw disk, in the classic operand form; `--if=<src>` and the rest work as flags too. At most 1 MiB per invocation; `/dev/zero` needs `count=` |
| `xxd [path] [--offset --len --cols]` / `hexdump` | Hex dump of a path, piped bytes, or piped text; on `/dev/hda` one sector at absolute addresses that match the dump |
| `mkdir`, `rmdir`, `rm`, `touch`, `cp` | The usual; `cp` into an existing directory keeps the source name |
| `stat <path>` | Name, type, size, timestamps, first cluster, chain, entry offset, FAT entry offset, data offset; `stat /dev/hda` reports the sector size and count |
| `df`, `mount` | Cluster usage; device, mount point, type, and `ok` or `corrupt` |
| `seek <addr>` | Move the hex dump (`0x200`, `512`, `s:1`, `c:2`) |
| `select [path]` | Select a file in every pane, or clear the selection |
| `mkfs [--sectors --spc --label --root-entries --fats --reserved]` | Format a fresh disk; the timeline is cleared |
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
  `dd`, and `xxd` all check this the same way).
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

## What is FAT-specific

The app is the FAT explorer today, but most of it does not know what FAT is.
The hex dump, ribbon, byte attribution, zero-run collapsing, strings overlay,
timeline, and diff replay read `layout()` regions and the operation journal
from `fs-emulator-wasm`, so a FAT32 or ext2 volume gets all of them
unchanged. The FAT-specific pieces are the FAT map, the entry → chain → data
trace in the inspector, the cluster labels in the dump, the Format panel's
geometry fields, and the scenarios. Adding a filesystem means new region
kinds and colors in `src/core/attribution.ts`, a map panel and inspector
section for its structures, and scenarios that teach what is different about
it. `docs/ROADMAP.md` has the checklist.

## Keyboard shortcuts

| Key | Effect |
|---|---|
| `[` / `]` | Step the timeline back / forward |
| `n` / `p` | Next / previous step on the Lesson card (while a lesson is running) |
| `/` | Focus the path field in Actions |
| Arrow keys | Move the dump's byte cursor, or seek one ribbon column (ribbon focused) |
| `Page Up` / `Page Down` | Scroll the dump by a page |
| `Home` / `End` | Jump to the start / end of the disk |
| `g` | Jump to an offset, sector, or cluster (dump focused) |
| `s` | Toggle string highlighting (dump focused) |
| `` ` `` | Toggle the terminal drawer |

All shortcuts except the dump's own (which need the dump focused) work from
anywhere that isn't a text field, so typing a path or file content in
Actions is never hijacked. The terminal counts as a text field: keys typed
into it, including `[`, `]`, `/`, `n`, `p`, and the backtick, never reach the
app's shortcuts, so close the drawer with `exit`, the Close button, or
`Escape` on its bar.

Respects `prefers-color-scheme` for light/dark and `prefers-reduced-motion`
(the diff fade, the ribbon's flash, and the timeline's Play speed all back
off).
