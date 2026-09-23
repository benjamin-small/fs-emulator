# FAT Explorer UI — Design

**Date:** 2026-09-21
**Status:** Approved
**Builds on:** `2026-09-21-fat16-emulator-design.md`, `2026-09-21-wasm-wrapper-design.md`

## Context

The repo now has a byte-accurate FAT16 emulator (`crates/fat` on `crates/fs-core`) and a wasm-bindgen package (`crates/wasm`, `fs-emulator-wasm`) whose `Volume` class returns plain JS objects: `OpRecord` with absolute byte changes and events, `layout()` regions, `geometry()`, `clusterOwners()`, `fatEntries()`, per-sector `annotateSectorWith()`, and `image()`. The demo under `web/demo` proves the package but shows one sector at a time and no ASCII.

The goal is the real learning UI: a whole-disk hex dump with ASCII/strings rendering so you can see where file contents land, plus the three learning experiences the user chose: operation timeline with byte-diff replay, follow-the-chain file tracing, and guided scenarios. Decisions already made with the user:

- Whole-disk virtualized scroll with collapsed zero runs and a minimap; not region-scoped windows.
- Svelte 5 + TypeScript (runes), Vite, pnpm, new app at `web/ui` alongside `web/demo` (demo stays as the package smoke test).
- ASCII gutter plus a strings overlay (printable runs of 4+, highlighted in the dump, listed with locations).
- Layout A: "disk ribbon workbench" (full-width ribbon on top, files + FAT map + actions left, hex dump center, byte inspector + strings + current step right).
- Visual direction: "engineering datasheet" (light paper canvas, ink text, IBM Plex superfamily, region colors that encode meaning, dark mode mirror).

## Design system (from the frontend-design pass)

**Subject, audience, job.** A FAT16 disk explorer for developers learning how the filesystem lays bytes out. The page's single job: make the relationship between an action ("add a file") and the bytes that change visible and traceable.

**Tokens** (`web/ui/src/styles/tokens.css`, light with a `prefers-color-scheme: dark` mirror):

| Role | Light | Dark | Notes |
|---|---|---|---|
| Canvas | `#EEF1F4` | `#14181D` | cool paper gray, not cream |
| Panel | `#FFFFFF` | `#1B2027` | |
| Ink | `#111827` | `#E6EAF0` | text |
| Ink muted | `#5B6472` | `#98A2B3` | offsets, labels |
| Hairline | `#D5DAE1` | `#2A313B` | grid rules |
| Boot / reserved | `#8B93A1` | `#6B7380` | slate |
| FAT tables | `#7F77DD` | `#AFA9EC` | violet (FAT 1 uses the 200 stop) |
| Directories | `#1D9E75` | `#5DCAA5` | teal |
| Changed bytes (current step) | `#EF9F27` fill / `#854F0B` text | `#FAC775` | amber; before-value on hover |
| Freed-but-present data | hatched over the owner hue | same | deleted remnants |
| Selection / focus | `#378ADD` | `#85B7EB` | blue outline, never a fill |
| File hues | 6-step categorical: `#5DCAA5 #F0997B #ED93B1 #85B7EB #97C459 #FAC775` | 400 stops | assigned per path, stable across renders |

**Type.** `IBM Plex Sans Condensed` 500 for panel titles and the ribbon legend; `IBM Plex Sans` 400 for body and inspector; `IBM Plex Mono` 400 for the dump, FAT map labels, offsets. Scale: 11px mono dump at 1.5 line height (16 rows per 264px), 12px captions, 13px body, 15px panel titles. Loaded from Google Fonts with `font-display: swap`; system fallbacks `ui-monospace` / `system-ui`.

**Layout.** Ribbon (28px tall, full width) → three-column grid `220px minmax(0,1fr) 260px` with 8px gutters (*2026-09-23: the right column is 340px, so Inspector annotation lines fit unwrapped*) → panels are bordered surfaces with 4px radius (this is a datasheet, not cards). Below 1100px the right column tucks under the left as tabs; below 760px the dump goes full width and side panels become a bottom sheet.

**Signature.** The disk ribbon: the entire disk as one horizontal strip, one pixel column per 32 sectors (1024 columns at 16 MiB), colored by region or owning file, with a bracket showing the current viewport and tick marks at region boundaries. Clicking scrolls the dump; hovering shows sector and owner. During replay, the ribbon flashes the columns whose bytes changed in the current step. Everything else stays quiet so this one element carries the "where did my file land" answer.

**Motion.** One deliberate animation: when a timeline step is selected, changed bytes fade in amber (150ms) and the ribbon columns pulse once. Scroll and hover are instant. `prefers-reduced-motion` disables both.

**Copy.** Sentence case, verb-first buttons ("Add file", "Export image"), errors in the status line as "Disk full. Free space or use a smaller file." (code shown as a muted suffix). Empty dump state: "Format a disk or load an image to start."

## Panes and behaviors

1. **Ribbon** (top): as above. Legend row beneath: region names and the files present, each in its hue, plus "N MB free".
2. **Files** (left, top): tree from recursive `listDir`, showing name, size, and first cluster. Selecting a file: highlights its directory entry slot in the dump, its chain in the FAT map (with arrows in chain order), and its clusters in the dump and ribbon; the inspector shows "entry → FAT chain → clusters" as three linked rows you can click through. Deleted files are not listed, but a "show remnants" toggle marks deleted directory slots and their still-present data with hatching.
3. **FAT map** (left, middle): grid of all clusters (8167 at default geometry) as 6px cells, colored free / owned (owner hue) / end-of-chain marker / bad. Hover shows cluster number, FAT value, owner. The selected file's chain draws arrows between consecutive cells.
4. **Actions** (left, bottom): path input, content textarea or file picker, buttons Add file / Overwrite / Delete / New folder / Remove folder; Format (options: total size, sectors per cluster, label) shows the cluster count those options would produce and blocks geometries outside FAT16's 4085–65524-cluster range; and Load image / Export image. Every action records a step in the timeline.
5. **Dump** (center): the virtualized whole-disk hexdump. Row = `offset | 16 hex bytes | 16-char ASCII gutter`. Bytes are tinted by owner (10–15% alpha of the hue) with a region strip in the left margin; row header changes show sector and cluster boundaries. Collapsed rows read "· · · 3,200 empty sectors (1.6 MB) · · ·" and expand on click. Header line shows offset, sector, cluster, and owner of the byte under the cursor. Keyboard: arrows move the cursor, PageUp/Down scroll, `g` jump-to-offset, `s` toggle strings, `/` focus the path input.
6. **Inspector** (right, top): "At this byte" facts plus the annotation for the sector (from `annotateSectorWith`) with the range under the cursor highlighted; for directory sectors, the slot's decoded fields; for FAT sectors, the entry and its chain.
7. **Strings** (right, middle): toggle "Highlight strings"; list of printable runs (4+) in the visible window by default, "whole disk" option with a cap, each with offset and owner; clicking scrolls to it.
8. **Step** (right, bottom; *2026-09-23: now a full-width strip under the top bar, so the right column holds Lesson, Strings, and Inspector*) and timeline scrubber: current step's op and events in plain language, changed sector list, Prev/Next/Play. Selecting a step highlights its changed bytes in amber in the dump and ribbon; hovering a changed byte shows the before value. Replay mode shows the disk as it was after that step (reconstructed by applying `before` bytes backwards from the current image; see architecture), with a banner "Viewing step 3 of 5" and a "Back to now" button.
9. **Scenarios** (top bar menu): guided walkthroughs; each step runs an action, selects what to look at, and shows a short explanation panel over the inspector with Next. Initial set: format an empty disk; add a small file; add a long-named file (LFN entries); overwrite with a larger file (chain grows); delete and see what remains; fill the disk (`DiskFull`); make a directory and put a file in it.

## Architecture

### Project structure (`web/ui`)

```
web/ui/
  index.html  package.json  vite.config.ts  svelte.config.js  tsconfig.json  vitest.config.ts
  src/main.ts                      mount App
  src/App.svelte                   ribbon + three-column grid, keyboard shortcuts, status line
  src/styles/tokens.css            design tokens above (light + dark), type scale
  src/lib/wasm.ts                  import { Volume } and types from fs-emulator-wasm
  src/core/attribution.ts          offset → region/cluster/owner/color (pure, O(1))
  src/core/zeros.ts                zero-sector bitmap, incremental rescan (pure)
  src/core/segments.ts             collapsed row model: rows/gap segments, row↔offset (pure)
  src/core/patch.ts                applyChanges(buf, changes, "forward"|"reverse") (pure)
  src/core/strings.ts              printable-run extraction with limits (pure)
  src/core/fatchain.ts             chain following from fatEntries, capped (pure)
  src/core/palette.ts              stable path → hue index (pure)
  src/core/tree.ts                 buildTree(vol, "/") via listDir recursion
  src/state/volume.svelte.ts       VolumeStore (runes): vol, image, geometry, layout, owners, fat, history, cursor, epoch
  src/state/selection.svelte.ts    selected path / byte cursor / hovered sector / strings toggle
  src/state/scenarios.svelte.ts    ScenarioRunner
  src/scenarios/*.ts               data-only scripts (format, smallFile, longName, overwriteGrows, deleteRemnants, fillDisk, directory)
  src/components/                  Ribbon, HexView, HexRow, FatMap, DirTree, ActionsPanel, Inspector, StringsPanel, StepPanel, Timeline, ScenarioPanel
  tests/*.test.ts                  Vitest over src/core
```

Toolchain replicates `web/demo/vite.config.ts` and `package.json` exactly (Vite 6, `vite-plugin-wasm`, `vite-plugin-top-level-await`, `file:../../crates/wasm/pkg`, the `@swc/core` override) and adds `svelte@^5`, `@sveltejs/vite-plugin-svelte@^5`, `svelte-check`, `vitest`. No UI, virtualization, or CSS library. Scripts: `dev`, `build` (`svelte-check` then `vite build`), `test` (`vitest run`), `preview`.

### State model

One `VolumeStore` instance. Typed arrays and wasm-returned objects live in `$state.raw` (a `$state` proxy over a 16 MiB `Uint8Array` is both wrong and slow); an `epoch` counter bumps on every byte change so components that read `image` also read `epoch`. Derived: attribution table, zero bitmap, segments, tree.

Mutations do not call `image()` again. The op's `OpRecord.changes` is applied to the cached buffer with `applyChanges`, which also yields the exact set of sectors to rescan. `image()` is called only on format, load, and export. In dev builds an invariant compares the patched buffer against `vol.image()` after each op and logs loudly on drift.

### Byte attribution

`buildAttribution(geometry, layout, owners)` produces: the region list, an `Int32Array(clusterCount + 2)` mapping cluster → owner index (`-1` free), and a parallel color-index array. `attrAtOffset(table, offset)` is arithmetic only: `sector = offset / bps`, and in the data region `cluster = 2 + floor((sector - firstDataSector) / sectorsPerCluster)` (mirrors `Geometry::cluster_of_sector` in `crates/fat/src/boot_sector.rs`). No wasm calls per byte.

### Hex view

DOM rows, windowed: a spacer sized `totalRows * ROW_H` and an absolutely positioned slice of ~60 rows, keyed `{#each}` so nodes are reused. Each byte gets a class string composed in TS (`own-N` plus `is-diff` / `is-sel` / `is-str`); CSS precedence diff > selection > string > owner. Diff and string membership use sorted interval arrays with binary search over the visible range. `annotateSectorWith(sector, owners)` runs lazily for the hovered and selected sectors only, memoized by `(sector, epoch)`. Spacer height capped at 10M px with a scale factor for the `scrollTop → row` mapping (Firefox's element-height limit); zero collapsing keeps realistic row counts in the thousands anyway.

### Zero-run collapsing

`scanZeroSectors(image, geometry)` once at load (Uint32 view, ~10 ms for 16 MiB); `rescanSectors(bits, image, geometry, sectors)` after each op for only the changed sectors. `buildSegments(bits, geometry, { minRun: 8, pinned })` turns runs of ≥ 8 zero sectors into a `gap` segment; `pinned` holds sectors containing the selection, the current step's diff, or a gap the user expanded, so those never collapse. Row↔offset via binary search over segment `firstRow`.

### Diff replay (timeline)

State at step i is reconstructed in place: from the current image, apply `before` bytes in reverse for every op after i; moving forward reapplies `after`. Exact, O(changed bytes), no extra memory. While the cursor is not at the latest step the actions panel is disabled with "Back to now". The selected step's `changes` feed the amber diff layer, its `events` the step panel, and hovering a changed byte shows `before → after`.

### FAT map

Canvas (8167 cells as DOM is where DOM stops being reasonable): fixed cell grid painted from `fatEntries(0)` plus the attribution table; hit-testing is arithmetic; the selected file's chain is a polyline overlay from `buildChain` (capped at `clusterCount` iterations).

### Ribbon

Canvas, one pixel column per 32 sectors at the default geometry, painted from the zero bitmap and attribution (free vs region vs owner hue), with a viewport bracket and region ticks. Click/drag seeks the hex view; during replay, columns intersecting the step's changes pulse.

### Directory tree and tracing

`buildTree` recurses `listDir`, memoized by `epoch`. Selecting a node sets the selected path; the inspector shows its `rawDirEntries` pair (LFN + short), the FAT map draws the chain, and the hex view pins and highlights the entry slot and every cluster. An SVG overlay draws connector lines between the entry row, the FAT map cell, and the first data row.

### Scenario engine

```ts
type Step = { text: string; action?: (v: Volume) => OpRecord | void;
              focus?: { offset?: number; sector?: number; path?: string; cluster?: number; pane?: "hex" | "fat" | "tree" };
              highlight?: Range[] };
type Scenario = { id: string; title: string; reset: "format" | "keep"; steps: Step[] };
```
`next()` runs the action through the normal mutation path (so the timeline stays coherent), then applies focus and highlight. Scripts are plain data; adding one needs no engine change.

### Strings

`findStrings(buf, start, end, minRun = 4, limit)` over the visible window by default; "whole disk" runs chunked at 1 MiB per animation frame, skipping zero runs, with a hit cap and a progress indicator. Hits feed the `is-str` layer and the side list.

### Performance budget

| Risk | Mitigation |
|---|---|
| 16 MiB copy per op | incremental `applyChanges`; `image()` only on load/format/export |
| full-disk zero scan | once at load; changed sectors only afterwards |
| proxy overhead | `$state.raw` for every typed array and wasm object |
| DOM nodes | ~60 windowed rows, keyed reuse, classes not inline styles |
| scroll height limit | zero collapsing plus a 10M px spacer cap with scaling |
| per-op wasm refreshes | `clusterOwners()` and `fatEntries(0)` only on `epoch` change, debounced during scenario playback |

### Build order

1. Scaffold `web/ui` with the demo's wasm config; render `listDir("/")` to prove the toolchain.
2. `VolumeStore` + `patch.ts` + dev invariant.
3. `attribution.ts` + tests. 4. `zeros.ts` + `segments.ts` + tests.
5. `HexView` with owner coloring, ASCII gutter, collapsed rows, jump and keyboard. 6. `Inspector` with lazy annotations.
7. `ActionsPanel` (add/overwrite/delete/mkdir/rmdir, format, load, export). 8. `Timeline` + `StepPanel` + rewind.
9. `DirTree` + chain tracing overlay. 10. `FatMap` canvas. 11. `StringsPanel`. 12. `ScenarioRunner` + seven scripts. 13. `Ribbon`. 14. Design polish pass (tokens, type, dark mode, reduced motion), responsive breakpoints. 15. CI wiring.

## Non-goals for v1

- Editing bytes directly in the dump
- FAT32 or ext2 views (the attribution and ribbon are region-driven so they can extend later)
- Persisting sessions or sharing links
- Mobile-first interaction; the responsive breakpoints keep it usable, not optimal
