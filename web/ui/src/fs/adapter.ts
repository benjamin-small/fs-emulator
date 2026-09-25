/**
 * The filesystem adapter seam. Every filesystem-specific fact the explorer, the shell and
 * the lessons need lives behind these interfaces, with one implementation per family:
 * `fs/fat16/` for `FAT16` and `fs/ext/` for `ext2` and `ext3` (a family lists the
 * `Volume.fsType()` strings it binds in `fsTypes`); `fs/index.ts` picks the implementation.
 *
 * Two layers. `UnitSpace` is pure arithmetic over one geometry and can be built from a
 * fixture without a `Volume`. `FsAdapter` is bound to one `Volume` and caches owners,
 * tables, and geometry as plain fields that `refresh()` re-reads.
 *
 * Contracts:
 *
 * - `unitCount + unit.first` is the size of the attribution tables (`ownerByUnit`,
 *   `colorByUnit`); for FAT that is `clusterCount + 2`, exactly the table size before the seam.
 * - `stat()` returns already-formatted values (hex strings, `""` for absent) so the shell
 *   prints them untouched. The generic keys (`path`, `name`, `type`, `size`, `created`,
 *   `modified`, `accessed`) stay in `shell/commands.ts`.
 * - `df()` returns generic keys; the shell derives the printed keys from the noun:
 *   `${unit.singular}Size` and `unit.plural`. FAT prints `clusterSize` and `clusters`, so
 *   `tests/shell/read-commands.test.ts` is unchanged.
 * - The sector noun rule. `sector` is what the chrome calls one disk sector (FAT "sector",
 *   ext "block"); `unit` is the allocation unit (FAT "cluster", ext "block"). Where the two
 *   coincide (`unitIsSector`) the chrome shows one name: the Inspector folds its unit row
 *   into the address row, and the address help and HexView's `g` prompt list one form.
 * - The address help is composed generically in `shell/addr.ts` from the two nouns:
 *   `addresses: 0x1f (hex), 512 (decimal), ${sector.letter}:65 (${sector.singular})`, then
 *   `, ${unit.letter}:3 (${unit.singular})` when the unit is not the sector, then the
 *   adapter's `extraAddrHelp`. For FAT that is byte-identical to the old `ADDR_HELP`.
 * - Journal blocks are never units. Attribution keys units off `data` regions only; a
 *   family's journal lies in regions of kind `journal` (coloured `COLOR_JOURNAL`), and those
 *   blocks never appear in `owners`. A journal block outside those regions (ext's pointer
 *   blocks, in a data region) is an owner row with a fixed `color`, so it reads as the
 *   journal's rather than free.
 * - Reactivity rule. Adapter caches are plain fields, not runes, so every `$derived` that
 *   calls an adapter method reads `volume.epoch` first (see state/volume.svelte.ts).
 */
import type { Volume, Region, RegionKind, Annotation, OpRecord } from "../lib/wasm";
import type { Interval } from "../core/intervals";
import type { ColorIndex } from "../core/palette";
import type { ByteChangeLike } from "../core/patch";

export type FsFamilyId = "fat16" | "ext";

/** A noun and its shell address letter: how the chrome names one disk sector. */
export interface AddrVocab {
  singular: string;   // "sector"
  plural: string;     // "sectors"
  letter: string;     // "s": the shell address prefix (s:N, which every family also accepts)
}

/** The allocation-unit noun: how the UI, shell, and lessons name a data unit. */
export interface UnitVocab {
  singular: string;   // "cluster"
  plural: string;     // "clusters"
  letter: string;     // "c": the shell address prefix (c:N)
  first: number;      // first valid unit index (FAT: 2)
  fileParts: string;  // the Lesson card's clause after a path: "its entry, chain, and clusters"
}

/** One unit -> the path that owns it. Generic mirror of wasm's ClusterOwner. `role` names what
 *  the unit holds for its path where the family tells them apart (ext: a data block, a
 *  directory block, or an indirect pointer block); FAT leaves it undefined. An indirect row
 *  takes its path's hue like a data row. */
export interface UnitOwner {
  unit: number;
  path: string;
  isDir: boolean;
  firstUnit: number;
  role?: "data" | "directory" | "indirect";
  /** A fixed colour override for special owners such as the journal's pointer blocks (ext:
   *  `COLOR_JOURNAL`); attribution uses it verbatim instead of the directory colour or the
   *  path's hue. FAT rows leave it undefined. */
  color?: ColorIndex;
}

/** An owner that names no file in the tree, such as ext's `<journal>` (its pointer blocks): the
 *  path is in angle brackets, which no volume path starts with. The map and the Inspector name
 *  it but never select it. */
export function isPseudoOwner(path: string): boolean {
  return path.startsWith("<");
}

/** Pure unit arithmetic over one geometry. Buildable without a Volume. */
export interface UnitSpace {
  readonly unit: UnitVocab;
  readonly sector: AddrVocab;                           // FAT: sector / s; ext: block / b
  readonly unitCount: number;                           // FAT: clusterCount
  readonly unitSize: number;                            // bytes per unit
  readonly sectorSize: number;                          // vol.sectorSize()
  readonly totalSectors: number;                        // sectors the filesystem describes (FAT: the BPB total); the store's `totalSectors` is the disk's
  unitOfSector(sector: number): number | undefined;     // undefined outside the data area
  unitByteRange(unit: number): Interval;                // [start, end) bytes; no bounds check, as today
  unitOfOffset(offset: number): number | undefined;
  unitStartsAt(sector: number): boolean;                // the dump's row-label rule
  colorForRegion(region: Region): ColorIndex;           // FAT: "FAT 1" gets COLOR_TABLE_ALT
}

/** True when the family's allocation unit is its sector (ext: both are the block), so the
 *  chrome names it once. Compares the singular nouns. */
export function unitIsSector(space: Pick<UnitSpace, "unit" | "sector">): boolean {
  return space.unit.singular === space.sector.singular;
}

/** One row of the Inspector's "Selected file" trace. */
export interface TraceRow { label: string; offset: number | null }   // null = muted text, no jump
/** stat facts the shell prints as key/value after the generic ones. Keys are the family's. */
export type StatFacts = Record<string, string | number | number[]>;
export interface DfFacts { unitSize: number; units: number; used: number; free: number }

export interface MkfsFlag { long: string; desc: string; kind: "int" | "str"; option: string }
/** The shell's `mkfs` flags for one family. Each tab's `mkfs` lists its own family's flags; its
 *  summary is the shell's (it names the types `--type` takes), and so is its done line: `formatted
 *  /dev/hda as ${vol.fsType()}; the timeline was cleared`, composed from the new volume, so a
 *  family with two types (ext2, ext3) names the one it made. */
export interface MkfsSpec { flags: MkfsFlag[] }

/** Where an armed crash stops the next journaled change: before its commit block is written,
 *  after the commit but before the checkpoint, or part way through the checkpoint. The strings
 *  are wasm's `armCrash` phases. */
export type CrashPhase = "before_commit" | "after_commit" | "during_checkpoint";
export const CRASH_PHASES: readonly CrashPhase[] = ["before_commit", "after_commit", "during_checkpoint"];
export const CRASH_PHASE_LABELS: Record<CrashPhase, string> = {
  before_commit: "before commit",
  after_commit: "after commit",
  during_checkpoint: "during checkpoint",
};

/** The journal's header facts: a structural twin of wasm's `JournalInfo`, declared here so
 *  nothing outside a family's folder imports an ext-only wasm type. */
export interface JournalState {
  mode: "ordered" | "data";
  sequence: number;
  head: number;
  start: number;
  maxlen: number;
  firstBlock: number;
  maxTransaction: number;
  needsRecovery: boolean;
}

/** One block of the journal ring, in reading order: a structural twin of wasm's
 *  `JournalBlock`. `home` is the block a copy is written back to; `stale` marks a transaction
 *  already checkpointed. */
export interface JournalRingBlock {
  index: number;
  block: number;
  kind: "superblock" | "descriptor" | "copy" | "commit" | "revoke" | "unused";
  tid: number | null;
  home: number | null;
  escaped: boolean | null;
  stale: boolean;
}

/**
 * The optional journal part of an adapter (ext3). `state()` answers from the adapter's cache,
 * refreshed with it; `blocks()` reads the volume, so callers memoise it per `volume.epoch`.
 * Arming and disarming are not recorded operations. `recover()` runs nothing through the
 * timeline itself: it returns the volume's op for the caller to run, as
 * `volume.run(() => journal.recover())` or `host.run(() => journal.recover())`.
 */
export interface JournalCapability {
  state(): JournalState;
  blocks(): JournalRingBlock[];
  arm(phase: CrashPhase): void;
  disarm(): void;
  phase(): CrashPhase | null;
  recover(): OpRecord;
}

/** Static, per family: what exists before a volume of that family does. */
export interface FsFamily<O = unknown> {
  readonly id: FsFamilyId;
  readonly name: string;                                // the family's display name: "FAT16"
  readonly fsTypes: readonly string[];                  // the Volume.fsType() strings it binds: ["FAT16"]
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
  readonly name: string;                                // the bound volume's type: "FAT16"
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
  /** The units the ribbon counts free. FAT: the units no owner row claims, the count the ribbon
   *  always made (a bad cluster or a lost chain stays free there). ext: `df().free`, because the
   *  inode tables, the bitmaps, and the journal have no owner rows and are not free. */
  freeUnits(): number;
  trace(path: string): TraceRow[];                      // Inspector rows, family wording
  describeUnit(unit: number): string | null;            // Inspector: "FAT: next -> 4"
  annotateSector(sector: number): Annotation[];         // FAT: annotateSectorWith(sector, cached owners)

  /** Family address forms beyond s:N / hex / decimal / <letter>:N (ext: i:N). undefined = not ours. */
  parseAddr?(v: string): number | undefined;
  /** What `parseAddr` adds to the address help, appended as is (ext: ", i:11 (inode)"). */
  readonly extraAddrHelp?: string;
  namesMatch(a: string, b: string): boolean;            // FAT: case-insensitive (vfs.canonicalize)

  /** True when the op may have moved regions; the store re-reads layout. FAT: any change starting below 512. */
  touchesMetadata(changes: ByteChangeLike[]): boolean;
  readonly corruptNote: string;                         // DirTree's note while the volume is corrupt
  readonly notes: { rewrite: string; partialWrite: string };  // the write --append and dd log clauses

  /** True while the volume holds an unfinished journal transaction; FAT and ext2: always false. */
  readonly needsRecovery: boolean;
  /** Present only for a family with a journal (ext3); drives the Journal panel, `crash`, and `recover`. */
  readonly journal?: JournalCapability;
}
