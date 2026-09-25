import type { Region, RegionKind, Volume } from "../lib/wasm";
import type { Interval } from "../core/intervals";
import type { ColorIndex } from "../core/palette";
import type { AddrVocab, UnitOwner, UnitSpace, UnitVocab } from "./adapter";

/** An offset as the shell and the Inspector print it: `0x` and lower-case hex. */
export const hexAddr = (n: number): string => `0x${n.toString(16)}`;

/**
 * The part of an adapter every family writes the same way, so each family writes it once:
 * the `UnitSpace` members delegate to the space its last `refresh()` built, `ownerOf` is the
 * first owner row for a path, and `regionStart` reads the volume's layout. A family extends it,
 * sets `space` and `owners` in `refresh()`, and supplies the rest of `FsAdapter`. The fields
 * stay plain (the reactivity rule in fs/adapter.ts).
 */
export abstract class SpaceAdapter implements UnitSpace {
  abstract readonly vol: Volume;
  abstract owners: UnitOwner[];
  /** The unit arithmetic over the geometry the last `refresh()` read. */
  protected abstract space: UnitSpace;

  get unit(): UnitVocab { return this.space.unit; }
  get sector(): AddrVocab { return this.space.sector; }
  get unitCount(): number { return this.space.unitCount; }
  get unitSize(): number { return this.space.unitSize; }
  get sectorSize(): number { return this.space.sectorSize; }
  get totalSectors(): number { return this.space.totalSectors; }
  unitOfSector(sector: number): number | undefined { return this.space.unitOfSector(sector); }
  unitByteRange(unit: number): Interval { return this.space.unitByteRange(unit); }
  unitOfOffset(offset: number): number | undefined { return this.space.unitOfOffset(offset); }
  unitStartsAt(sector: number): boolean { return this.space.unitStartsAt(sector); }
  colorForRegion(region: Region): ColorIndex { return this.space.colorForRegion(region); }

  /** The first owner row for `path` (its first unit's row on FAT). */
  ownerOf(path: string): UnitOwner | undefined {
    return this.owners.find((o) => o.path === path);
  }

  /** The units from `unit.first` through the last with no owner row: the ribbon's free count.
   *  A family whose unowned units are not all free (ext) overrides it. */
  freeUnits(): number {
    const owned = new Set(this.owners.map((o) => o.unit));
    let free = 0;
    for (let u = this.unit.first; u < this.unit.first + this.unitCount; u++) if (!owned.has(u)) free++;
    return free;
  }

  /** The first sector of the first region of that kind in the volume's layout. */
  regionStart(kind: RegionKind): number | undefined {
    return this.vol.layout().find((r) => r.kind === kind)?.sectors.start;
  }
}
