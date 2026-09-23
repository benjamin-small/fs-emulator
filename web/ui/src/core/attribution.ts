import type { Region, RegionKind } from "../lib/wasm";
import type { UnitOwner, UnitSpace } from "../fs/adapter";
import { COLOR_BOOT, COLOR_DIR, COLOR_FREE, COLOR_TABLE, colorIndexForPath, type ColorIndex } from "./palette";

export type { ColorIndex };
export interface Attr {
  regionKind: RegionKind; regionName: string; sector: number;
  unit?: number; ownerPath?: string; isDir?: boolean; free: boolean; colorIndex: ColorIndex;
}
/** Byte ownership for one epoch. `ownerByUnit` and `colorByUnit` are indexed by unit number and
 *  have `space.unit.first + space.unitCount` rows (FAT: `clusterCount + 2`), so the rows below
 *  `unit.first` are never owned. */
export interface AttributionTable {
  space: UnitSpace; regions: Region[]; owners: readonly UnitOwner[];
  /** unit -> index into owners, or -1 */
  ownerByUnit: Int32Array;
  colorByUnit: Uint8Array;
}

export function buildAttribution(space: UnitSpace, layout: Region[], owners: readonly UnitOwner[]): AttributionTable {
  const n = space.unit.first + space.unitCount;
  const ownerByUnit = new Int32Array(n).fill(-1);
  const colorByUnit = new Uint8Array(n);
  owners.forEach((o, i) => {
    if (o.unit < n) {
      ownerByUnit[o.unit] = i;
      colorByUnit[o.unit] = o.isDir ? COLOR_DIR : colorIndexForPath(o.path);
    }
  });
  return { space, regions: layout, owners, ownerByUnit, colorByUnit };
}

function regionOf(regions: Region[], sector: number): Region {
  for (const r of regions) if (sector >= r.sectors.start && sector < r.sectors.end) return r;
  return { name: "unknown", sectors: { start: sector, end: sector + 1 }, kind: "other" };
}

/** A region's colour from its kind alone. A family's `UnitSpace.colorForRegion` starts here and
 *  adds what only it knows (FAT: the mirror copy "FAT 1" gets `COLOR_TABLE_ALT`). */
export function defaultColorForRegion(region: Region): ColorIndex {
  switch (region.kind) {
    case "allocationTable": return COLOR_TABLE;
    case "directory": return COLOR_DIR;
    case "data": return COLOR_FREE;
    default: return COLOR_BOOT;
  }
}

export function attrAtSector(t: AttributionTable, sector: number): Attr {
  const region = regionOf(t.regions, sector);
  const base: Attr = { regionKind: region.kind, regionName: region.name, sector, free: false, colorIndex: t.space.colorForRegion(region) };
  if (region.kind !== "data") return base;
  const unit = t.space.unitOfSector(sector);
  if (unit === undefined) return { ...base, free: true };
  const idx = t.ownerByUnit[unit];
  if (idx < 0) return { ...base, unit, free: true, colorIndex: COLOR_FREE };
  const o = t.owners[idx];
  return { ...base, unit, ownerPath: o.path, isDir: o.isDir, free: false, colorIndex: t.colorByUnit[unit] };
}

export function attrAtOffset(t: AttributionTable, offset: number): Attr {
  return attrAtSector(t, Math.floor(offset / t.space.sectorSize));
}
