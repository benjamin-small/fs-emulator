# FAT Explorer UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Svelte 5 learning UI at `web/ui`: a whole-disk virtualized hexdump with ASCII and strings, a disk ribbon, file tracing through the FAT map, an operation timeline with byte-diff replay, and guided scenarios, on top of the `fs-emulator-wasm` package.

**Architecture:** Pure TypeScript core modules (attribution, zero-run collapsing, segments, patching, strings, chains) that take plain data and are unit-tested under Vitest; two rune-based stores (`VolumeStore`, `SelectionStore`) that own the `Volume` handle and a cached image patched incrementally from `OpRecord.changes`; Svelte components that render from the stores. The `web/demo` app is untouched.

**Tech Stack:** Svelte 5 (runes), TypeScript 5, Vite 6, `@sveltejs/vite-plugin-svelte` 5, `vite-plugin-wasm`, `vite-plugin-top-level-await`, Vitest, pnpm; the local `fs-emulator-wasm` package from `crates/wasm/pkg`.

**Spec:** `docs/superpowers/specs/2026-09-21-fat-explorer-ui-design.md`

## Global Constraints

- App lives at `web/ui`; `web/demo` is not modified.
- Dependencies: `svelte ^5`, `@sveltejs/vite-plugin-svelte ^5`, `svelte-check ^4`, `vitest ^3`, `typescript ^5.6`, `vite ^6`, `vite-plugin-wasm ^3.4.1`, `vite-plugin-top-level-await ^1.4.4`, and `"fs-emulator-wasm": "file:../../crates/wasm/pkg"`; `pnpm.overrides["@swc/core"] = "1.12.14"` copied from `web/demo/package.json`. No UI, virtualization, or CSS library.
- Typed arrays and wasm-returned objects are held in `$state.raw`, never `$state`; `VolumeStore.epoch` is bumped on every byte change and read by anything that renders bytes.
- `image()` is called only on format, load, and export. Every mutation patches the cached image with `applyChanges` from the op's `changes`. In dev (`import.meta.env.DEV`) an invariant compares the patched image with `vol.image()` after each op and `console.error`s on drift.
- Numbers from the package are JS numbers; offsets are absolute disk byte offsets; `Annotation.range` is relative to its sector.
- Design tokens exactly as the spec's table, in `web/ui/src/styles/tokens.css`, with a dark mirror under `@media (prefers-color-scheme: dark)`; owner colors are CSS custom properties `--own-0` … `--own-9`; fonts IBM Plex Sans Condensed / Sans / Mono from Google Fonts with system fallbacks.
- Copy in sentence case; buttons verb-first; errors as `message` with the `code` as a muted suffix.
- Every task ends with `pnpm test` and `pnpm build` (svelte-check + Vite) passing in `web/ui`; conventional commit messages; no attribution lines.
- Components may differ from the plan's markup in detail, but every listed behavior, prop, exported function, and test is binding.

## Shared interfaces (all tasks depend on these; defined in the task that creates the file)

```ts
// src/core/attribution.ts
export type ColorIndex = number;           // 0 free, 1 boot/reserved, 2 FAT, 3 directory, 4..9 file hues
export interface Attr { regionKind: RegionKind; regionName: string; sector: number; cluster?: number; ownerPath?: string; isDir?: boolean; free: boolean; colorIndex: ColorIndex }
export interface AttributionTable { geometry: Geometry; regions: Region[]; owners: ClusterOwner[]; ownerByCluster: Int32Array; colorByCluster: Uint8Array }
export function buildAttribution(geometry: Geometry, layout: Region[], owners: ClusterOwner[]): AttributionTable
export function clusterOfSector(geometry: Geometry, sector: number): number | undefined
export function clusterByteRange(geometry: Geometry, cluster: number): { start: number; end: number }
export function attrAtSector(table: AttributionTable, sector: number): Attr
export function attrAtOffset(table: AttributionTable, offset: number): Attr

// src/core/palette.ts
export const FILE_HUE_COUNT = 6;
export function colorIndexForPath(path: string): ColorIndex   // 4 + stable hash % 6
export function cssVarForColor(index: ColorIndex): string      // `var(--own-${index})`

// src/core/patch.ts
export interface ByteChangeLike { offset: number; before: Uint8Array; after: Uint8Array }
export function applyChanges(buf: Uint8Array, changes: ByteChangeLike[], direction: "forward" | "reverse"): void
export function changedSectors(changes: ByteChangeLike[], sectorSize: number): number[]  // sorted, unique

// src/core/zeros.ts
export function scanZeroSectors(image: Uint8Array, sectorSize: number): Uint8Array       // 1 = all-zero
export function rescanSectors(bits: Uint8Array, image: Uint8Array, sectorSize: number, sectors: Iterable<number>): void

// src/core/segments.ts
export const BYTES_PER_ROW = 16;
export interface Segment { kind: "rows" | "gap"; startSector: number; sectorCount: number; firstRow: number; rowCount: number }
export function buildSegments(bits: Uint8Array, opts: { minRun: number; pinned: ReadonlySet<number>; sectorSize?: number }): Segment[]   // sectorSize defaults to 512
export function totalRows(segments: Segment[]): number
export function rowsPerSector(sectorSize: number): number
export interface RowRef { segment: Segment; rowInSegment: number }
export function rowAt(segments: Segment[], row: number): RowRef
export function rowToOffset(segments: Segment[], sectorSize: number, row: number): number   // for a gap row, the gap's first byte
export function offsetToRow(segments: Segment[], sectorSize: number, offset: number): number

// src/core/strings.ts
export interface StringHit { offset: number; length: number; text: string }
export function findStrings(buf: Uint8Array, start: number, end: number, minRun?: number, limit?: number): StringHit[]

// src/core/fatchain.ts
export type ClusterState = "free" | "used" | "end" | "bad" | "reserved";
export function clusterState(entry: FatEntry): ClusterState
export function buildChain(fat: FatEntry[], first: number): number[]   // follows next links; stops on any non-next; hard cap fat.length

// src/core/tree.ts
export interface TreeNode { name: string; path: string; isDir: boolean; size: number; firstCluster: number; children: TreeNode[] }
export function buildTree(vol: Volume, owners: ClusterOwner[]): TreeNode   // root node, path "/"

// src/state/volume.svelte.ts
export class VolumeStore {
  vol: Volume; image: Uint8Array; epoch: number; geometry: Geometry; layout: Region[]; owners: ClusterOwner[]; fat: FatEntry[]; zeros: Uint8Array;
  history: OpRecord[]; cursor: number;            // cursor = index of the step being viewed; -1 with empty history
  status: { text: string; code?: string } | null;
  readonly attribution: AttributionTable;         // $derived
  readonly sectorSize: number; readonly atLatest: boolean;
  format(options?: FormatOptions): void; load(bytes: Uint8Array): void; export(): Uint8Array;
  run(fn: (v: Volume) => OpRecord): OpRecord | null;   // false on error (status set)
  seek(step: number): void;                       // rewind/forward the cached image; no wasm calls
  backToNow(): void;
}
export const volume: VolumeStore;                 // singleton

// src/state/selection.svelte.ts
export class SelectionStore {
  path: string | null; cursorOffset: number | null; hoverOffset: number | null;
  stringsOn: boolean; showRemnants: boolean; expandedGaps: Set<number>;   // gap start sectors
  scrollTarget: { offset: number; nonce: number } | null;                   // HexView consumes
  jumpTo(offset: number): void; select(path: string | null): void;
}
export const selection: SelectionStore;
```

## File Structure

```
web/ui/index.html, package.json, pnpm-lock.yaml, vite.config.ts, svelte.config.js, tsconfig.json, vitest.config.ts
web/ui/src/main.ts, App.svelte, app.css, styles/tokens.css, lib/wasm.ts
web/ui/src/core/{attribution,palette,patch,zeros,segments,strings,fatchain,tree}.ts
web/ui/src/state/{volume,selection,scenarios}.svelte.ts
web/ui/src/components/{Ribbon,HexView,HexRow,Inspector,ActionsPanel,DirTree,Timeline,StepPanel,FatMap,StringsPanel,ScenarioPanel,StatusLine}.svelte
web/ui/src/scenarios/{index,format,smallFile,longName,overwriteGrows,deleteRemnants,fillDisk,directory}.ts
web/ui/tests/{attribution,patch,zeros,segments,strings,fatchain,integration}.test.ts
.github/workflows/ci.yml   (wasm job: add web/ui install/test/build)
.gitignore                 (web/ui/node_modules/, web/ui/dist/)
README.md                  (mention web/ui)
```

---

### Task 1: Scaffold `web/ui` and prove the toolchain

**Files:**
- Create: `web/ui/package.json`, `web/ui/vite.config.ts`, `web/ui/svelte.config.js`, `web/ui/tsconfig.json`, `web/ui/vitest.config.ts`, `web/ui/index.html`, `web/ui/src/main.ts`, `web/ui/src/App.svelte`, `web/ui/src/app.css`, `web/ui/src/styles/tokens.css`, `web/ui/src/lib/wasm.ts`, `web/ui/tests/smoke.test.ts`
- Modify: `.gitignore`, `.github/workflows/ci.yml`, `README.md`

**Interfaces:**
- Produces: `src/lib/wasm.ts` re-exporting `Volume` and every type from `fs-emulator-wasm`; the token CSS variables; the CI wiring later tasks rely on.

- [ ] **Step 1: Build the package and write the manifests**

Run `wasm-pack build crates/wasm --target bundler` first (from the repo root) so `crates/wasm/pkg` exists.

`web/ui/package.json`:
```json
{
  "name": "fs-emulator-ui",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "svelte-check --tsconfig ./tsconfig.json && vite build",
    "test": "vitest run",
    "preview": "vite preview"
  },
  "dependencies": {
    "fs-emulator-wasm": "file:../../crates/wasm/pkg"
  },
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "^5.0.0",
    "svelte": "^5.0.0",
    "svelte-check": "^4.0.0",
    "typescript": "^5.6.0",
    "vite": "^6.0.0",
    "vite-plugin-top-level-await": "^1.4.4",
    "vite-plugin-wasm": "^3.4.1",
    "vitest": "^3.0.0"
  },
  "pnpm": { "overrides": { "@swc/core": "1.12.14" } }
}
```

`web/ui/vite.config.ts`:
```ts
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

export default defineConfig({
  plugins: [svelte(), wasm(), topLevelAwait()],
  build: { target: "esnext" },
  optimizeDeps: { exclude: ["fs-emulator-wasm"] },
});
```

`web/ui/svelte.config.js`:
```js
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
export default { preprocess: vitePreprocess() };
```

`web/ui/tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "noUncheckedIndexedAccess": false,
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "skipLibCheck": true,
    "types": ["vite/client", "svelte"],
    "verbatimModuleSyntax": true,
    "isolatedModules": true
  },
  "include": ["src", "tests", "vite.config.ts", "vitest.config.ts"]
}
```

`web/ui/vitest.config.ts`:
```ts
import { defineConfig } from "vitest/config";
export default defineConfig({
  test: { environment: "node", include: ["tests/**/*.test.ts"] },
});
```

`web/ui/src/lib/wasm.ts`:
```ts
export { Volume } from "fs-emulator-wasm";
export type {
  Annotation, BootSector, ByteChange, ClusterOwner, DateTime, EntryInfo, EventRecord, FatEntry,
  FormatOptions, FsError, Geometry, OpRecord, RawEntry, Range, Region, RegionKind,
} from "fs-emulator-wasm";
```

- [ ] **Step 2: Tokens and base styles**

`web/ui/src/styles/tokens.css`:
```css
@import url("https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;500&family=IBM+Plex+Sans+Condensed:wght@500&family=IBM+Plex+Sans:wght@400;500&display=swap");

:root {
  --canvas: #eef1f4; --panel: #ffffff; --ink: #111827; --ink-muted: #5b6472; --hairline: #d5dae1;
  --focus: #378add; --diff: #ef9f27; --diff-ink: #854f0b;
  --own-0: transparent; --own-1: #8b93a1; --own-2: #7f77dd; --own-3: #1d9e75;
  --own-4: #5dcaa5; --own-5: #f0997b; --own-6: #ed93b1; --own-7: #85b7eb; --own-8: #97c459; --own-9: #fac775;
  --font-display: "IBM Plex Sans Condensed", system-ui, sans-serif;
  --font-body: "IBM Plex Sans", system-ui, sans-serif;
  --font-mono: "IBM Plex Mono", ui-monospace, SFMono-Regular, Menlo, monospace;
  --row-h: 17px; --dump-size: 11px;
  --radius: 4px;
}
@media (prefers-color-scheme: dark) {
  :root {
    --canvas: #14181d; --panel: #1b2027; --ink: #e6eaf0; --ink-muted: #98a2b3; --hairline: #2a313b;
    --focus: #85b7eb; --diff: #fac775; --diff-ink: #412402;
    --own-1: #6b7380; --own-2: #afa9ec; --own-3: #5dcaa5;
  }
}
```

`web/ui/src/app.css`:
```css
@import "./styles/tokens.css";
* { box-sizing: border-box; }
html, body { margin: 0; height: 100%; background: var(--canvas); color: var(--ink); font: 13px/1.45 var(--font-body); }
button, input, textarea, select { font: inherit; color: inherit; }
button { background: var(--panel); border: 1px solid var(--hairline); border-radius: var(--radius); padding: 4px 10px; cursor: pointer; }
button:hover { border-color: var(--ink-muted); }
button:focus-visible, input:focus-visible, textarea:focus-visible, [tabindex]:focus-visible { outline: 2px solid var(--focus); outline-offset: 1px; }
.panel { background: var(--panel); border: 1px solid var(--hairline); border-radius: var(--radius); padding: 8px 10px; min-width: 0; }
.panel > h2 { margin: 0 0 6px; font: 500 15px/1.2 var(--font-display); }
.mono { font-family: var(--font-mono); font-size: var(--dump-size); }
.muted { color: var(--ink-muted); }
@media (prefers-reduced-motion: reduce) { *, *::before, *::after { animation: none !important; transition: none !important; } }
```

- [ ] **Step 3: Minimal app proving the toolchain**

`web/ui/index.html`:
```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>FAT explorer</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

`web/ui/src/main.ts`:
```ts
import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";
export default mount(App, { target: document.getElementById("app")! });
```

`web/ui/src/App.svelte` (temporary; Task 4 replaces it):
```svelte
<script lang="ts">
  import { Volume } from "./lib/wasm";
  const vol = Volume.formatFat16(undefined);
  vol.createFile("/HELLO.TXT", new TextEncoder().encode("hello"));
  const entries = vol.listDir("/");
</script>
<main class="panel" style="margin: 16px">
  <h2>FAT explorer</h2>
  <p class="muted">{vol.fsType()} · {vol.sectorCount()} sectors</p>
  <ul class="mono">{#each entries as e}<li>{e.name} · {e.size} bytes</li>{/each}</ul>
</main>
```

`web/ui/tests/smoke.test.ts`:
```ts
import { describe, expect, it } from "vitest";
describe("toolchain", () => {
  it("runs vitest", () => { expect(1 + 1).toBe(2); });
});
```

- [ ] **Step 4: Install, test, build**

Run in `web/ui`: `pnpm install`, `pnpm test`, `pnpm build`. Expected: the smoke test passes; svelte-check reports 0 errors; Vite emits `dist/`. Then `pnpm dev` and confirm the page lists `HELLO.TXT · 5 bytes` (the controller will do this in the built-in browser if the implementer cannot).

- [ ] **Step 5: Repo wiring**

`.gitignore`: append `web/ui/node_modules/` and `web/ui/dist/`.

`.github/workflows/ci.yml`, in the `wasm` job: change the `setup-node` step's `cache-dependency-path` to `web/*/pnpm-lock.yaml`, and append after the demo's `pnpm build`:
```yaml
      - run: pnpm install --frozen-lockfile
        working-directory: web/ui
      - run: pnpm test
        working-directory: web/ui
      - run: pnpm build
        working-directory: web/ui
```

`README.md`: under "## Demo" add a "## Explorer UI" section: `web/ui` is the learning UI (Svelte 5); build the package first, then `cd web/ui && pnpm install && pnpm dev`.

- [ ] **Step 6: Commit**

```bash
git add .gitignore .github/workflows/ci.yml README.md web/ui/package.json web/ui/pnpm-lock.yaml web/ui/vite.config.ts web/ui/svelte.config.js web/ui/tsconfig.json web/ui/vitest.config.ts web/ui/index.html web/ui/src web/ui/tests
git commit -m "feat(ui): scaffold Svelte 5 explorer app with wasm toolchain and CI"
```

---

### Task 2: Pure core, part 1: patch, palette, attribution, fatchain, strings

**Files:**
- Create: `web/ui/src/core/patch.ts`, `palette.ts`, `attribution.ts`, `fatchain.ts`, `strings.ts`; `web/ui/tests/patch.test.ts`, `attribution.test.ts`, `fatchain.test.ts`, `strings.test.ts`

**Interfaces:** as in Shared interfaces. Tests use hand-built `Geometry`/`Region`/`ClusterOwner` objects (plain data), so no wasm is loaded.

- [ ] **Step 1: Failing tests**

`web/ui/tests/patch.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { applyChanges, changedSectors } from "../src/core/patch";

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
});
```

`web/ui/tests/attribution.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { attrAtOffset, attrAtSector, buildAttribution, clusterByteRange, clusterOfSector } from "../src/core/attribution";
import { colorIndexForPath } from "../src/core/palette";
import type { ClusterOwner, Geometry, Region } from "../src/lib/wasm";

export const geo: Geometry = { variant: "fat16", bytesPerSector: 512, sectorsPerCluster: 4, reservedSectors: 1, fatCount: 2, sectorsPerFat: 32, rootEntries: 512, rootDirSectors: 32, firstRootDirSector: 65, firstDataSector: 97, totalSectors: 32768, clusterCount: 8167 };
export const layout: Region[] = [
  { name: "reserved (boot sector)", sectors: { start: 0, end: 1 }, kind: "boot" },
  { name: "FAT 0", sectors: { start: 1, end: 33 }, kind: "allocationTable" },
  { name: "FAT 1", sectors: { start: 33, end: 65 }, kind: "allocationTable" },
  { name: "root directory", sectors: { start: 65, end: 97 }, kind: "directory" },
  { name: "data", sectors: { start: 97, end: 32768 }, kind: "data" },
];
const owners: ClusterOwner[] = [
  { cluster: 2, path: "/DOCS", isDir: true, firstCluster: 2 },
  { cluster: 3, path: "/DOCS/N.TXT", isDir: false, firstCluster: 3 },
  { cluster: 4, path: "/DOCS/N.TXT", isDir: false, firstCluster: 3 },
];

describe("attribution", () => {
  const t = buildAttribution(geo, layout, owners);
  it("maps sectors to clusters like the core", () => {
    expect(clusterOfSector(geo, 96)).toBeUndefined();
    expect(clusterOfSector(geo, 97)).toBe(2);
    expect(clusterOfSector(geo, 100)).toBe(2);
    expect(clusterOfSector(geo, 101)).toBe(3);
    expect(clusterOfSector(geo, 32764)).toBe(8168);
    expect(clusterOfSector(geo, 32767)).toBeUndefined(); // past the last full cluster
    expect(clusterByteRange(geo, 2)).toEqual({ start: 97 * 512, end: 97 * 512 + 2048 });
  });
  it("attributes metadata regions", () => {
    expect(attrAtSector(t, 0)).toMatchObject({ regionKind: "boot", colorIndex: 1, free: false });
    expect(attrAtSector(t, 1)).toMatchObject({ regionName: "FAT 0", colorIndex: 2 });
    expect(attrAtSector(t, 64)).toMatchObject({ regionName: "FAT 1", colorIndex: 2 });
    expect(attrAtSector(t, 65)).toMatchObject({ regionKind: "directory", colorIndex: 3 });
  });
  it("attributes data clusters to owners and free space", () => {
    expect(attrAtOffset(t, 97 * 512)).toMatchObject({ cluster: 2, ownerPath: "/DOCS", isDir: true, colorIndex: 3, free: false });
    expect(attrAtOffset(t, 101 * 512 + 7)).toMatchObject({ cluster: 3, ownerPath: "/DOCS/N.TXT", isDir: false, colorIndex: colorIndexForPath("/DOCS/N.TXT") });
    expect(attrAtOffset(t, 105 * 512)).toMatchObject({ cluster: 4, ownerPath: "/DOCS/N.TXT" });
    expect(attrAtOffset(t, 109 * 512)).toMatchObject({ cluster: 5, free: true, colorIndex: 0 });
    expect(attrAtOffset(t, 109 * 512).ownerPath).toBeUndefined();
  });
  it("file hues are stable and in range", () => {
    const i = colorIndexForPath("/A.TXT");
    expect(i).toBeGreaterThanOrEqual(4);
    expect(i).toBeLessThanOrEqual(9);
    expect(colorIndexForPath("/A.TXT")).toBe(i);
  });
});
```

`web/ui/tests/fatchain.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { buildChain, clusterState } from "../src/core/fatchain";
import type { FatEntry } from "../src/lib/wasm";

const fat: FatEntry[] = [
  { kind: "reserved" }, { kind: "reserved" },
  { kind: "next", cluster: 3 }, { kind: "next", cluster: 4 }, { kind: "endOfChain" },
  { kind: "free" }, { kind: "next", cluster: 7 }, { kind: "next", cluster: 6 }, { kind: "bad" },
];

describe("fatchain", () => {
  it("follows a chain to its end", () => { expect(buildChain(fat, 2)).toEqual([2, 3, 4]); });
  it("stops on a loop without hanging", () => { expect(buildChain(fat, 6).length).toBeLessThanOrEqual(fat.length); });
  it("returns [] for a free or invalid start", () => {
    expect(buildChain(fat, 5)).toEqual([]);
    expect(buildChain(fat, 0)).toEqual([]);
    expect(buildChain(fat, 99)).toEqual([]);
  });
  it("classifies entries", () => {
    expect(clusterState({ kind: "free" })).toBe("free");
    expect(clusterState({ kind: "next", cluster: 9 })).toBe("used");
    expect(clusterState({ kind: "endOfChain" })).toBe("end");
    expect(clusterState({ kind: "bad" })).toBe("bad");
  });
});
```

`web/ui/tests/strings.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { findStrings } from "../src/core/strings";

const bytes = (s: string) => new TextEncoder().encode(s);

describe("findStrings", () => {
  it("finds printable runs of at least minRun", () => {
    const buf = new Uint8Array([...bytes("ab"), 0, ...bytes("hello"), 0, 0, ...bytes("wor"), 1, ...bytes("READMETXT")]);
    expect(findStrings(buf, 0, buf.length)).toEqual([
      { offset: 3, length: 5, text: "hello" },
      { offset: 14, length: 9, text: "READMETXT" },
    ]);
  });
  it("respects the range, minRun, and limit", () => {
    const buf = bytes("aaaa bbbb cccc");
    expect(findStrings(buf, 5, 9)).toEqual([{ offset: 5, length: 4, text: "bbbb" }]);
    expect(findStrings(buf, 0, buf.length, 15)).toEqual([]);
    expect(findStrings(buf, 0, buf.length, 4, 1).length).toBe(1);
  });
  it("a run touching the range end is included", () => {
    const buf = bytes("xxxxyyyy");
    expect(findStrings(buf, 0, 8, 4)).toEqual([{ offset: 0, length: 8, text: "xxxxyyyy" }]);
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run in `web/ui`: `pnpm test`. Expected: module-not-found failures for the four core files.

- [ ] **Step 3: Implement**

`web/ui/src/core/patch.ts`:
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

`web/ui/src/core/palette.ts`:
```ts
export type ColorIndex = number;
export const FILE_HUE_COUNT = 6;
export const COLOR_FREE = 0, COLOR_BOOT = 1, COLOR_FAT = 2, COLOR_DIR = 3, COLOR_FILE_BASE = 4;

/** FNV-1a over the path so a file keeps its hue across renders and sessions. */
export function colorIndexForPath(path: string): ColorIndex {
  let h = 0x811c9dc5;
  for (let i = 0; i < path.length; i++) { h ^= path.charCodeAt(i); h = Math.imul(h, 0x01000193) >>> 0; }
  return COLOR_FILE_BASE + (h % FILE_HUE_COUNT);
}

export function cssVarForColor(index: ColorIndex): string { return `var(--own-${index})`; }
```

`web/ui/src/core/attribution.ts`:
```ts
import type { ClusterOwner, Geometry, Region, RegionKind } from "../lib/wasm";
import { COLOR_BOOT, COLOR_DIR, COLOR_FAT, COLOR_FREE, colorIndexForPath, type ColorIndex } from "./palette";

export type { ColorIndex };
export interface Attr {
  regionKind: RegionKind; regionName: string; sector: number;
  cluster?: number; ownerPath?: string; isDir?: boolean; free: boolean; colorIndex: ColorIndex;
}
export interface AttributionTable {
  geometry: Geometry; regions: Region[]; owners: ClusterOwner[];
  /** cluster -> index into owners, or -1 */
  ownerByCluster: Int32Array;
  colorByCluster: Uint8Array;
}

export function clusterOfSector(g: Geometry, sector: number): number | undefined {
  if (sector < g.firstDataSector) return undefined;
  const c = 2 + Math.floor((sector - g.firstDataSector) / g.sectorsPerCluster);
  return c <= g.clusterCount + 1 ? c : undefined;
}

export function clusterByteRange(g: Geometry, cluster: number): { start: number; end: number } {
  const size = g.bytesPerSector * g.sectorsPerCluster;
  const start = g.firstDataSector * g.bytesPerSector + (cluster - 2) * size;
  return { start, end: start + size };
}

export function buildAttribution(geometry: Geometry, layout: Region[], owners: ClusterOwner[]): AttributionTable {
  const n = geometry.clusterCount + 2;
  const ownerByCluster = new Int32Array(n).fill(-1);
  const colorByCluster = new Uint8Array(n);
  owners.forEach((o, i) => {
    if (o.cluster < n) {
      ownerByCluster[o.cluster] = i;
      colorByCluster[o.cluster] = o.isDir ? COLOR_DIR : colorIndexForPath(o.path);
    }
  });
  return { geometry, regions: layout, owners, ownerByCluster, colorByCluster };
}

function regionOf(regions: Region[], sector: number): Region {
  for (const r of regions) if (sector >= r.sectors.start && sector < r.sectors.end) return r;
  return { name: "unknown", sectors: { start: sector, end: sector + 1 }, kind: "other" };
}

function colorForRegion(kind: RegionKind): ColorIndex {
  switch (kind) {
    case "allocationTable": return COLOR_FAT;
    case "directory": return COLOR_DIR;
    case "data": return COLOR_FREE;
    default: return COLOR_BOOT;
  }
}

export function attrAtSector(t: AttributionTable, sector: number): Attr {
  const region = regionOf(t.regions, sector);
  const base: Attr = { regionKind: region.kind, regionName: region.name, sector, free: false, colorIndex: colorForRegion(region.kind) };
  if (region.kind !== "data") return base;
  const cluster = clusterOfSector(t.geometry, sector);
  if (cluster === undefined) return { ...base, free: true };
  const idx = t.ownerByCluster[cluster];
  if (idx < 0) return { ...base, cluster, free: true, colorIndex: COLOR_FREE };
  const o = t.owners[idx];
  return { ...base, cluster, ownerPath: o.path, isDir: o.isDir, free: false, colorIndex: t.colorByCluster[cluster] };
}

export function attrAtOffset(t: AttributionTable, offset: number): Attr {
  return attrAtSector(t, Math.floor(offset / t.geometry.bytesPerSector));
}
```

`web/ui/src/core/fatchain.ts`:
```ts
import type { FatEntry } from "../lib/wasm";

export type ClusterState = "free" | "used" | "end" | "bad" | "reserved";

export function clusterState(e: FatEntry): ClusterState {
  switch (e.kind) {
    case "free": return "free";
    case "next": return "used";
    case "endOfChain": return "end";
    case "bad": return "bad";
    default: return "reserved";
  }
}

/** Clusters in chain order starting at `first`; [] if the start is not an allocated cluster. */
export function buildChain(fat: FatEntry[], first: number): number[] {
  const out: number[] = [];
  let c = first;
  while (c >= 2 && c < fat.length && out.length < fat.length) {
    const e = fat[c];
    if (e.kind !== "next" && e.kind !== "endOfChain") break;
    out.push(c);
    if (e.kind === "endOfChain") break;
    c = e.cluster;
  }
  return out;
}
```

`web/ui/src/core/strings.ts`:
```ts
export interface StringHit { offset: number; length: number; text: string }

const isPrintable = (b: number) => b >= 0x20 && b <= 0x7e;

/** Printable ASCII runs of at least `minRun` bytes within [start, end). */
export function findStrings(buf: Uint8Array, start: number, end: number, minRun = 4, limit = 2000): StringHit[] {
  const hits: StringHit[] = [];
  const stop = Math.min(end, buf.length);
  let runStart = -1;
  const flush = (at: number) => {
    if (runStart >= 0 && at - runStart >= minRun) {
      hits.push({ offset: runStart, length: at - runStart, text: String.fromCharCode(...buf.subarray(runStart, at)) });
    }
    runStart = -1;
  };
  for (let i = Math.max(0, start); i < stop; i++) {
    if (isPrintable(buf[i])) { if (runStart < 0) runStart = i; }
    else { flush(i); if (hits.length >= limit) return hits; }
  }
  flush(stop);
  return hits.slice(0, limit);
}
```

- [ ] **Step 4: Run tests and build**

Run: `pnpm test && pnpm build`. Expected: 13 tests pass; svelte-check clean.

- [ ] **Step 5: Commit**

```bash
git add web/ui/src/core web/ui/tests
git commit -m "feat(ui): pure core for patching, attribution, chains, and strings"
```

---

### Task 3: Pure core, part 2: zero bitmap and collapsed segments

**Files:**
- Create: `web/ui/src/core/zeros.ts`, `web/ui/src/core/segments.ts`, `web/ui/tests/zeros.test.ts`, `web/ui/tests/segments.test.ts`

**Interfaces:** as in Shared interfaces.

- [ ] **Step 1: Failing tests**

`web/ui/tests/zeros.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { rescanSectors, scanZeroSectors } from "../src/core/zeros";

describe("zero sectors", () => {
  it("marks all-zero sectors and rescans only the sectors asked", () => {
    const image = new Uint8Array(512 * 6);
    image[512 * 1 + 100] = 1;
    image[512 * 4 + 511] = 0xff;
    const bits = scanZeroSectors(image, 512);
    expect(Array.from(bits)).toEqual([1, 0, 1, 1, 0, 1]);
    image[512 * 1 + 100] = 0;
    image[512 * 2] = 5;
    rescanSectors(bits, image, 512, [1]);
    expect(Array.from(bits)).toEqual([1, 1, 1, 1, 0, 1]);
    rescanSectors(bits, image, 512, [2]);
    expect(Array.from(bits)).toEqual([1, 1, 0, 1, 0, 1]);
  });
});
```

`web/ui/tests/segments.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { BYTES_PER_ROW, buildSegments, offsetToRow, rowAt, rowToOffset, rowsPerSector, totalRows } from "../src/core/segments";

const bits = (s: string) => Uint8Array.from(s, (c) => (c === "0" ? 1 : 0)); // "0" = zero sector, "x" = data
const RPS = rowsPerSector(512); // 32

describe("segments", () => {
  it("collapses runs of at least minRun, keeps shorter runs as rows", () => {
    const segs = buildSegments(bits("xx000x00000000xx"), { minRun: 4, pinned: new Set() });
    expect(segs.map((s) => [s.kind, s.startSector, s.sectorCount, s.rowCount])).toEqual([
      ["rows", 0, 6, 6 * RPS],   // xx000x (the 3-run is shorter than minRun)
      ["gap", 6, 8, 1],
      ["rows", 14, 2, 2 * RPS],
    ]);
    expect(totalRows(segs)).toBe(8 * RPS + 1);
    expect(segs[1].firstRow).toBe(6 * RPS);
  });
  it("collapses at the start and end of the disk", () => {
    const segs = buildSegments(bits("0000x0000"), { minRun: 4, pinned: new Set() });
    expect(segs.map((s) => s.kind)).toEqual(["gap", "rows", "gap"]);
  });
  it("a pinned sector splits a run and is never collapsed", () => {
    const segs = buildSegments(bits("0000000000"), { minRun: 4, pinned: new Set([5]) });
    expect(segs.map((s) => [s.kind, s.startSector, s.sectorCount])).toEqual([["gap", 0, 5], ["rows", 5, 1], ["gap", 6, 4]]);
  });
  it("maps rows to offsets and back", () => {
    const segs = buildSegments(bits("xx0000000000xx"), { minRun: 4, pinned: new Set() });
    for (const row of [0, 1, RPS, 2 * RPS - 1, 2 * RPS, 2 * RPS + 1, totalRows(segs) - 1]) {
      const off = rowToOffset(segs, 512, row);
      expect(offsetToRow(segs, 512, off)).toBe(row);
    }
    expect(rowToOffset(segs, 512, 3)).toBe(3 * BYTES_PER_ROW);
    expect(rowAt(segs, 2 * RPS).segment.kind).toBe("gap");
    expect(rowToOffset(segs, 512, 2 * RPS)).toBe(2 * 512);
    expect(offsetToRow(segs, 512, 7 * 512 + 100)).toBe(2 * RPS);        // inside the gap -> the gap row
    expect(offsetToRow(segs, 512, 12 * 512 + 16)).toBe(2 * RPS + 2);   // first row after the gap is 2*RPS+1
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm test`. Expected: module-not-found for `zeros` and `segments`.

- [ ] **Step 3: Implement**

`web/ui/src/core/zeros.ts`:
```ts
/** One byte per sector: 1 when every byte of the sector is zero. */
export function scanZeroSectors(image: Uint8Array, sectorSize: number): Uint8Array {
  const count = Math.floor(image.length / sectorSize);
  const bits = new Uint8Array(count);
  for (let s = 0; s < count; s++) bits[s] = isZero(image, sectorSize, s) ? 1 : 0;
  return bits;
}

export function rescanSectors(bits: Uint8Array, image: Uint8Array, sectorSize: number, sectors: Iterable<number>): void {
  for (const s of sectors) if (s >= 0 && s < bits.length) bits[s] = isZero(image, sectorSize, s) ? 1 : 0;
}

function isZero(image: Uint8Array, sectorSize: number, sector: number): boolean {
  const start = sector * sectorSize;
  const aligned = (image.byteOffset + start) % 4 === 0 && sectorSize % 4 === 0;
  if (aligned) {
    const words = new Uint32Array(image.buffer, image.byteOffset + start, sectorSize / 4);
    for (let i = 0; i < words.length; i++) if (words[i] !== 0) return false;
    return true;
  }
  for (let i = start; i < start + sectorSize; i++) if (image[i] !== 0) return false;
  return true;
}
```

`web/ui/src/core/segments.ts`:
```ts
export const BYTES_PER_ROW = 16;

export interface Segment {
  kind: "rows" | "gap";
  startSector: number;
  sectorCount: number;
  firstRow: number;
  rowCount: number;
}

export interface RowRef { segment: Segment; rowInSegment: number }

export function rowsPerSector(sectorSize: number): number { return sectorSize / BYTES_PER_ROW; }

/** Collapse runs of >= minRun consecutive all-zero sectors into one gap row each.
 *  Pinned sectors are always rendered as rows and split any run they fall in. */
export function buildSegments(bits: Uint8Array, opts: { minRun: number; pinned: ReadonlySet<number>; sectorSize?: number }): Segment[] {
  const rps = rowsPerSector(opts.sectorSize ?? 512);
  const n = bits.length;
  const segs: Segment[] = [];
  const runAt = (k: number) => { let j = k; while (j < n && bits[j] === 1 && !opts.pinned.has(j)) j++; return j - k; };
  const collapsibleAt = (k: number) => bits[k] === 1 && !opts.pinned.has(k) && runAt(k) >= opts.minRun;
  let i = 0, row = 0;
  while (i < n) {
    if (collapsibleAt(i)) {
      const len = runAt(i);
      segs.push({ kind: "gap", startSector: i, sectorCount: len, firstRow: row, rowCount: 1 });
      row += 1; i += len;
    } else {
      let k = i + 1;
      while (k < n && !collapsibleAt(k)) k++;
      segs.push({ kind: "rows", startSector: i, sectorCount: k - i, firstRow: row, rowCount: (k - i) * rps });
      row += (k - i) * rps; i = k;
    }
  }
  return segs;
}

export function totalRows(segments: Segment[]): number {
  const last = segments[segments.length - 1];
  return last ? last.firstRow + last.rowCount : 0;
}

/** Binary search the segment containing `row`. */
export function rowAt(segments: Segment[], row: number): RowRef {
  let lo = 0, hi = segments.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (segments[mid].firstRow <= row) lo = mid; else hi = mid - 1;
  }
  const segment = segments[lo];
  return { segment, rowInSegment: Math.min(row - segment.firstRow, segment.rowCount - 1) };
}

export function rowToOffset(segments: Segment[], sectorSize: number, row: number): number {
  const { segment, rowInSegment } = rowAt(segments, row);
  if (segment.kind === "gap") return segment.startSector * sectorSize;
  return segment.startSector * sectorSize + rowInSegment * BYTES_PER_ROW;
}

export function offsetToRow(segments: Segment[], sectorSize: number, offset: number): number {
  const sector = Math.floor(offset / sectorSize);
  let lo = 0, hi = segments.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (segments[mid].startSector <= sector) lo = mid; else hi = mid - 1;
  }
  const seg = segments[lo];
  if (seg.kind === "gap") return seg.firstRow;
  return seg.firstRow + Math.floor((offset - seg.startSector * sectorSize) / BYTES_PER_ROW);
}
```

- [ ] **Step 4: Run tests and build**

Run: `pnpm test && pnpm build`. Expected: all tests pass (the segments tests cover start/end gaps, short runs, pinned splits, and row↔offset round trips).

- [ ] **Step 5: Commit**

```bash
git add web/ui/src/core web/ui/tests
git commit -m "feat(ui): zero-sector bitmap and collapsed segment model"
```

---

### Task 4: Stores: VolumeStore, SelectionStore, tree, integration test

**Files:**
- Create: `web/ui/src/state/volume.svelte.ts`, `web/ui/src/state/selection.svelte.ts`, `web/ui/src/core/tree.ts`, `web/ui/tests/integration.test.ts`
- Modify: `web/ui/src/App.svelte` (use the store), `web/ui/vitest.config.ts` (wasm plugin for the integration test)

**Interfaces:** `VolumeStore`, `SelectionStore`, `buildTree` as in Shared interfaces. `VolumeStore.run` is the only path that mutates the disk; `seek` is the only path that rewinds.

- [ ] **Step 1: Failing integration test**

`web/ui/vitest.config.ts` becomes:
```ts
import { defineConfig } from "vitest/config";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";
export default defineConfig({
  plugins: [wasm(), topLevelAwait()],
  test: { environment: "node", include: ["tests/**/*.test.ts"], server: { deps: { inline: ["fs-emulator-wasm"] } } },
});
```

`web/ui/tests/integration.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";
import { applyChanges, changedSectors } from "../src/core/patch";
import { attrAtOffset, buildAttribution, clusterByteRange } from "../src/core/attribution";
import { buildTree } from "../src/core/tree";

describe("package integration", () => {
  it("patching the cached image from OpRecord.changes matches vol.image()", () => {
    const vol = Volume.formatFat16(undefined);
    const image = vol.image();
    const rec = vol.createFile("/Hello world.txt", new TextEncoder().encode("hello from the ui"));
    applyChanges(image, rec.changes, "forward");
    expect(Buffer.compare(Buffer.from(image), Buffer.from(vol.image()))).toBe(0);
    expect(changedSectors(rec.changes, 512)).toContain(97);
    const rec2 = vol.deleteFile("/Hello world.txt");
    applyChanges(image, rec2.changes, "forward");
    expect(Buffer.compare(Buffer.from(image), Buffer.from(vol.image()))).toBe(0);
    applyChanges(image, rec2.changes, "reverse");
    applyChanges(image, rec.changes, "reverse");
    expect(Buffer.compare(Buffer.from(image), Buffer.from(Volume.formatFat16(undefined).image()))).toBe(0);
  });
  it("attribution names the file that owns a cluster and the tree lists it", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createDir("/DOCS");
    vol.createFile("/DOCS/N.TXT", new Uint8Array(3000));
    const owners = vol.clusterOwners();
    const t = buildAttribution(vol.geometry(), vol.layout(), owners);
    const { start } = clusterByteRange(vol.geometry(), 3);
    expect(attrAtOffset(t, start)).toMatchObject({ cluster: 3, ownerPath: "/DOCS/N.TXT", isDir: false });
    const tree = buildTree(vol, owners);
    expect(tree.children[0]).toMatchObject({ name: "DOCS", path: "/DOCS", isDir: true, firstCluster: 2 });
    expect(tree.children[0].children[0]).toMatchObject({ name: "N.TXT", path: "/DOCS/N.TXT", size: 3000, firstCluster: 3 });
  });
});
```

If the bundler-target package cannot be imported under Vitest/Node even with the plugins (symptom: `import * as wasm from "./fs_emulator_wasm_bg.wasm"` unresolved), gate this file with `describe.skipIf(!process.env.UI_INTEGRATION)` and record the exact error in the report; the pure suite remains the CI gate.

- [ ] **Step 2: Run to verify failure**

Run: `pnpm test`. Expected: `tree` module not found.

- [ ] **Step 3: Implement**

`web/ui/src/core/tree.ts`:
```ts
import type { ClusterOwner, Volume } from "../lib/wasm";

export interface TreeNode { name: string; path: string; isDir: boolean; size: number; firstCluster: number; children: TreeNode[] }

/** Recursive listing; first clusters come from the owner map (0 for empty files). */
export function buildTree(vol: Volume, owners: ClusterOwner[]): TreeNode {
  const firstByPath = new Map<string, number>();
  for (const o of owners) if (!firstByPath.has(o.path)) firstByPath.set(o.path, o.firstCluster);
  const walk = (path: string): TreeNode[] =>
    vol.listDir(path).map((e) => {
      const childPath = path === "/" ? `/${e.name}` : `${path}/${e.name}`;
      return { name: e.name, path: childPath, isDir: e.isDir, size: e.size, firstCluster: firstByPath.get(childPath) ?? 0, children: e.isDir ? walk(childPath) : [] };
    });
  return { name: "/", path: "/", isDir: true, size: 0, firstCluster: 0, children: walk("/") };
}
```

`web/ui/src/state/volume.svelte.ts`:
```ts
import { Volume, type ClusterOwner, type FatEntry, type FormatOptions, type FsError, type Geometry, type OpRecord, type Region } from "../lib/wasm";
import { buildAttribution, type AttributionTable } from "../core/attribution";
import { applyChanges, changedSectors } from "../core/patch";
import { rescanSectors, scanZeroSectors } from "../core/zeros";

export class VolumeStore {
  vol = $state.raw<Volume>(Volume.formatFat16(undefined));
  image = $state.raw<Uint8Array>(new Uint8Array(0));
  epoch = $state(0);
  geometry = $state.raw<Geometry>(this.vol.geometry());
  layout = $state.raw<Region[]>(this.vol.layout());
  owners = $state.raw<ClusterOwner[]>([]);
  fat = $state.raw<FatEntry[]>([]);
  zeros = $state.raw<Uint8Array>(new Uint8Array(0));
  history = $state.raw<OpRecord[]>([]);
  cursor = $state(-1);
  status = $state<{ text: string; code?: string } | null>(null);

  attribution: AttributionTable = $derived(buildAttribution(this.geometry, this.layout, this.owners));
  sectorSize = $derived(this.geometry.bytesPerSector);
  atLatest = $derived(this.cursor === this.history.length - 1);

  constructor() { this.adopt(this.vol); }

  /** Take over a fresh Volume: full image copy, full scans, empty history. */
  private adopt(vol: Volume) {
    this.vol = vol;
    this.image = vol.image();
    this.geometry = vol.geometry();
    this.layout = vol.layout();
    this.zeros = scanZeroSectors(this.image, this.geometry.bytesPerSector);
    this.history = [];
    this.cursor = -1;
    this.refreshMeta();
    this.epoch++;
  }

  private refreshMeta() {
    this.owners = this.vol.clusterOwners();
    this.fat = this.vol.fatEntries(0);
  }

  format(options?: FormatOptions) {
    try { this.adopt(Volume.formatFat16(options)); this.status = null; } catch (e) { this.fail(e); }
  }

  load(bytes: Uint8Array) {
    try { this.adopt(Volume.fromImage(bytes)); this.status = null; } catch (e) { this.fail(e); }
  }

  export(): Uint8Array { return this.vol.image(); }

  /** Run a mutation at the latest state. Returns the record, or null on failure (status set). */
  run(fn: (v: Volume) => OpRecord): OpRecord | null {
    if (!this.atLatest) this.backToNow();
    let rec: OpRecord;
    try { rec = fn(this.vol); } catch (e) { this.fail(e); return null; }
    applyChanges(this.image, rec.changes, "forward");
    rescanSectors(this.zeros, this.image, this.sectorSize, changedSectors(rec.changes, this.sectorSize));
    this.history = [...this.history, rec];
    this.cursor = this.history.length - 1;
    this.refreshMeta();
    this.status = null;
    this.epoch++;
    if (import.meta.env.DEV) this.checkInvariant();
    return rec;
  }

  /** View the disk as it was after `step` (0-based). No wasm calls; patches the cached image. */
  seek(step: number) {
    step = Math.max(-1, Math.min(step, this.history.length - 1));
    if (step === this.cursor) return;
    const touched = new Set<number>();
    if (step < this.cursor) {
      for (let i = this.cursor; i > step; i--) { applyChanges(this.image, this.history[i].changes, "reverse"); changedSectors(this.history[i].changes, this.sectorSize).forEach((s) => touched.add(s)); }
    } else {
      for (let i = this.cursor + 1; i <= step; i++) { applyChanges(this.image, this.history[i].changes, "forward"); changedSectors(this.history[i].changes, this.sectorSize).forEach((s) => touched.add(s)); }
    }
    rescanSectors(this.zeros, this.image, this.sectorSize, touched);
    this.cursor = step;
    this.epoch++;
  }

  backToNow() { this.seek(this.history.length - 1); }

  private fail(e: unknown) {
    const err = e as Partial<FsError>;
    this.status = { text: err.message ?? String(e), code: err.code };
  }

  private checkInvariant() {
    const truth = this.vol.image();
    for (let i = 0; i < truth.length; i++) {
      if (truth[i] !== this.image[i]) { console.error(`cached image drifted from the volume at offset ${i}`); return; }
    }
  }
}

export const volume = new VolumeStore();
```

Note: while `cursor` is not at the latest step, `owners`/`fat`/`attribution` still describe the latest state (they come from the wasm side). Components that show ownership during replay use the attribution as-is; the spec accepts this because replay is about bytes, and the "Viewing step N" banner makes the mode explicit.

`web/ui/src/state/selection.svelte.ts`:
```ts
export class SelectionStore {
  path = $state<string | null>(null);
  cursorOffset = $state<number | null>(null);
  hoverOffset = $state<number | null>(null);
  stringsOn = $state(false);
  showRemnants = $state(false);
  expandedGaps = $state(new Set<number>());
  scrollTarget = $state<{ offset: number; nonce: number } | null>(null);

  jumpTo(offset: number) { this.cursorOffset = offset; this.scrollTarget = { offset, nonce: (this.scrollTarget?.nonce ?? 0) + 1 }; }
  select(path: string | null) { this.path = path; }
  expandGap(startSector: number) { const s = new Set(this.expandedGaps); s.add(startSector); this.expandedGaps = s; }
}

export const selection = new SelectionStore();
```

`web/ui/src/App.svelte` (still temporary; Task 5 lays out the real grid):
```svelte
<script lang="ts">
  import { volume } from "./state/volume.svelte";
  import { buildTree } from "./core/tree";
  const tree = $derived((volume.epoch, buildTree(volume.vol, volume.owners)));
</script>
<main class="panel" style="margin: 16px">
  <h2>FAT explorer</h2>
  <p class="muted">{volume.vol.fsType()} · {volume.geometry.totalSectors} sectors · epoch {volume.epoch}</p>
  <button onclick={() => volume.run((v) => v.createFile(`/FILE${volume.history.length}.TXT`, new TextEncoder().encode("hello")))}>Add file</button>
  <ul class="mono">{#each tree.children as n}<li>{n.name} · {n.size} bytes · cluster {n.firstCluster}</li>{/each}</ul>
  {#if volume.status}<p style="color: var(--diff-ink)">{volume.status.text} <span class="muted">{volume.status.code}</span></p>{/if}
</main>
```

- [ ] **Step 4: Run tests, build, and try the page**

Run: `pnpm test && pnpm build`, then `pnpm dev` and click Add file twice: the list grows and the dev console shows no "drifted" error.

- [ ] **Step 5: Commit**

```bash
git add web/ui
git commit -m "feat(ui): volume and selection stores with incremental image patching"
```

---

### Task 5: App layout, highlight layers, HexView, Inspector, StatusLine

**Files:**
- Create: `web/ui/src/state/layers.svelte.ts`, `web/ui/src/core/intervals.ts`, `web/ui/src/components/HexView.svelte`, `HexRow.svelte`, `Inspector.svelte`, `StatusLine.svelte`, `web/ui/tests/intervals.test.ts`
- Modify: `web/ui/src/App.svelte` (real layout with placeholders for panels built later), `web/ui/src/app.css`

**Interfaces:**
- Produces:
  ```ts
  // src/core/intervals.ts
  export interface Interval { start: number; end: number }             // [start, end)
  export function normalize(list: Interval[]): Interval[]              // sorted, merged
  export function contains(list: Interval[], offset: number): boolean  // binary search
  export function intervalsToSectors(list: Interval[], sectorSize: number): Set<number>
  // src/state/layers.svelte.ts
  export class LayersStore { diff: Interval[]; sel: Interval[]; str: Interval[]; pinnedSectors: Set<number>; visible: Interval }
  export const layers: LayersStore    // diff = current step's changes; sel = selected file's clusters (entry slot added in Task 6); str filled by Task 9; visible set by HexView
  ```
  Hex layout constants in `HexView.svelte`: `ROW_H = 17`, `OVERSCAN = 10`, `MIN_RUN = 8`, `GAP_REVEAL = 64` (an expanded gap shows its first 64 sectors as rows, the rest stays collapsed), `MAX_SPACER = 10_000_000`.

- [ ] **Step 1: Failing tests for intervals**

`web/ui/tests/intervals.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { contains, intervalsToSectors, normalize } from "../src/core/intervals";

describe("intervals", () => {
  it("normalizes overlapping and adjacent ranges", () => {
    expect(normalize([{ start: 10, end: 20 }, { start: 0, end: 5 }, { start: 15, end: 25 }, { start: 25, end: 30 }])).toEqual([{ start: 0, end: 5 }, { start: 10, end: 30 }]);
  });
  it("contains uses half-open ranges", () => {
    const l = normalize([{ start: 10, end: 20 }, { start: 40, end: 41 }]);
    expect(contains(l, 9)).toBe(false); expect(contains(l, 10)).toBe(true); expect(contains(l, 19)).toBe(true);
    expect(contains(l, 20)).toBe(false); expect(contains(l, 40)).toBe(true); expect(contains([], 0)).toBe(false);
  });
  it("maps to sectors", () => {
    expect([...intervalsToSectors([{ start: 500, end: 1030 }], 512)]).toEqual([0, 1, 2]);
  });
});
```

- [ ] **Step 2: Implement intervals and layers**

`web/ui/src/core/intervals.ts`:
```ts
export interface Interval { start: number; end: number }

export function normalize(list: Interval[]): Interval[] {
  const sorted = list.filter((i) => i.end > i.start).sort((a, b) => a.start - b.start);
  const out: Interval[] = [];
  for (const i of sorted) {
    const last = out[out.length - 1];
    if (last && i.start <= last.end) last.end = Math.max(last.end, i.end);
    else out.push({ ...i });
  }
  return out;
}

export function contains(list: Interval[], offset: number): boolean {
  let lo = 0, hi = list.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    const i = list[mid];
    if (offset < i.start) hi = mid - 1; else if (offset >= i.end) lo = mid + 1; else return true;
  }
  return false;
}

export function intervalsToSectors(list: Interval[], sectorSize: number): Set<number> {
  const s = new Set<number>();
  for (const i of list) for (let x = Math.floor(i.start / sectorSize); x <= Math.floor((i.end - 1) / sectorSize); x++) s.add(x);
  return s;
}
```

`web/ui/src/state/layers.svelte.ts`:
```ts
import { clusterByteRange } from "../core/attribution";
import { buildChain } from "../core/fatchain";
import { intervalsToSectors, normalize, type Interval } from "../core/intervals";
import { selection } from "./selection.svelte";
import { volume } from "./volume.svelte";

export class LayersStore {
  /** Extra selection ranges other stores contribute (Task 6 adds the entry slot). */
  extraSel = $state.raw<Interval[]>([]);
  str = $state.raw<Interval[]>([]);
  visible = $state.raw<Interval>({ start: 0, end: 0 });

  diff: Interval[] = $derived.by(() => {
    const rec = volume.history[volume.cursor];
    return rec ? normalize(rec.changes.map((c) => ({ start: c.offset, end: c.offset + c.after.length }))) : [];
  });

  chain: number[] = $derived.by(() => {
    const path = selection.path;
    if (!path) return [];
    const owner = volume.owners.find((o) => o.path === path);
    return owner ? buildChain(volume.fat, owner.firstCluster) : [];
  });

  sel: Interval[] = $derived(normalize([...this.chain.map((c) => clusterByteRange(volume.geometry, c)), ...this.extraSel]));

  pinnedSectors: Set<number> = $derived.by(() => {
    const s = new Set<number>([...intervalsToSectors(this.diff, volume.sectorSize), ...intervalsToSectors(this.sel, volume.sectorSize)]);
    if (selection.cursorOffset !== null) s.add(Math.floor(selection.cursorOffset / volume.sectorSize));
    for (const g of selection.expandedGaps) for (let i = 0; i < 64; i++) s.add(g + i);
    return s;
  });
}

export const layers = new LayersStore();
```

- [ ] **Step 3: HexView and HexRow**

`web/ui/src/components/HexRow.svelte`:
```svelte
<script lang="ts">
  import type { Attr } from "../core/attribution";
  interface Cell { hex: string; asc: string; cls: string; title: string }
  let { offset, attr, cells, sectorStart, clusterStart, label = "" }: { offset: number; attr: Attr; cells: Cell[]; sectorStart: boolean; clusterStart: boolean; label?: string } = $props();
  const hex8 = $derived(offset.toString(16).padStart(8, "0"));
</script>
<div class="row own-{attr.colorIndex}" class:sector-start={sectorStart} class:cluster-start={clusterStart} data-offset={offset}>
  <span class="off mono">{hex8}</span>
  <span class="hex mono">{#each cells as c, i}<span class={c.cls} data-off={offset + i} title={c.title}>{c.hex}</span>{/each}</span>
  <span class="asc mono">{#each cells as c, i}<span class={c.cls} data-off={offset + i}>{c.asc}</span>{/each}</span>
  {#if label}<span class="lbl muted">{label}</span>{/if}
</div>
```

`web/ui/src/components/HexView.svelte` — required behavior, with the core of the implementation:
```svelte
<script lang="ts">
  import { volume } from "../state/volume.svelte";
  import { selection } from "../state/selection.svelte";
  import { layers } from "../state/layers.svelte";
  import { attrAtOffset } from "../core/attribution";
  import { BYTES_PER_ROW, buildSegments, offsetToRow, rowAt, rowToOffset, totalRows, type Segment } from "../core/segments";
  import { contains } from "../core/intervals";
  import HexRow from "./HexRow.svelte";

  const ROW_H = 17, OVERSCAN = 10, MIN_RUN = 8, MAX_SPACER = 10_000_000;
  let container = $state<HTMLDivElement>();
  let scrollTop = $state(0);
  let height = $state(600);

  const segments: Segment[] = $derived((volume.epoch, buildSegments(volume.zeros, { minRun: MIN_RUN, pinned: layers.pinnedSectors, sectorSize: volume.sectorSize })));
  const rows = $derived(totalRows(segments));
  const fullHeight = $derived(rows * ROW_H);
  const spacer = $derived(Math.min(fullHeight, MAX_SPACER));
  const scale = $derived(fullHeight > 0 ? spacer / fullHeight : 1);
  const firstRow = $derived(Math.max(0, Math.floor(scrollTop / scale / ROW_H) - OVERSCAN));
  const count = $derived(Math.ceil(height / ROW_H) + 2 * OVERSCAN);
  const window = $derived.by(() => {
    const out = [];
    for (let r = firstRow; r < Math.min(rows, firstRow + count); r++) out.push(describeRow(r));
    return out;
  });

  $effect(() => { layers.visible = { start: rowToOffset(segments, volume.sectorSize, firstRow), end: rowToOffset(segments, volume.sectorSize, Math.min(rows - 1, firstRow + count)) + BYTES_PER_ROW }; });

  $effect(() => {
    const t = selection.scrollTarget;
    if (!t || !container) return;
    const row = offsetToRow(segments, volume.sectorSize, t.offset);
    container.scrollTop = Math.max(0, (row - 3) * ROW_H * scale);
  });

  function describeRow(r: number) {
    const { segment, rowInSegment } = rowAt(segments, r);
    if (segment.kind === "gap") {
      const bytes = segment.sectorCount * volume.sectorSize;
      return { kind: "gap" as const, key: r, segment, text: `· · · ${segment.sectorCount.toLocaleString()} empty sectors (${fmtBytes(bytes)}) · · ·` };
    }
    const offset = segment.startSector * volume.sectorSize + rowInSegment * BYTES_PER_ROW;
    const attr = attrAtOffset(volume.attribution, offset);
    const bytes = volume.image.subarray(offset, offset + BYTES_PER_ROW);
    const rec = volume.history[volume.cursor];
    const cells = Array.from(bytes, (b, i) => {
      const off = offset + i;
      let cls = "b";
      if (layers.str.length && contains(layers.str, off)) cls += " is-str";
      if (contains(layers.sel, off)) cls += " is-sel";
      if (contains(layers.diff, off)) cls += " is-diff";
      if (selection.cursorOffset === off) cls += " is-cur";
      let title = "";
      if (cls.includes("is-diff") && rec) { const before = beforeByte(rec, off); if (before !== null) title = `before 0x${before.toString(16).padStart(2, "0")} → after 0x${b.toString(16).padStart(2, "0")}`; }
      return { hex: b.toString(16).padStart(2, "0"), asc: b >= 0x20 && b <= 0x7e ? String.fromCharCode(b) : "·", cls, title };
    });
    const sectorStart = offset % volume.sectorSize === 0;
    const clusterStart = attr.cluster !== undefined && sectorStart && (attr.sector - volume.geometry.firstDataSector) % volume.geometry.sectorsPerCluster === 0;
    const label = clusterStart ? `cluster ${attr.cluster}${attr.ownerPath ? ` · ${attr.ownerPath}` : ""}` : sectorStart ? `sector ${attr.sector}${attr.cluster === undefined ? ` · ${attr.regionName}` : ""}` : "";
    return { kind: "row" as const, key: r, offset, attr, cells, sectorStart, clusterStart, label };
  }

  function beforeByte(rec: { changes: { offset: number; before: Uint8Array }[] }, off: number): number | null {
    for (const c of rec.changes) if (off >= c.offset && off < c.offset + c.before.length) return c.before[off - c.offset];
    return null;
  }

  function fmtBytes(n: number) { return n >= 1 << 20 ? `${(n / (1 << 20)).toFixed(1)} MB` : n >= 1024 ? `${(n / 1024).toFixed(1)} KB` : `${n} B`; }

  function onKey(e: KeyboardEvent) {
    const cur = selection.cursorOffset ?? layers.visible.start;
    const max = volume.image.length - 1;
    const move = (d: number) => { selection.jumpTo(Math.max(0, Math.min(max, cur + d))); e.preventDefault(); };
    switch (e.key) {
      case "ArrowLeft": return move(-1); case "ArrowRight": return move(1);
      case "ArrowUp": return move(-BYTES_PER_ROW); case "ArrowDown": return move(BYTES_PER_ROW);
      case "PageUp": return move(-Math.floor(height / ROW_H) * BYTES_PER_ROW); case "PageDown": return move(Math.floor(height / ROW_H) * BYTES_PER_ROW);
      case "Home": return move(-cur); case "End": return move(max - cur);
      case "g": { const v = window.prompt("Jump to offset (0x…, decimal, s:sector, c:cluster)"); if (v) jump(v); return; }
      case "s": selection.stringsOn = !selection.stringsOn; return;
    }
  }
  function jump(v: string) {
    const g = volume.geometry;
    let off: number | null = null;
    if (/^s:\d+$/i.test(v)) off = Number(v.slice(2)) * g.bytesPerSector;
    else if (/^c:\d+$/i.test(v)) off = g.firstDataSector * g.bytesPerSector + (Number(v.slice(2)) - 2) * g.bytesPerSector * g.sectorsPerCluster;
    else if (/^0x[0-9a-f]+$/i.test(v)) off = parseInt(v, 16);
    else if (/^\d+$/.test(v)) off = Number(v);
    if (off !== null && off >= 0 && off < volume.image.length) selection.jumpTo(off);
  }
  function onOver(e: MouseEvent) { const t = (e.target as HTMLElement).closest<HTMLElement>("[data-off]"); selection.hoverOffset = t ? Number(t.dataset.off) : null; }
  function onClick(e: MouseEvent) { const t = (e.target as HTMLElement).closest<HTMLElement>("[data-off]"); if (t) selection.cursorOffset = Number(t.dataset.off); }
</script>

<div class="hexview panel" bind:this={container} bind:clientHeight={height} onscroll={(e) => (scrollTop = (e.currentTarget as HTMLDivElement).scrollTop)} onmouseover={onOver} onmouseleave={() => (selection.hoverOffset = null)} onclick={onClick} onkeydown={onKey} tabindex="0" role="grid" aria-label="Disk bytes">
  {#if volume.image.length === 0}
    <p class="muted">Format a disk or load an image to start.</p>
  {:else}
    <div class="spacer" style:height="{spacer}px">
      <div class="window" style:transform="translateY({firstRow * ROW_H * scale}px)">
        {#each window as r (r.key)}
          {#if r.kind === "gap"}
            <button class="gap mono" onclick={() => selection.expandGap(r.segment.startSector)}>{r.text}</button>
          {:else}
            <HexRow offset={r.offset} attr={r.attr} cells={r.cells} sectorStart={r.sectorStart} clusterStart={r.clusterStart} label={r.label} />
          {/if}
        {/each}
      </div>
    </div>
  {/if}
</div>
```
Add to `app.css` the dump styles: `.hexview { overflow: auto; height: 100%; position: relative; padding: 0 }`, `.spacer { position: relative }`, `.window { position: absolute; left: 0; right: 0; will-change: transform }`, `.row { display: grid; grid-template-columns: 9ch 1fr 17ch auto; gap: 12px; height: var(--row-h); line-height: var(--row-h); padding-left: 10px; border-left: 3px solid transparent; white-space: pre }`, `.row.sector-start { border-top: 1px solid var(--hairline) }`, `.row.cluster-start { border-top-color: var(--ink-muted) }`, per-owner tint via `.row.own-N { border-left-color: var(--own-N) } .row.own-N .b { background: color-mix(in srgb, var(--own-N) 14%, transparent) }` for N in 1..9 (`own-0` untinted), `.hex .b { display: inline-block; width: 3ch; text-align: center }`, `.asc .b { display: inline-block; width: 1ch }`, `.is-str { text-decoration: underline dotted }`, `.is-sel { outline: 1px solid var(--focus); outline-offset: -1px }`, `.is-diff { background: var(--diff) !important; color: var(--diff-ink) }`, `.is-cur { outline: 2px solid var(--focus) }`, `.gap { display: block; width: 100%; height: var(--row-h); border: 0; background: transparent; color: var(--ink-muted); text-align: left; padding-left: 10px }`, `.lbl { font: 11px var(--font-display) }`.

When the timeline cursor changes (`volume.cursor`), rows with `is-diff` get a one-shot 150ms fade: `@keyframes diffin { from { background: transparent } }` applied by `.is-diff { animation: diffin 150ms ease-out }` (disabled by the reduced-motion rule already in `app.css`).

- [ ] **Step 4: Inspector and StatusLine**

`web/ui/src/components/Inspector.svelte`:
```svelte
<script lang="ts">
  import { volume } from "../state/volume.svelte";
  import { selection } from "../state/selection.svelte";
  import { attrAtOffset } from "../core/attribution";
  import type { Annotation } from "../lib/wasm";

  const offset = $derived(selection.hoverOffset ?? selection.cursorOffset);
  const attr = $derived(offset === null ? null : attrAtOffset(volume.attribution, offset));
  const memo = new Map<string, Annotation[]>();
  const annotations = $derived.by(() => {
    if (attr === null) return [];
    const key = `${attr.sector}:${volume.epoch}`;
    let a = memo.get(key);
    if (!a) { if (memo.size > 64) memo.clear(); a = volume.vol.annotateSectorWith(attr.sector, volume.owners); memo.set(key, a); }
    return a;
  });
  const inSector = $derived(offset === null ? -1 : offset % volume.sectorSize);
  const hex = (n: number) => "0x" + n.toString(16);
</script>
<section class="panel inspector">
  <h2>At this byte</h2>
  {#if offset === null || attr === null}
    <p class="muted">Hover or click a byte in the dump.</p>
  {:else}
    <dl class="facts">
      <dt>Offset</dt><dd class="mono">{hex(offset)} · {offset.toLocaleString()}</dd>
      <dt>Sector</dt><dd class="mono">{attr.sector} · {attr.regionName}</dd>
      {#if attr.cluster !== undefined}<dt>Cluster</dt><dd class="mono">{attr.cluster}{#if volume.fat[attr.cluster]} · FAT: {describe(volume.fat[attr.cluster])}{/if}</dd>{/if}
      {#if attr.ownerPath}<dt>Owner</dt><dd><button class="link" onclick={() => selection.select(attr.ownerPath!)}>{attr.ownerPath}</button></dd>{:else if attr.regionKind === "data"}<dt>Owner</dt><dd class="muted">free</dd>{/if}
    </dl>
    {#if !volume.atLatest}<p class="muted">Annotations describe the latest state, not the step you are viewing.</p>{/if}
    <ul class="annotations">
      {#each annotations as a}
        <li class:hit={inSector >= a.range.start && inSector < a.range.end}><span class="mono muted">{a.range.start}..{a.range.end}</span> {a.label}: <span class="mono">{a.value}</span></li>
      {/each}
    </ul>
  {/if}
</section>
```
with `describe(e: FatEntry)` returning `free | next → N | end of chain | bad | reserved`. Style: `.facts { display: grid; grid-template-columns: auto 1fr; gap: 2px 10px } .annotations { list-style: none; padding: 0; margin: 8px 0 0; max-height: 40vh; overflow: auto; font-size: 12px } .annotations li.hit { background: color-mix(in srgb, var(--focus) 15%, transparent) } .link { border: 0; padding: 0; background: none; color: var(--focus); cursor: pointer }`.

`web/ui/src/components/StatusLine.svelte`:
```svelte
<script lang="ts">
  import { volume } from "../state/volume.svelte";
</script>
<div class="status" role="status">
  {#if !volume.atLatest && volume.history.length}
    <span>Viewing step {volume.cursor + 1} of {volume.history.length}</span>
    <button onclick={() => volume.backToNow()}>Back to now</button>
  {/if}
  {#if volume.status}<span class="err">{volume.status.text}{#if volume.status.code} <span class="muted">{volume.status.code}</span>{/if}</span>{/if}
</div>
```

- [ ] **Step 5: App layout**

`web/ui/src/App.svelte`:
```svelte
<script lang="ts">
  import HexView from "./components/HexView.svelte";
  import Inspector from "./components/Inspector.svelte";
  import StatusLine from "./components/StatusLine.svelte";
</script>
<div class="app">
  <header class="topbar">
    <h1>FAT explorer</h1>
    <div id="scenario-slot"></div>
    <StatusLine />
  </header>
  <div id="ribbon-slot" class="ribbon-slot"></div>
  <div class="grid">
    <aside class="col left">
      <section class="panel"><h2>Files</h2><p class="muted">Task 6</p></section>
      <section class="panel"><h2>FAT map</h2><p class="muted">Task 8</p></section>
      <section class="panel"><h2>Actions</h2><p class="muted">Task 6</p></section>
    </aside>
    <main class="col center"><HexView /></main>
    <aside class="col right">
      <Inspector />
      <section class="panel"><h2>Strings</h2><p class="muted">Task 9</p></section>
      <section class="panel"><h2>Step</h2><p class="muted">Task 7</p></section>
    </aside>
  </div>
  <footer id="timeline-slot"></footer>
</div>
```
`app.css` layout: `.app { display: grid; grid-template-rows: auto auto minmax(0, 1fr) auto; height: 100vh; gap: 8px; padding: 8px }`, `.topbar { display: flex; align-items: center; gap: 16px } .topbar h1 { font: 500 18px var(--font-display); margin: 0 }`, `.grid { display: grid; grid-template-columns: 220px minmax(0, 1fr) 260px; gap: 8px; min-height: 0 } .col { display: flex; flex-direction: column; gap: 8px; min-height: 0; overflow: auto } .center { overflow: hidden }`, plus the breakpoints from the spec: `@media (max-width: 1100px) { .grid { grid-template-columns: 220px minmax(0,1fr) } .right { grid-column: 1; } }` and `@media (max-width: 760px) { .grid { grid-template-columns: minmax(0,1fr) } .left, .right { max-height: 40vh } }`.

- [ ] **Step 6: Verify**

`pnpm test && pnpm build`, then `pnpm dev`: the dump shows the boot sector at the top with labels "sector 0 · reserved (boot sector)", FAT sectors tinted violet, one collapsed row for the empty data region, hover fills the inspector with boot-sector annotations, and `g` then `s:65` jumps to the root directory.

- [ ] **Step 7: Commit**

```bash
git add web/ui
git commit -m "feat(ui): virtualized whole-disk hex dump with inspector and layout"
```

---

### Task 6: Actions panel, directory tree, entry-slot tracing

**Files:**
- Create: `web/ui/src/components/ActionsPanel.svelte`, `DirTree.svelte`, `TreeNodeView.svelte`, `web/ui/src/core/direntry.ts`
- Modify: `web/ui/src/App.svelte` (mount them), `web/ui/src/state/layers.svelte.ts` (entry slot into `extraSel`), `web/ui/tests/integration.test.ts` (entry-slot test)

**Interfaces:**
- Produces: `findEntrySlots(vol: Volume, geometry: Geometry, fat: FatEntry[], owners: ClusterOwner[], path: string): { start: number; end: number } | null` in `src/core/direntry.ts`: the absolute byte range covering the file's LFN entries plus its short entry (`null` for `/` or a missing path).

- [ ] **Step 1: Failing test (append to integration.test.ts)**

```ts
import { findEntrySlots } from "../src/core/direntry";
it("finds a file's directory entry slots, including LFN entries, in root and subdirectories", () => {
  const vol = Volume.formatFat16(undefined);
  vol.createFile("/My long name.txt", new Uint8Array(1));
  vol.createDir("/D");
  vol.createFile("/D/inner.txt", new Uint8Array(1));
  const g = vol.geometry(), fat = vol.fatEntries(0), owners = vol.clusterOwners();
  const root = g.firstRootDirSector * g.bytesPerSector;
  expect(findEntrySlots(vol, g, fat, owners, "/My long name.txt")).toEqual({ start: root, end: root + 3 * 32 });   // 2 LFN + short
  expect(findEntrySlots(vol, g, fat, owners, "/D")).toEqual({ start: root + 3 * 32, end: root + 4 * 32 });
  const dStart = g.firstDataSector * g.bytesPerSector; // /D is cluster 2
  expect(findEntrySlots(vol, g, fat, owners, "/D/inner.txt")).toEqual({ start: dStart + 2 * 32, end: dStart + 3 * 32 });   // after . and ..
  expect(findEntrySlots(vol, g, fat, owners, "/nope")).toBeNull();
  expect(findEntrySlots(vol, g, fat, owners, "/")).toBeNull();
});
```
(`inner.txt` uppercases to a valid short name, so it has no LFN entries; `My long name.txt` needs 2.)

- [ ] **Step 2: Implement direntry.ts**

```ts
import type { ClusterOwner, FatEntry, Geometry, RawEntry, Volume } from "../lib/wasm";
import { clusterByteRange } from "./attribution";
import { buildChain } from "./fatchain";

const ENTRY = 32;

export function findEntrySlots(vol: Volume, g: Geometry, fat: FatEntry[], owners: ClusterOwner[], path: string): { start: number; end: number } | null {
  if (path === "/") return null;
  const cut = path.lastIndexOf("/");
  const parent = cut === 0 ? "/" : path.slice(0, cut);
  const name = path.slice(cut + 1).toUpperCase();
  let entries: RawEntry[];
  try { entries = vol.rawDirEntries(parent); } catch { return null; }
  let lfnText = "", lfnStart = -1;
  for (let i = 0; i < entries.length; i++) {
    const e = entries[i];
    if (e.kind === "lfn") { if (e.isLast) { lfnText = ""; lfnStart = i; } lfnText = e.text + lfnText; continue; }
    if (e.kind === "short") {
      const matches = e.name.toUpperCase() === name || lfnText.toUpperCase() === name;
      if (matches) {
        const first = lfnStart >= 0 && lfnText ? lfnStart : i;
        return { start: slotOffset(g, fat, owners, parent, first), end: slotOffset(g, fat, owners, parent, i) + ENTRY };
      }
    }
    if (e.kind === "free") break;
    lfnText = ""; lfnStart = -1;
  }
  return null;
}

function slotOffset(g: Geometry, fat: FatEntry[], owners: ClusterOwner[], dir: string, slot: number): number {
  if (dir === "/") return g.firstRootDirSector * g.bytesPerSector + slot * ENTRY;
  const owner = owners.find((o) => o.path === dir);
  const chain = owner ? buildChain(fat, owner.firstCluster) : [];
  const perCluster = (g.bytesPerSector * g.sectorsPerCluster) / ENTRY;
  const cluster = chain[Math.floor(slot / perCluster)];
  return clusterByteRange(g, cluster).start + (slot % perCluster) * ENTRY;
}
```
Note: the LFN accumulation must reset when a short entry does not match (the `lfnText = ""` at the loop end handles the non-matching case; a deleted entry also resets).

In `layers.svelte.ts` add `entry: Interval | null = $derived(selection.path ? (volume.epoch, findEntrySlots(volume.vol, volume.geometry, volume.fat, volume.owners, selection.path)) : null)` and fold it into `sel`: `normalize([...chainRanges, ...(this.entry ? [this.entry] : []), ...this.extraSel])`.

- [ ] **Step 3: DirTree**

`DirTree.svelte` builds `tree = $derived((volume.epoch, buildTree(volume.vol, volume.owners)))` and renders `<TreeNodeView node={tree} depth={0} />` inside a panel titled "Files" with a "Show remnants" checkbox bound to `selection.showRemnants` (its effect on the dump arrives in Task 8's remnant layer; wire the checkbox now). `TreeNodeView.svelte` renders a row per node: a disclosure for directories (open by default), the name, size, and `first cluster N` (or "no data") in muted mono; clicking a row calls `selection.select(node.path)` and, if `firstCluster >= 2`, `selection.jumpTo(clusterByteRange(volume.geometry, node.firstCluster).start)`; the selected row has `aria-selected` and a focus-colored left border. Keyboard: rows are buttons.

- [ ] **Step 4: ActionsPanel**

`ActionsPanel.svelte`, panel titled "Actions", disabled entirely (with the explanation "Return to the latest step to make changes") while `!volume.atLatest`:
- Inputs: `path` (default `/Hello world.txt`), `content` textarea (default "Hello from the browser"), a file input "Use a file's bytes" that replaces the content with the file's bytes (keep a `bytes: Uint8Array | null` state; the textarea is used when null).
- Buttons calling `volume.run`: Add file → `createFile(path, data)`; Overwrite → `writeFile`; Delete → `deleteFile`; New folder → `createDir(path)`; Remove folder → `removeDir(path)`. After a successful run, `selection.select(path)` for creates/overwrites; for deletes, `selection.select(null)`.
- A `<details>` "Format" with size select (`4 MB` → 8192 sectors, `16 MB` → 32768, `64 MB` → 131072), sectors-per-cluster select (1, 2, 4, 8), label input (11 chars max) and a "Format disk" button calling `volume.format({ totalSectors, sectorsPerCluster, volumeLabel })`, then `selection.select(null)`.
- "Load image" file input → `volume.load(new Uint8Array(await file.arrayBuffer()))` inside try/catch that sets `volume.status` on read failure.
- "Export image" button: builds a Blob from `new Uint8Array(volume.export())`, creates an object URL, clicks a temporary anchor named `volume.img`, revokes the URL afterwards.

Mount both in `App.svelte`'s left column, replacing the placeholders.

- [ ] **Step 5: Verify**

`pnpm test && pnpm build`, then in the browser: add `/My long name.txt`, click it in the tree: the root directory rows for its three entries and its data cluster get the selection outline, the dump scrolls to the cluster, the inspector shows the owner. Delete it: the tree empties and the entry rows lose the outline.

- [ ] **Step 6: Commit**

```bash
git add web/ui
git commit -m "feat(ui): actions panel, directory tree, and entry-slot tracing"
```

---

### Task 7: Timeline and step panel with byte-diff replay

**Files:**
- Create: `web/ui/src/components/Timeline.svelte`, `StepPanel.svelte`
- Modify: `web/ui/src/App.svelte`

**Interfaces:** consumes `volume.history`, `volume.cursor`, `volume.seek`, `volume.backToNow`, `layers.diff`.

- [ ] **Step 1: Timeline**

`Timeline.svelte` (mounted in the footer slot, full width):
- Disabled/empty state text "No operations yet" when `history.length === 0`.
- `<input type="range" min="0" max={history.length - 1} value={cursor}>` bound so that input calls `volume.seek(Number(value))`; step labels under it show `op` names truncated; keyboard `[` and `]` (handled in `App.svelte`'s window keydown) call `seek(cursor - 1)` / `seek(cursor + 1)`.
- Buttons Prev, Next, Play/Pause: Play advances one step every 700ms until the last step, then stops; Pause stops; both respect `prefers-reduced-motion` by switching to 1500ms.
- When `cursor` changes, call `selection.jumpTo(first changed offset of the step)` so the dump scrolls to the change.

- [ ] **Step 2: StepPanel**

`StepPanel.svelte` (right column, "Step" panel): shows `Step {cursor+1} of {history.length}`, the op string in mono, the list of changed sectors (from `changedSectors(rec.changes, sectorSize)`, each a button that jumps to the sector), and the events as `<li><code>{kind}</code> {text}</li>` where an event with a `region` is a button that jumps to `region.start`. Empty state: "Run an action to see what it changes."

- [ ] **Step 3: Verify**

Add three files then delete one. Drag the slider back: the dump shows the deleted file's bytes present again, the status line shows "Viewing step 2 of 4" with "Back to now", the actions panel is disabled, the changed bytes of step 2 are amber and hovering one shows `before 0x00 → after 0x48`. Press Play: the steps advance to the end. The dev console shows no drift error after returning to now and running another action.

- [ ] **Step 4: Commit**

```bash
git add web/ui
git commit -m "feat(ui): timeline scrubber and step panel with byte-diff replay"
```

---

### Task 8: FAT map canvas and deleted remnants layer

**Files:**
- Create: `web/ui/src/components/FatMap.svelte`, `web/ui/src/core/remnants.ts`, `web/ui/tests/remnants.test.ts`
- Modify: `web/ui/src/App.svelte`, `web/ui/src/state/layers.svelte.ts`, `web/ui/src/components/HexView.svelte` + `app.css` (hatched `is-remnant` class)

**Interfaces:**
- Produces: `findRemnants(vol: Volume, geometry: Geometry, fat: FatEntry[], owners: ClusterOwner[], zeros: Uint8Array): Interval[]` in `src/core/remnants.ts`: byte ranges of (a) deleted directory slots (`RawEntry.kind === "deleted"`) in the root and every reachable directory, and (b) free clusters that are not all-zero (data left behind by deletes); `layers.remnant: Interval[]` derived from it when `selection.showRemnants` is on, else `[]`.

- [ ] **Step 1: Failing test**

`web/ui/tests/remnants.test.ts` (uses the real package like `integration.test.ts`, same skip guard):
```ts
import { describe, expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";
import { findRemnants } from "../src/core/remnants";
import { scanZeroSectors } from "../src/core/zeros";
import { clusterByteRange } from "../src/core/attribution";

describe("remnants", () => {
  it("marks deleted directory slots and non-zero free clusters", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createFile("/GONE.TXT", new TextEncoder().encode("still here"));
    vol.deleteFile("/GONE.TXT");
    const g = vol.geometry();
    const r = findRemnants(vol, g, vol.fatEntries(0), vol.clusterOwners(), scanZeroSectors(vol.image(), g.bytesPerSector));
    const root = g.firstRootDirSector * g.bytesPerSector;
    expect(r).toContainEqual({ start: root, end: root + 32 });
    expect(r).toContainEqual(clusterByteRange(g, 2));
    expect(findRemnants(Volume.formatFat16(undefined), g, vol.fatEntries(0), [], scanZeroSectors(Volume.formatFat16(undefined).image(), 512))).toEqual([]);
  });
});
```

- [ ] **Step 2: Implement remnants.ts**

Walk directories starting at `/` using `rawDirEntries`, following `short` entries with `isDir` (skip `.`/`..` names) into subdirectories; for each `deleted` slot compute its offset with the same slot math as `direntry.ts` (extract `slotOffset` into `src/core/direntry.ts` as an export and reuse it). Then for every cluster `c` in `2..clusterCount+1` with `fat[c].kind === "free"`, if any sector of the cluster has `zeros[sector] === 0`, push `clusterByteRange(g, c)`. Return `normalize(...)`.

- [ ] **Step 3: Layers and dump**

`layers.remnant = $derived(selection.showRemnants ? (volume.epoch, findRemnants(volume.vol, volume.geometry, volume.fat, volume.owners, volume.zeros)) : [])`; in `HexView.describeRow` add `is-remnant` when `contains(layers.remnant, off)`; CSS `.is-remnant { background-image: repeating-linear-gradient(45deg, transparent 0 3px, color-mix(in srgb, var(--ink-muted) 35%, transparent) 3px 4px) }`. Remnant sectors are also pinned so they never collapse while the toggle is on.

- [ ] **Step 4: FatMap**

`FatMap.svelte` ("FAT map · N clusters" panel, left column):
- A `<canvas>` sized to the panel width; `CELL = 6`, `GAP = 1`, `cols = floor(width / (CELL+GAP))`, height from `ceil(clusterCount / cols)` rows (the panel scrolls vertically if needed, max 260px).
- Paint on `$effect` over `volume.epoch`, `layers.chain`, `selection.hoverOffset`: for each cluster `c` in `2..clusterCount+1`: fill with `getComputedStyle` values of `--own-{colorByCluster[c]}` for owned clusters, `--hairline` for free, `--diff-ink` for bad, and an inner 2px dark dot for `endOfChain`. The selected chain is drawn as a polyline between cell centers in `--focus` with 2px width, and its cells get a `--focus` outline. Clusters in `layers.diff` (current step) get an amber outline.
- Hit-testing: `onmousemove` computes the cell; a caption line under the canvas shows `cluster N · state · owner`; `onclick` selects the owner path (if any) and jumps to the cluster's byte range start.
- Read the CSS variables once per paint via `getComputedStyle(canvas)` so dark mode works.

- [ ] **Step 5: Verify**

Add two files, one large enough to span several clusters (paste 5000 characters). Select it: the map shows its cells in its hue with a polyline through them; the dump outlines its clusters. Delete the first file and toggle "Show remnants": its old directory slot and cluster are hatched in the dump.

- [ ] **Step 6: Commit**

```bash
git add web/ui
git commit -m "feat(ui): FAT map canvas with chain tracing and deleted-remnant layer"
```

---

### Task 9: Strings panel and overlay

**Files:**
- Create: `web/ui/src/components/StringsPanel.svelte`
- Modify: `web/ui/src/App.svelte`, `web/ui/src/state/layers.svelte.ts`

**Interfaces:** consumes `findStrings`, `layers.visible`, `volume.zeros`; produces `layers.str`.

- [ ] **Step 1: Implement**

`StringsPanel.svelte` ("Strings" panel, right column):
- Checkbox "Highlight strings" bound to `selection.stringsOn` (keyboard `s` toggles it, already wired).
- Scope select: "Visible bytes" (default) or "Whole disk"; min length input (default 4, min 2).
- Visible scope: `hits = $derived(selection.stringsOn ? findStrings(volume.image, layers.visible.start, layers.visible.end, minRun, 500) : [])` re-evaluated on `volume.epoch` and `layers.visible`.
- Whole-disk scope: on toggle or epoch change, run a chunked scan: for each 1 MiB chunk (skipping chunks whose sectors are all zero per `volume.zeros`) schedule via `requestAnimationFrame`, accumulate hits up to 2000, show "Scanning… N%" and stop when done; cancel a running scan when scope/epoch changes.
- Set `layers.str = normalize(hits.map(h => ({ start: h.offset, end: h.offset + h.length })))` when the toggle is on, `[]` otherwise.
- List: each hit as a button `0xOFFSET · owner-or-region · "text"` (text truncated at 40 chars) that calls `selection.jumpTo(offset)`; empty state "No printable runs in this range."

- [ ] **Step 2: Verify**

Add a file with several sentences; toggle strings: the ASCII gutter and hex bytes of the text get the dotted underline and the list shows the sentence starts and directory entry names (`HELLOW~1TXT`). Switch to "Whole disk": the list includes `FAT16EMU` from the boot sector and finishes quickly because the free region is skipped.

- [ ] **Step 3: Commit**

```bash
git add web/ui
git commit -m "feat(ui): strings overlay and panel"
```

---

### Task 10: Scenario engine and guided walkthroughs

**Files:**
- Create: `web/ui/src/state/scenarios.svelte.ts`, `web/ui/src/scenarios/index.ts`, `format.ts`, `smallFile.ts`, `longName.ts`, `overwriteGrows.ts`, `deleteRemnants.ts`, `fillDisk.ts`, `directory.ts`, `web/ui/src/components/ScenarioPanel.svelte`, `web/ui/tests/scenarios.test.ts`
- Modify: `web/ui/src/App.svelte`

**Interfaces:**
```ts
// src/state/scenarios.svelte.ts
export interface StepFocus { offset?: number; sector?: number; cluster?: number; path?: string | null; showRemnants?: boolean; strings?: boolean }
export interface Step { title: string; text: string; action?: (v: Volume) => OpRecord; format?: FormatOptions; focus?: StepFocus | ((v: Volume) => StepFocus) }
export interface Scenario { id: string; title: string; summary: string; steps: Step[] }
export class ScenarioRunner { current: Scenario | null; index: number; readonly step: Step | null; start(s: Scenario): void; next(): void; prev(): void; stop(): void }
export const scenarios: ScenarioRunner
// src/scenarios/index.ts
export const all: Scenario[]
```

- [ ] **Step 1: Failing test**

`web/ui/tests/scenarios.test.ts` (real package, same skip guard): for every scenario in `all`, run its steps against a fresh `Volume` outside the UI (apply `format` when present, call `action` when present, resolve `focus` when it is a function) and assert none throws except steps whose `title` starts with "Expect:" (the disk-full step), which must throw with `code === "DiskFull"`. Also assert every scenario has at least 3 steps and non-empty `text`.

- [ ] **Step 2: Runner**

`ScenarioRunner.start(s)`: `volume.format()` (every scenario begins from a clean default disk unless its first step has `format`), set `current`, `index = -1`, then `next()`. `next()`: advance `index`; if the step has `format`, `volume.format(step.format)`; if `action`, `volume.run(step.action)` (a thrown `DiskFull` is expected on the "Expect:" step and surfaces through `volume.status`, which the panel shows as the lesson); then apply `focus`: `path` → `selection.select`, `offset`/`sector`/`cluster` → `selection.jumpTo`, `showRemnants`/`strings` → the selection flags. `prev()`: `volume.seek(volume.cursor - 1)`-style replay is not used; instead prev just moves `index` back and re-applies that step's focus (the disk stays as is), because re-running actions backwards would desync history. `stop()` clears `current`.

- [ ] **Step 3: Scripts** (each file exports one `Scenario`; texts are the lesson, 1–3 sentences, sentence case)

- `format.ts` "Format an empty disk": steps: (1) "The boot sector" focus sector 0: explain BPB fields and that the inspector decodes them; (2) "Two copies of the FAT" focus sector 1: entries 0 and 1 are reserved, everything else free; (3) "The root directory" focus sector `firstRootDirSector`: 512 fixed slots of 32 bytes; (4) "Free space collapses" focus first data sector: the dump folds thousands of empty sectors into one row.
- `smallFile.ts` "Add a small file": (1) action `createFile("/HELLO.TXT", "Hello, FAT16!")` focus path: three things changed: a directory entry, a FAT entry, a data cluster; (2) focus the entry slot (offset from `findEntrySlots`): name, size, first cluster live here; (3) focus sector 1: the FAT entry for cluster 2 is now end-of-chain; (4) focus cluster 2: the bytes themselves; the rest of the cluster stays zero.
- `longName.ts` "A long file name": (1) action `createFile("/Quarterly report (draft).txt", ...)`; (2) focus entry slot: LFN entries come first, backwards, each holding 13 characters, followed by the short `QUARTE~1.TXT` entry; (3) explain the checksum byte that ties them together (inspector annotation).
- `overwriteGrows.ts` "Overwrite with a bigger file": (1) create a 1-cluster file; (2) `writeFile` with 3 clusters of text; focus path: the chain now spans three clusters, visible as a polyline in the FAT map; (3) focus FAT sector: entries point cluster to cluster; (4) create a second file, then overwrite the first with 5 clusters: the chain skips the second file's cluster, which is fragmentation.
- `deleteRemnants.ts` "Delete and see what remains": (1) create a file with a memorable sentence; (2) `deleteFile`; focus `showRemnants: true` and the entry slot: only the first byte of the entry changed to `E5`, the name is still readable; (3) focus cluster 2: the data is untouched and the FAT entry is free, which is why undelete tools work; (4) create another file: it reuses the slot and cluster, overwriting the remnant.
- `fillDisk.ts` "Fill the disk": format with `{ totalSectors: 8192, sectorsPerCluster: 1, enforceFat16Range: true }` in step 1; (2) action creating a file of `(clusterCount - 1) * 512` bytes; (3) "Expect: disk full" action creating a 2-cluster file: the error names `DiskFull` and the timeline shows no new step because failed operations are rolled back.
- `directory.ts` "Directories are files too": (1) `createDir("/DOCS")` focus path: a directory is a cluster holding `.` and `..` entries; (2) focus cluster of `/DOCS`: the dot entries in the dump; (3) `createFile("/DOCS/NOTE.TXT", ...)` focus the entry slot inside the directory cluster; (4) `removeDir("/DOCS")` fails with `DirectoryNotEmpty` ("Expect: not empty"); (5) delete the file, then remove the directory.

- [ ] **Step 4: ScenarioPanel**

Mounted in the topbar slot: a "Scenarios" `<select>` listing `all` with a "Start" button; while a scenario runs, a panel overlays the right column's top (`position: absolute` within the right column, `panel` style with a `--focus` border): title, `Step {i+1} of {n}`, the step title in display font, the text, and Prev / Next / Close buttons; Next on the last step reads "Finish". Keyboard: `n` and `p` while a scenario is active.

- [ ] **Step 5: Verify**

`pnpm test` (the scenarios test runs every script) and run each scenario in the browser end to end; the fill-disk scenario shows the `DiskFull` message in the status line and the panel text explains it.

- [ ] **Step 6: Commit**

```bash
git add web/ui
git commit -m "feat(ui): scenario engine and seven guided walkthroughs"
```

---

### Task 11: Disk ribbon, polish, and responsive pass

**Files:**
- Create: `web/ui/src/components/Ribbon.svelte`, `web/ui/README.md`
- Modify: `web/ui/src/App.svelte`, `web/ui/src/app.css`, `README.md`

- [ ] **Step 1: Ribbon**

`Ribbon.svelte` (mounted in the ribbon slot, full width, 28px canvas plus a legend row):
- Columns: `cols = canvas width in CSS px`; each column covers `ceil(totalSectors / cols)` sectors. Column color: if any sector in the span is non-zero data owned by a file, that file's hue; else if it lies in a metadata region, that region's color (`--own-1/2/3`); else canvas hairline for free space. Owned-but-zero sectors still take the owner hue (ownership, not content, is the point).
- Overlay: region boundary ticks (1px `--ink-muted`) at each `layout` region start; the viewport bracket (2px `--focus` top and bottom lines) spanning the columns covered by `layers.visible`; during replay (`volume.cursor` change) the columns intersecting `layers.diff` flash `--diff` for 300ms (skipped under reduced motion).
- Interaction: click or drag → `selection.jumpTo(column start sector * sectorSize)`; hover updates a caption to the right of the legend: `sector N · region or owner`.
- Legend row beneath: `boot · FAT 0 · FAT 1 · root` swatches, then each file in the tree with its hue swatch (click selects it), then `N MB free` computed from free clusters × cluster size.
- Repaint on `volume.epoch`, `layers.visible`, `layers.diff`, and resize (`ResizeObserver`).

- [ ] **Step 2: Polish**

- Panel headings use `--font-display`; dump offsets and gutters use `--font-mono`; verify the type scale from the spec (11px dump, 12px captions, 13px body, 15px panel titles).
- Dark mode check: every color comes from tokens; the canvases read tokens via `getComputedStyle`.
- Reduced motion: the diff fade and ribbon flash are off; Play interval slows.
- Breakpoints from the spec (1100px: right column tucks under the left; 760px: single column with side panels limited to 40vh).
- Keyboard: `App.svelte` window keydown handles `[`/`]` (timeline), `n`/`p` (scenario), `/` focuses the path input; the dump handles its own keys when focused. Focus rings visible on every control.
- Empty and error copy per the spec.

- [ ] **Step 3: Docs**

`web/ui/README.md`: what the explorer is, the panes, keyboard shortcuts, `pnpm dev/test/build`, and the prerequisite `wasm-pack build crates/wasm --target bundler`. Root `README.md`: the "Explorer UI" section links to it.

- [ ] **Step 4: Verify (the plan's full manual pass)**

`pnpm test && pnpm build`; then in the browser: add a long-named file and confirm the ribbon shows it in its hue at the first data column, the dump colors its cluster and the ASCII gutter reads the text, the inspector decodes the LFN + short entry, the FAT map shows the chain; delete it and step back on the timeline to see the bytes return and the ribbon flash; overwrite with a larger file and watch the chain grow; run each scenario; toggle strings; switch the OS to dark mode and reduced motion; resize to 1000px and 700px wide.

- [ ] **Step 5: Commit**

```bash
git add web/ui README.md
git commit -m "feat(ui): disk ribbon, design polish, responsive layout, docs"
```
