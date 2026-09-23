/**
 * The filesystem adapter seam. Every filesystem-specific fact the explorer, the shell and
 * the lessons need lives behind these interfaces, with one implementation per
 * `Volume.fsType()` family (`fs/fat16/` today); `fs/index.ts` picks the implementation.
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
 * - The address help is composed generically in `shell/addr.ts`:
 *   `addresses: 0x1f (hex), 512 (decimal), s:65 (sector), ${letter}:3 (${singular})`,
 *   which for FAT is byte-identical to the old `ADDR_HELP`.
 * - Reactivity rule. Adapter caches are plain fields, not runes, so every `$derived` that
 *   calls an adapter method reads `volume.epoch` first (see state/volume.svelte.ts).
 */
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
  readonly totalSectors: number;                        // sectors the filesystem describes (FAT: the BPB total); the store's `totalSectors` is the disk's
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
