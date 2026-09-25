import { Volume, type Annotation, type ClusterOwner, type FatEntry, type FormatOptions, type Geometry } from "../../lib/wasm";
import type { Interval } from "../../core/intervals";
import type { ByteChangeLike } from "../../core/patch";
import type { DfFacts, FsAdapter, FsFamily, FsFamilyId, StatFacts, TraceRow, UnitOwner, UnitSpace } from "../adapter";
import { SpaceAdapter, hexAddr } from "../base";
import { findEntrySlots } from "./direntry";
import { buildChain, describeFatEntry } from "./fatchain";
import { MKFS } from "./format";
import { fat16Space } from "./geometry";
import { CORRUPT_NOTE, NOTES, touchesBootSector } from "./metadata";
import { findRemnants } from "./remnants";

/** wasm's row in the generic shape. The raw rows are kept too, for the helpers that take `ClusterOwner[]`. */
function toUnitOwner(o: ClusterOwner): UnitOwner {
  return { unit: o.cluster, path: o.path, isDir: o.isDir, firstUnit: o.firstCluster };
}

/**
 * FAT16 behind the adapter. `refresh()` makes the same three wasm calls the store's
 * `refreshMeta` made before the seam (`geometry`, `clusterOwners`, `fatEntries(0)`), unguarded,
 * so a corrupt volume behaves exactly as it did: the FAT crate answers from its last good
 * geometry, and the directory walks inside `entrySlots` and `remnants` absorb `CorruptImage`
 * themselves. Every cache is a plain field (see the reactivity rule in fs/adapter.ts). The unit
 * arithmetic, `ownerOf`, and `regionStart` are the shared `SpaceAdapter`'s (fs/base.ts).
 */
export class Fat16Adapter extends SpaceAdapter implements FsAdapter {
  readonly id: FsFamilyId = "fat16";
  readonly name = "FAT16";
  readonly family: FsFamily<FormatOptions> = fat16;
  readonly vol: Volume;
  /** Every entry of FAT 0, indexed by cluster; FatMap paints from it. */
  fat: FatEntry[] = [];
  /** The owner rows in the generic shape, in cluster order (wasm returns them sorted). */
  owners: UnitOwner[] = [];
  readonly corruptNote = CORRUPT_NOTE;
  readonly notes = NOTES;
  /** FAT has no journal, so there is never a transaction to recover (and no `journal`). */
  readonly needsRecovery = false;
  protected space!: UnitSpace;
  private geo!: Geometry;
  private rawOwners: ClusterOwner[] = [];

  constructor(vol: Volume) {
    super();
    this.vol = vol;
    this.refresh();
  }

  refresh(): void {
    this.geo = this.vol.geometry();
    this.space = fat16Space(this.geo);
    this.rawOwners = this.vol.clusterOwners();
    this.owners = this.rawOwners.map(toUnitOwner);
    this.fat = this.vol.fatEntries(0);
  }

  chain(path: string): number[] {
    const owner = this.ownerOf(path);
    return owner ? buildChain(this.fat, owner.firstUnit) : [];
  }

  entrySlots(path: string): Interval | null {
    return findEntrySlots(this.vol, this.geo, this.fat, this.rawOwners, path);
  }

  remnants(zeros: Uint8Array): Interval[] {
    return findRemnants(this.vol, this.geo, this.fat, this.rawOwners, zeros);
  }

  dataStart(path: string): number | null {
    if (path === "/") return this.geo.firstRootDirSector * this.geo.bytesPerSector;
    const owner = this.ownerOf(path);
    return owner ? this.unitByteRange(owner.firstUnit).start : null;
  }

  /** Byte offset of `cluster`'s 16-bit entry in FAT copy `copy`; copy 0 is the one the explorer reads. */
  fatEntryOffset(cluster: number, copy = 0): number {
    const g = this.geo;
    return (g.reservedSectors + copy * g.sectorsPerFat) * g.bytesPerSector + cluster * 2;
  }

  /** The seven FAT facts `stat` prints after the generic ones, formatted as the shell always printed them. */
  stat(path: string): StatFacts {
    const first = this.ownerOf(path)?.firstUnit ?? 0;
    const chain = buildChain(this.fat, first);
    const slots = this.entrySlots(path);
    return {
      firstCluster: first,
      chain,
      clusters: chain.length,
      entryOffset: slots ? hexAddr(slots.start) : "",
      entrySlots: slots ? `${hexAddr(slots.start)}-${hexAddr(slots.end)}` : "",
      fatEntryOffset: first >= 2 ? hexAddr(this.fatEntryOffset(first)) : "",
      dataOffset: path === "/" ? hexAddr(this.geo.firstRootDirSector * this.geo.bytesPerSector) : first >= 2 ? hexAddr(this.unitByteRange(first).start) : "",
    };
  }

  /** Used clusters counted from FAT 0, as `df` always counted them. */
  df(): DfFacts {
    const g = this.geo, fat = this.fat;
    let used = 0;
    for (let c = 2; c < g.clusterCount + 2 && c < fat.length; c++) if (fat[c].kind !== "free") used++;
    return { unitSize: g.bytesPerSector * g.sectorsPerCluster, units: g.clusterCount, used, free: g.clusterCount - used };
  }

  /** The Inspector's "Selected file" rows: the directory entry, the FAT chain, the data. Wording unchanged. */
  trace(path: string): TraceRow[] {
    const rows: TraceRow[] = [];
    const entry = this.entrySlots(path);
    if (entry) rows.push({ label: `Directory entry · offset ${hexAddr(entry.start)}`, offset: entry.start });
    const chain = this.chain(path);
    const first = chain[0];
    if (first === undefined) {
      rows.push({ label: "Data · no data clusters", offset: null });
    } else {
      const fatEntry = this.fatEntryOffset(first);
      const data = this.unitByteRange(first).start;
      rows.push({ label: `FAT chain · ${chain.length} clusters starting at ${first} · FAT entry at ${hexAddr(fatEntry)}`, offset: fatEntry });
      rows.push({ label: `Data · cluster ${first} at ${hexAddr(data)}`, offset: data });
    }
    return rows;
  }

  describeUnit(unit: number): string | null {
    const e = this.fat[unit];
    return e ? `FAT: ${describeFatEntry(e)}` : null;
  }

  annotateSector(sector: number): Annotation[] {
    return this.vol.annotateSectorWith(sector, this.rawOwners);
  }

  /** FAT lookups are case-insensitive, so `/docs/n.txt` is `/DOCS/N.TXT` on disk. */
  namesMatch(a: string, b: string): boolean {
    return a.toUpperCase() === b.toUpperCase();
  }

  touchesMetadata(changes: ByteChangeLike[]): boolean {
    return touchesBootSector(changes);
  }
}

/**
 * The FAT16 family: the one place `Volume.formatFat16` is called, and the adapter it binds.
 * Defined beside the class rather than in index.ts because the two refer to each other
 * (`bind` news the class, the class's `family` is this object): keeping both in one module
 * avoids an import cycle, which vite-node (the vitest runner) resolves to an exports object
 * that never receives the late binding. index.ts re-exports it.
 */
export const fat16: FsFamily<FormatOptions> = {
  id: "fat16",
  name: "FAT16",
  fsTypes: ["FAT16"],
  format: (options) => Volume.formatFat16(options),
  bind: (vol) => new Fat16Adapter(vol),
  mkfs: MKFS,
};
