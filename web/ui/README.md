# FAT explorer UI

A Svelte 5 (runes) app for exploring a FAT16 volume byte by byte: a
whole-disk hex dump with ASCII and a strings overlay, a disk ribbon and FAT
cluster map for seeing where files land, an operation timeline with
byte-diff replay, and seven guided scenarios. It runs entirely in the
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
pnpm test      # vitest over src/core
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
  step changed, in amber, in both the dump and the ribbon.
- **Scenarios** (top bar) — guided walkthroughs: format an empty disk, add
  a small file, add a long-named file (LFN entries), overwrite with a
  larger file (chain grows), delete and see what remains, fill the disk,
  and make a directory. Starting one formats a fresh disk. Each step runs
  at most once: **Next** runs a step the first time you reach it, but once
  it has run, Prev and Next replay it through the timeline — the disk is
  rewound or fast-forwarded to the state that step left behind, so stepping
  back and forth never repeats an operation or adds duplicate timeline
  entries. A step that formats the disk throws the old history away, so
  **Prev** stops there rather than rewinding past it.

## Keyboard shortcuts

| Key | Effect |
|---|---|
| `[` / `]` | Step the timeline back / forward |
| `n` / `p` | Next / previous scenario step (while a scenario is running) |
| `/` | Focus the path field in Actions |
| Arrow keys | Move the dump's byte cursor, or seek one ribbon column (ribbon focused) |
| `Page Up` / `Page Down` | Scroll the dump by a page |
| `Home` / `End` | Jump to the start / end of the disk |
| `g` | Jump to an offset, sector, or cluster (dump focused) |
| `s` | Toggle string highlighting (dump focused) |

All shortcuts except the dump's own (which need the dump focused) work from
anywhere that isn't a text field, so typing a path or file content in
Actions is never hijacked.

Respects `prefers-color-scheme` for light/dark and `prefers-reduced-motion`
(the diff fade, the ribbon's flash, and the timeline's Play speed all back
off).
