import type { Geometry, Region } from "../../lib/wasm";
import { defaultColorForRegion } from "../../core/attribution";
import { COLOR_TABLE_ALT, type ColorIndex } from "../../core/palette";
import type { AddrVocab, UnitSpace, UnitVocab } from "../adapter";

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

/** How FAT names its allocation unit: the shell writes `c:N`, and data clusters start at 2. */
export const FAT_UNIT: UnitVocab = { singular: "cluster", plural: "clusters", letter: "c", first: 2, fileParts: "its entry, chain, and clusters" };

/** How FAT names one disk sector: the shell writes `s:N`. */
export const FAT_SECTOR: AddrVocab = { singular: "sector", plural: "sectors", letter: "s" };

/** FAT's one colour rule of its own: the mirror copy "FAT 1" gets a lighter violet so "the FAT
 *  is written twice" is visible in the ribbon and the dump's owner stripe. Everything else is
 *  coloured by kind. */
export function fatColorForRegion(region: Region): ColorIndex {
  if (region.kind === "allocationTable" && region.name === "FAT 1") return COLOR_TABLE_ALT;
  return defaultColorForRegion(region);
}

/** Pure cluster arithmetic over one geometry; buildable from a fixture without a Volume. */
export function fat16Space(g: Geometry): UnitSpace {
  return {
    unit: FAT_UNIT,
    sector: FAT_SECTOR,
    unitCount: g.clusterCount,
    unitSize: g.bytesPerSector * g.sectorsPerCluster,
    sectorSize: g.bytesPerSector,
    totalSectors: g.totalSectors,
    unitOfSector: (sector) => clusterOfSector(g, sector),
    unitByteRange: (unit) => clusterByteRange(g, unit),
    unitOfOffset: (offset) => clusterOfSector(g, Math.floor(offset / g.bytesPerSector)),
    // The dump labels a row "cluster N" only on the first sector of a data cluster.
    unitStartsAt: (sector) => clusterOfSector(g, sector) !== undefined && (sector - g.firstDataSector) % g.sectorsPerCluster === 0,
    colorForRegion: fatColorForRegion,
  };
}
