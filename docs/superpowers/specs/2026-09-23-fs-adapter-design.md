# fs explorer: the filesystem adapter seam

Design for slice 1 of the ext work: rename the explorer and move every
filesystem-specific assumption in the UI behind one adapter interface, so a
second family (ext2 layout plus an ext3 journal) can plug in without forking
the explorer. FAT16 behaviour is unchanged by construction.

## Why

The user wants a second filesystem family with the same explorer experience
FAT16 has, and wants the naming reframed first. Exploration on 2026-09-23
found:

- `fs-core` is filesystem-agnostic where it matters (`FileSystem`, `Region`,
  `Annotation`, the byte journal). Its small leaks (the FAT epoch as the
  default `DateTime`, `RegionKind::AllocationTable`, `DirectoryFull`,
  backslash as a path separator, `run_op` private to `FatFs`) do not block
  this slice and are left to the ext crate's spec.
- The wasm `Volume` is `Inner::Fat(FatFs)`; `fromImage` always parses FAT;
  `corruption()` sits in the FAT-only impl block although the gate is a
  trait-level concept; the documented `NotFat` code is never produced.
- The UI is FAT all the way down: the store constructs `formatFat16` and
  calls `geometry()`, `clusterOwners()`, `fatEntries()` unguarded; byte
  ownership, the map, the chain trace, the tree meta, `c:N` addresses,
  `stat`, `df`, `mkfs`, the Format form, lesson text, and all nine scenarios
  do cluster arithmetic on the FAT `Geometry`, at about twenty sites.

## Decisions (2026-09-23, with the user)

1. The ext work is four sub-projects, each with its own spec and plan, in
   this order: **(1) this slice**; (2) `crates/ext` with the ext2 on-disk
   layout, verified by e2fsprogs; (3) the JBD journal on top (ext3) with an
   explicit crash point and recovery; (4) the parallel explorer experience
   for ext (block-group map, inode inspector, journal panel, scenarios,
   terminal).
2. The explorer is renamed **"fs explorer"**. Filesystem-specific panels
   keep their own words: "FAT map" stays; ext will get "Block groups".
3. Per-filesystem knowledge in the UI lives behind a **TypeScript adapter
   interface, one implementation per `fsType()` family** (`fat16` now,
   `ext3` later). Wasm keeps honest family-specific methods and gains only
   the truly generic pieces.

Naming clash to keep straight: the repo already calls its per-operation byte
record "the journal" (`OpRecord`, `ByteChange`). The ext3 JBD journal is a
different thing and slice 3 must name it distinctly.

## Outcome

- FAT behaviour, every shell output string, every lesson string, and every
  attribution colour are unchanged.
- Every FAT assumption in `web/ui/src` outside `src/fs/fat16/` is gone, and a
  source-scan test keeps it gone.
- The app says "fs explorer".
- The wasm crate exposes `corruption()` generically and names its
  image-detection rule.

## Non-goals

- No ext code. Nothing is added "for ext" except the interface shape.
- No visible filesystem picker: one family exists. The seam is
  `volume.format(family, options)` and `host.format(family, options)`.
- `mkfs --type` and re-registering terminal commands when the family changes
  belong to the ext slice.
- `web/demo` stays FAT-specific; it is the wasm smoke test.
- The fs-core leaks listed above stay as they are.

## 1. The adapter (`web/ui/src/fs/adapter.ts`)

Two layers. `UnitSpace` is pure arithmetic over one geometry and can be
built from a fixture without a `Volume`. `FsAdapter` is bound to one
`Volume` and caches owners, tables, and geometry as plain fields that
`refresh()` re-reads.

```ts
import type { Volume, Region, RegionKind, Annotation } from "../lib/wasm";
import type { Interval } from "../core/intervals";
import type { ColorIndex } from "../core/palette";
import type { ByteChangeLike } from "../core/patch";

export type FsFamilyId = "fat16";                       // ext adds "ext3"

/** The allocation-unit noun: how the UI, shell, and lessons name a data unit. */
export interface UnitVocab {
  singular: string;   // "cluster"
  plural: string;     // "clusters"
  letter: string;     // "c": the shell address prefix (c:N)
  first: number;      // first valid unit index (FAT: 2)
}

/** One unit -> the path that owns it. Generic mirror of wasm's ClusterOwner. */
export interface UnitOwner { unit: number; path: string; isDir: boolean; firstUnit: number }

/** Pure unit arithmetic over one geometry. Buildable without a Volume. */
export interface UnitSpace {
  readonly unit: UnitVocab;
  readonly unitCount: number;                           // FAT: clusterCount
  readonly unitSize: number;                            // bytes per unit
  readonly sectorSize: number;                          // vol.sectorSize()
  readonly totalSectors: number;                        // vol.sectorCount()
  unitOfSector(sector: number): number | undefined;     // undefined outside the data area
  unitByteRange(unit: number): Interval;                // [start, end) bytes; no bounds check, as today
  unitOfOffset(offset: number): number | undefined;
  unitStartsAt(sector: number): boolean;                // the dump's row-label rule
  colorForRegion(region: Region): ColorIndex;           // FAT: "FAT 1" gets COLOR_TABLE_ALT
}

/** One row of the Inspector's "Selected file" trace. */
export interface TraceRow { label: string; offset: number | null }   // null = muted text, no jump
/** stat facts the shell prints as key/value after the generic ones. Keys are the family's. */
export type StatFacts = Record<string, string | number | number[]>;
export interface DfFacts { unitSize: number; units: number; used: number; free: number }

export interface MkfsFlag { long: string; desc: string; kind: "int" | "str"; option: string }
export interface MkfsSpec { summary: string; done: string; flags: MkfsFlag[] }

/** Static, per family: what exists before a volume of that family does. */
export interface FsFamily<O = unknown> {
  readonly id: FsFamilyId;
  readonly name: string;                                // "FAT16"; equals Volume.fsType() of format()'s result
  format(options?: O): Volume;                          // the one place Volume.formatFat16 is called
  bind(vol: Volume): FsAdapter;                         // caller has matched fsType; reads the caches once
  readonly mkfs: MkfsSpec;
}

/**
 * Bound to one Volume. Caches (owners, tables, geometry) are PLAIN fields refreshed by
 * `refresh()`; they are not reactive. Any `$derived` that calls a method here must read
 * `volume.epoch` first (see state/volume.svelte.ts). Tests use it without runes.
 */
export interface FsAdapter extends UnitSpace {
  readonly id: FsFamilyId;
  readonly name: string;
  readonly family: FsFamily;
  readonly vol: Volume;

  /** Re-read owners, tables and geometry from `vol`. Called by the store after every op and on bind. */
  refresh(): void;
  readonly owners: readonly UnitOwner[];                // sorted by unit
  ownerOf(path: string): UnitOwner | undefined;         // first owner row for a path

  chain(path: string): number[];                        // units in chain order; [] when none
  entrySlots(path: string): Interval | null;            // FAT: LFN + short slots; ext later: the inode's bytes
  /** Present only for families with "deleted things still on disk". Absent hides the Show remnants toggle. */
  remnants?(zeros: Uint8Array): Interval[];
  dataStart(path: string): number | null;               // "/" -> root directory bytes; file -> first unit start
  regionStart(kind: RegionKind): number | undefined;    // first sector of the first region of that kind (from vol.layout())

  stat(path: string): StatFacts;                        // FAT: firstCluster, chain, clusters, entryOffset, entrySlots, fatEntryOffset, dataOffset
  df(): DfFacts;
  trace(path: string): TraceRow[];                      // Inspector rows, family wording
  describeUnit(unit: number): string | null;            // Inspector: "FAT: next -> 4"
  annotateSector(sector: number): Annotation[];         // FAT: annotateSectorWith(sector, cached owners)

  /** Family address forms beyond s:N / hex / decimal / <letter>:N (ext: i:N). undefined = not ours. */
  parseAddr?(v: string): number | undefined;
  namesMatch(a: string, b: string): boolean;            // FAT: case-insensitive (vfs.canonicalize)

  /** True when the op may have moved regions; the store re-reads layout. FAT: any change starting below 512. */
  touchesMetadata(changes: ByteChangeLike[]): boolean;
  readonly corruptNote: string;                         // DirTree's note while the volume is corrupt
  readonly notes: { rewrite: string; partialWrite: string };  // the write --append and dd log clauses
}
```

Contracts, stated in the file's doc comment:

- `unitCount + unit.first` is the size of the attribution tables
  (`ownerByUnit`, `colorByUnit`); for FAT that is `clusterCount + 2`,
  exactly today's `n`.
- `stat()` returns already-formatted values (hex strings, `""` for absent)
  so the shell prints them untouched. The generic keys (`path`, `name`,
  `type`, `size`, `created`, `modified`, `accessed`) stay in `commands.ts`.
- `df()` returns generic keys; the shell derives the printed keys from the
  noun: `` `${unit.singular}Size` `` and `unit.plural`. FAT prints
  `clusterSize` and `clusters`, so `read-commands.test.ts` is unchanged.
- The address help is composed generically in `shell/addr.ts`:
  `` `addresses: 0x1f (hex), 512 (decimal), s:65 (sector), ${letter}:3 (${singular})` ``,
  which for FAT is byte-identical to today's `ADDR_HELP`.
- **Reactivity rule.** Adapter caches are plain, so every `$derived` that
  calls an adapter method reads `volume.epoch` first.

## 2. The FAT implementation (`web/ui/src/fs/fat16/`)

| file | contents |
|---|---|
| `index.ts` | re-exports `fat16: FsFamily<FormatOptions>` (`format: (o) => Volume.formatFat16(o)`, `bind: (v) => new Fat16Adapter(v)`, `mkfs: MKFS`), which is defined in `adapter.ts` because the class needs it for `family` and a definition in `index.ts` would form an import cycle that vite-node resolves to an `undefined` binding; `export type Fat16FormatOptions = FormatOptions`; `asFat16(fs: FsAdapter): Fat16Adapter` (throws unless `fs.id === "fat16"`); re-exports of the moved helpers |
| `adapter.ts` | `class Fat16Adapter implements FsAdapter`: fields `vol`, private `geo: Geometry`, private `space: UnitSpace`, public `fat: FatEntry[]` (FatMap reads it), private `rawOwners: ClusterOwner[]`, `owners: UnitOwner[]`. `refresh()` = `geo = vol.geometry(); space = fat16Space(geo); rawOwners = vol.clusterOwners(); owners = rawOwners.map(toUnitOwner); fat = vol.fatEntries(0)`: the same three wasm calls `refreshMeta` makes today, unguarded, so corrupt-volume behaviour is identical. `fatEntryOffset(cluster, copy = 0)` extracted once from `stat` and the Inspector. Unit methods delegate to `space` |
| `geometry.ts` | moved `clusterOfSector`, `clusterByteRange`; new `FAT_UNIT: UnitVocab`, `fat16Space(g: Geometry): UnitSpace`, `fatColorForRegion` (the `"FAT 1"` rule, else `defaultColorForRegion`) |
| `fatchain.ts` | moved `buildChain`, `clusterState`; plus `describeFatEntry` (the Inspector's `describe`) |
| `direntry.ts`, `remnants.ts` | moved verbatim, same signatures (they take raw `ClusterOwner[]`; the adapter passes `rawOwners`) |
| `metadata.ts` | moved `BOOT_SECTOR_LEN`, `touchesBootSector`; `CORRUPT_NOTE`, `NOTES` |
| `format.ts` | moved `SIZES`, `CLUSTER_SIZES`, the FAT16 geometry constants, `clusterCountFor`; new `checkFormat(o): { clusters: number; problem: string \| null }`, `DEFAULTS`, `MKFS: MkfsSpec` (the six flags with `option` keys `totalSectors`, `sectorsPerCluster`, `volumeLabel`, `rootEntries`, `fatCount`, `reservedSectors`; summary and done strings unchanged) |
| `FatMap.svelte` | moved from `components/`; reads `asFat16(volume.adapter)` for `.fat` and `volume.attribution` for colours |
| `FormatForm.svelte` | the body of ActionsPanel's `<details class="format">`: the two selects, the cluster-count line, the label input, the button; calls `volume.format("fat16", options)` |

`web/ui/src/fs/index.ts` is the registry:

```ts
export const FAMILIES: Record<FsFamilyId, FsFamily> = { fat16 };
export const DEFAULT_FAMILY: FsFamilyId = "fat16";
export function familyIdOf(fsType: string): FsFamilyId;   // "FAT16" -> "fat16"; throws Error(`no adapter for ${fsType}`)
export function adapterFor(vol: Volume): FsAdapter;      // FAMILIES[familyIdOf(vol.fsType())].bind(vol)
```

`web/ui/src/fs/panels.ts` maps a family id to its Svelte panels:
`PANELS: Record<FsFamilyId, { map: Component; format: Component }>`.
Ruling: panels live in this registry rather than on the adapter because
`vitest.config.ts` has no Svelte plugin and the adapter must stay importable
from node tests.

## 3. The generic store and its consumers

| `VolumeStore` (generic, reactive) | adapter (per epoch, plain) |
|---|---|
| `vol`, `image`, `epoch`, `layout`, `zeros`, `history`, `cursor`, `status`, `corruption`, `atLatest` | `owners` (plus raw owners), `fat`, geometry, the `UnitSpace` |
| `adapter = $state.raw<FsAdapter>` | |
| `sectorSize = $state(vol.sectorSize())`, `totalSectors = $state(vol.sectorCount())`, set in `adopt` | |
| `attribution = $derived.by(() => { this.epoch; return buildAttribution(this.adapter, this.layout, this.adapter.owners); })` | |

- `vol = $state.raw(FAMILIES[DEFAULT_FAMILY].format())`,
  `adapter = $state.raw(adapterFor(this.vol))`.
- `adopt(vol)`: set `vol`, `adapter = adapterFor(vol)`, `image`,
  `layout`, `sectorSize`, `totalSectors`, `zeros`; clear history and cursor;
  `refreshMeta()`; `epoch++`.
- `refreshMeta()`: `adapter.refresh(); corruption = vol.corruption()`. No
  try/catch: `corruption()` is on the trait and cannot throw `NotFat`.
- `format(family = adapter.id, options?)`: `adopt(FAMILIES[family].format(options))`.
- `load(bytes)`: unchanged; an unregistered `fsType()` makes `adapterFor`
  throw and lands in `status` like any other load failure.
- `run(fn)`: `adapter.touchesMetadata(rec.changes)` guards only the
  `layout` re-read; `refresh()` re-reads geometry unconditionally.

The reactivity rule applies at these sites: `layers.chain` (new read),
`layers.entry` and `layers.remnant` (already read `epoch`), `Ribbon`'s
files, free label, and region ticks (already), `DirTree.tree` (already),
`HexView.rowsInView` (already), `Inspector.annotations` (memo key includes
`epoch`) and its new `trace` derived (add), `LessonPanel.lookAt` (add).

`core/attribution.ts` becomes generic: `AttributionTable { space: UnitSpace;
regions; owners: readonly UnitOwner[]; ownerByUnit; colorByUnit }`,
`buildAttribution(space, layout, owners)`, `Attr.unit?` (was `cluster?`),
`attrAtSector` uses `space.unitOfSector` and `space.colorForRegion`,
`attrAtOffset` uses `space.sectorSize`, plus `defaultColorForRegion(region)`
by kind only. `core/palette.ts` renames `COLOR_FAT` and `COLOR_FAT_ALT` to
`COLOR_TABLE` and `COLOR_TABLE_ALT` with values and `--own-N` variables
unchanged. `core/tree.ts` takes `readonly UnitOwner[]` and exposes
`TreeNode.firstUnit: number | null`. `core/lesson.ts` is
`describeFocus(focus, space: UnitSpace, regionNameAt)` over `focus.unit`,
with the text `` `${space.unit.singular} ${n} in the data region (offset ...)` ``
(identical output; the "in the data region" literal stays).
`core/patch.ts` keeps `applyChanges` and `changedSectors` only.

`shell/addr.ts`: `parseAddr(v, space: AddrSpace)` handles `s:N`, hex,
decimal, `${unit.letter}:N` (`unitByteRange(N).start`), then
`space.parseAddr?.(v)`; `addrHelp(space)` replaces `ADDR_HELP`;
`parseSize` and `SIZE_HELP` unchanged;
`AddrSpace = Pick<UnitSpace, "sectorSize" | "unit" | "unitByteRange"> & { parseAddr?(v: string): number | undefined }`.

`shell/host.ts`: `ShellHost` gains `readonly adapter: FsAdapter` and
`format(family: FsFamilyId, options?: unknown): void`; `selectPath` uses
`canonicalize(host.adapter, path)`. `storeHost.svelte.ts` returns
`get adapter() { return volume.adapter; }` and forwards `format`. Commands
never call `adapterFor` themselves.

`shell/commands.ts`: `stat` keeps its generic keys and spreads
`host.adapter.stat(canon)`; `df` composes from `adapter.df()` with
noun-derived keys and the summary
`` `${cap(unit.singular)} usage of the mounted volume` ``; `seek`,
`write --at`, and `xxd --offset` use `parseAddr(x, host.adapter)` and
`addrHelp(host.adapter)` in their flag descriptions; `mkfs` builds its flags
from `host.adapter.family.mkfs.flags` (the int validation message unchanged)
and calls `host.format(host.adapter.id, options)`; the `touch` message says
"fs explorer"; the `write --append` clause is `host.adapter.notes.rewrite`.
`shell/dd.ts` uses `host.adapter.notes.partialWrite`. `shell/vfs.ts`:
`canonicalize(fs: Pick<FsAdapter, "vol" | "namesMatch">, path)`.
`shell/errors.ts`: `CORRUPT_HELP` reworded family-neutral, keeping
`dd --of=/dev/hda`: "the on-disk metadata no longer parses; rewind on the
timeline, or write the saved bytes back with: <the saved bytes> | dd --of=/dev/hda".

Scenarios (`state/scenarios.svelte.ts`):

```ts
export interface StepFocus { offset?: number; sector?: number; unit?: number; path?: string | null; showRemnants?: boolean; strings?: boolean }
export interface Step<O = unknown> {
  title: string; text: string;
  action?: (v: Volume, fs: FsAdapter) => OpRecord;
  format?: O;
  focus?: StepFocus | ((fs: FsAdapter) => StepFocus);
}
export interface Scenario<O = unknown> { id: string; title: string; summary: string; family: FsFamilyId; steps: Step<O>[] }
```

`start(s)` formats `s.family`; `applyStep` formats
`this.current.family` with `step.format` and runs
`volume.run((v) => step.action!(v, volume.adapter))`; `applyFocus`
resolves the function form with `volume.adapter` and maps `focus.unit` to
`volume.adapter.unitByteRange(unit).start` and `focus.sector` to
`sector * volume.sectorSize`. The nine FAT scenarios declare
`family: "fat16"`; the fundamentals scenario is the only one that needs a
FAT-only fact and uses `asFat16(fs).fatEntryOffset(cluster, copy)`.
Everything else they need is generic: `fs.entrySlots(p)?.start`,
`fs.ownerOf(p)?.firstUnit`, `fs.dataStart("/")`,
`fs.regionStart("directory")`, `fs.regionStart("data")`, `fs.unitSize`,
`fs.unitCount`.

Components:

| file | change |
|---|---|
| `App.svelte` | `const MapPanel = $derived(PANELS[volume.adapter.id].map)`; `<MapPanel />`; h1 "fs explorer" |
| `ActionsPanel.svelte` | keeps `<details class="format"><summary>Format</summary><FormatPanel /></details>` with `const FormatPanel = $derived(PANELS[volume.adapter.id].format)`; the FAT16 constants, `clusterCountFor`, and the form body move to `fs/fat16/` |
| `HexView.svelte`, `HexRow.svelte`, `app.css` | `unitStart = attr.unit !== undefined && sectorStart && volume.adapter.unitStartsAt(attr.sector)`; label `` `${unit.singular} ${attr.unit}...` ``; the `g` prompt names the family's letter and noun; `parseAddr(v, volume.adapter)`; prop `clusterStart` -> `unitStart`; class `.row.cluster-start` -> `.row.unit-start` |
| `Inspector.svelte` | `volume.adapter.annotateSector(sector)`; `<dt>{cap(unit.singular)}</dt>` with `describeUnit`; the trace list renders `volume.adapter.trace(selection.path)` rows |
| `TreeNodeView.svelte` | `node.firstUnit !== null ? \`first ${unit.singular} ${n}\` : "no data"`; jump via `adapter.unitByteRange` |
| `Ribbon.svelte` | `volume.totalSectors`; the free label loops `unit.first .. unit.first + unitCount` over `ownerByUnit` with `unitSize`; `buildTree(volume.vol, volume.adapter.owners)` |
| `StatusLine.svelte` | `` `That name isn't valid for ${volume.adapter.name}.` `` and `` `That image doesn't look like a valid ${volume.adapter.name} volume.` `` (identical today) |
| `DirTree.svelte` | `{volume.adapter.corruptNote}`; the Show remnants toggle inside `{#if volume.adapter.remnants}` |
| `LessonPanel.svelte` | `describeFocus(scenarios.focus, volume.adapter, ...)` with an `epoch` read |

## 4. Wasm and Rust

Only what this slice needs. `FatFs` behaviour and every DTO are untouched.

1. `FileSystem::corruption(&self) -> Option<&Error> { None }` on the trait
   (`crates/fs-core/src/fs.rs`), documented as "the `CorruptImage` a raw
   write left behind, or `None` while mounted; families without a gate keep
   the default". `FatFs` keeps its inherent `corruption()` and the trait impl
   delegates to it. `crates/wasm/src/volume.rs` moves `corruption` from the
   FAT-only impl block to the generic one as `self.fs().corruption()`; the
   wasm test `corruption_is_null_until_sector_zero_breaks_and_clears_when_repaired`
   passes unchanged; the `NullFs` test double needs nothing.
2. `fn detect(bytes: &[u8]) -> Detected` in `volume.rs`, with
   `enum Detected { Fat, Unknown }`, native-testable under
   `cargo test -p fs-emulator-wasm`. Rule: FAT when `len >= 512` and
   `bytes[510..512] == [0x55, 0xAA]`; a parsable BPB stays
   `FatFs::from_image`'s job. `from_image` maps `Fat | Unknown` to
   `FatFs::from_image` today, so every current error text and code stays
   identical. A comment reserves the ext check (`u16le` at 1080 equal to
   `0xEF53`, checked before the FAT check because a bootable ext image can
   also carry `55 AA` at 510) and the `Unsupported("no recognisable filesystem signature")`
   fallthrough for the ext slice.
3. `fat()` keeps its single arm; a second arm is an unreachable pattern
   under `-D warnings`. Its doc comment says the `NotFat` arm arrives with
   `Inner::Ext`. `host.ts`'s `corruptionOf` keeps its defensive try/catch
   with a reworded comment.
4. `formatFat16` stays; there is no generic wasm `format(kind, options)`.
5. `crates/wasm/README.md`: `corruption` joins the generic list; the FAT-only
   list becomes `bootSector`, `geometry`, `fatEntries`, `clusterChain`,
   `rawDirEntries`, `clusterOwners`, `annotateSectorWith`; a "Detection"
   paragraph states the rule above.
6. `docs/ROADMAP.md`: under "What stays fixed" add the adapter bullet
   (per-family UI knowledge lives behind `FsAdapter` in
   `web/ui/src/fs/<family>/`, chosen from `fsType()`, guarded by a
   source-scan test); checklist step 5 becomes "implement `FsAdapter` under
   `web/ui/src/fs/<family>/`, register it in `fs/index.ts` and `fs/panels.ts`,
   add its signature to `detect`"; delete the deferred note about
   `fatEntryOffset` duplication (resolved).

## 5. Naming pass

Renamed to "fs explorer": `web/ui/index.html` `<title>`, `App.svelte` h1,
`web/ui/README.md` (title line and the "What is FAT-specific" section, which
becomes "What is filesystem-specific" and describes `src/fs/` and the guard
test), `.github/workflows/pages.yml` job `name: build fs explorer`, the
`touch` message in `commands.ts` and its pin in
`tests/shell/mutations.test.ts`.

Deliberately not renamed: `FatMap.svelte` (file and component), the "FAT
map" heading and `aria-label="FAT cluster map"`, the `.fatmap*` CSS classes,
all scenario copy (they teach FAT16), the spec and plan file names
(historical records), the wasm names (`formatFat16`, `NotFat`, `FatFs`),
`tests/fixtures/geometry.ts`, the `fs-explorer.*` storage keys, the package
names, the crates README's "FAT-specific" wording, and the text of the
`write --append` and `dd` rewrite clauses (moved to `adapter.notes`,
strings unchanged).

## 6. Tests

New:

- `tests/fs/fat16.test.ts` (real `Volume`, no runes): `adapterFor(vol).id`
  and `.name`; the unit vocab; `unitOfSector(96 | 97 | 101)`,
  `unitByteRange(2)`, `unitOfOffset`, `unitStartsAt(97 | 98 | 101)`; `owners`
  and `ownerOf`; `chain("/DOCS/N.TXT")` equal to `[3, 4]`; `entrySlots` for
  root, a subdirectory, an LFN name, and `"/"`; `remnants` after a delete;
  `dataStart("/")` and of a file; `regionStart("directory") === 65`; `stat`
  returns exactly the seven FAT keys; `df()` used equal to 4 on the
  read-commands fixture; `trace` rows and `describeUnit(2) === "FAT: end of chain"`;
  `touchesMetadata`; `namesMatch`; `fatEntryOffset(2, 1)`; cache
  correctness (after `vol.createFile` without `refresh()` the adapter still
  reports the old owners, after `refresh()` the new); the corruption round
  trip (zero sector 0: `entrySlots` null, `remnants` `[]`, `buildTree`
  empty, `vol.corruption()` a string), ported from `corruption.test.ts`.
- `tests/fs/registry.test.ts`: `familyIdOf("FAT16") === "fat16"`, an unknown
  type throws, `FAMILIES.fat16.format({ rootEntries: 16 }).fsType() === "FAT16"`,
  `DEFAULT_FAMILY`.
- `tests/fs/fat16-format.test.ts`: `clusterCountFor(32768, 4) === 8167`,
  `checkFormat` problems at both FAT16 bounds, every `MKFS.flags[i].option`
  is a `FormatOptions` key, `DEFAULTS` reproduces the default disk's geometry.
- `tests/shell/addr.test.ts` additions: a stub `AddrSpace` with
  `parseAddr: (v) => v === "i:1" ? 1234 : undefined` proving the family hook
  and that unknown prefixes still throw with the composed help.
- `tests/adapterBoundary.test.ts`, a source scan like `layout.test.ts`: walk
  `src/**/*.{ts,svelte}`; outside `src/fs/fat16/**` and `src/lib/wasm.ts`
  fail on `/\.(geometry|fatEntries|clusterOwners|annotateSectorWith|rawDirEntries|bootSector|clusterChain|formatFat16)\s*\(/`
  and on an import from `lib/wasm` naming `ClusterOwner`, `FatEntry`,
  `Geometry`, `RawEntry`, `BootSector`, or `FormatOptions`; additionally fail
  on imports from `fs/fat16` except from `src/fs/index.ts`,
  `src/fs/panels.ts`, and `src/scenarios/**`. Scope is `src/` only; tests may
  poke FAT internals to compute expectations.

Existing tests that change, and why: `attribution` (space, owner shape,
`unit`, the palette constant rename), `lesson` (space, `unit`), `shell/addr`
(space, `addrHelp`), `integration` (import paths, `adapterFor`, `unit`,
`firstUnit`), `direntry`, `remnants`, `fatchain`, `patch`, `corruption`
(import paths only), `scenarios` and `fundamentals`
(`FAMILIES[s.family].format(...)`, `step.action(vol, adapterFor(vol))`,
`step.focus(adapterFor(vol))`, plus "every scenario names a registered
family"), `shell/read-commands` (helper imports, `ADDR_HELP` to `addrHelp`),
`shell/mutations` (the touch message), `shell/helpers.ts` (`TestHost` holds
an adapter, refreshes it after `run`, re-binds after `format(family, options)`),
`shell/errors` (the stub host gains `adapter`), `shell/vfs`
(`canonicalize(adapterFor(vol), ...)`). `tests/fixtures/geometry.ts` gains
`export const space = fat16Space(geo)`. Every asserted output string stays
the same; the only expectation edits are the field renames `cluster` to
`unit` and `firstCluster` to `firstUnit` in generic types. Rust adds one
native test for `detect`; the wasm boundary tests are unchanged.

## 7. Rulings and risks

1. Plain caches inside `$derived` are the main correctness risk: miss an
   `epoch` read and the dump keeps the previous op's owners, or a delete
   leaves the old chain highlighted. The rule is documented once in
   `fs/adapter.ts`, applied at the sites listed in section 3, and a browser
   pass covers add, delete, format, and raw write. Rejected: mirroring
   `owners` as `$state.raw` on the store, which is two sources of truth.
2. The shell output shape is kept by construction: `Fat16Adapter.stat`
   returns the seven FAT keys with the same hex formatting, `df`'s printed
   keys come from the noun, `addrHelp(fat16)` equals the old `ADDR_HELP`.
3. `CORRUPT_HELP` is attached by `wrapFs`, which has no adapter; the
   family-neutral rewording keeps the recovery command and the FAT sentence
   lives in `adapter.corruptNote` and the Rust message.
4. The `write --append` and `dd` clauses describe a core API limitation in
   FAT words; `adapter.notes` carries identical strings now so the ext
   adapter can say something true without touching the shell.
5. The generic-type renames (`Attr.unit`, `StepFocus.unit`,
   `TreeNode.firstUnit`) are done now: leaving `cluster` in generic types is
   the assumption this slice exists to remove.
6. `refresh()` on a corrupt volume calls the three FAT methods unguarded, as
   `refreshMeta` does today; the FAT crate answers from its last good
   geometry, and the adapter test covers it.
7. The family id stays `"fat16"`; when FAT32 shares the adapter,
   `familyIdOf` is the one place to remap.

## Carried forward to slices 2 to 4 (leanings, not decisions)

ext3 with the minimal feature set (revision 1, `has_journal`, `filetype`, no
extents, htree, 64-bit, or checksums); 1 KiB blocks on the 16 MiB default
disk so a learner sees two block groups; a real JBD2 v2 journal (descriptor,
data, commit, and revoke blocks) so images are Linux-mountable and e2fsck
replays them; an explicit crash point (before commit, and after commit
before checkpoint) with a recover operation as the teaching payoff;
e2fsprogs (`e2fsck -fn`, `debugfs`, `dumpe2fs`) as the CI gate, which also
runs on macOS through Homebrew without mounting. Each gets its own
brainstorm and spec.

## Verification

- Rust: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` (includes the native `detect` test),
  `wasm-pack test --node crates/wasm`, `wasm-pack build crates/wasm --target bundler`.
- Web: `pnpm install --frozen-lockfile && pnpm test && pnpm build`;
  svelte-check is the second net for any missed `cluster` or `geometry`
  access; the boundary test must pass.
- Browser (dev server `ui`): the title and h1 read "fs explorer"; add,
  overwrite, and delete a file and watch the tree, ribbon, dump, map, and
  Inspector trace update; format from the form and from `mkfs`; wipe sector
  0 with `dd`, see the corruption note, then repair; run "The fundamentals"
  and "Work from the shell" end to end; `stat`, `df`, `seek c:2`, and
  `xxd --offset s:65` print exactly what they print today.
