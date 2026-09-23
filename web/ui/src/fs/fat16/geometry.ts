import type { Geometry, Region } from "../../lib/wasm";
import { clusterByteRange, clusterOfSector } from "../../core/attribution";
import { COLOR_BOOT, COLOR_DIR, COLOR_FAT, COLOR_FAT_ALT, COLOR_FREE, type ColorIndex } from "../../core/palette";
import type { UnitSpace, UnitVocab } from "../adapter";

// The cluster arithmetic still lives in core/attribution.ts until Task 3 moves the two
// functions here for real; this is already their FAT home for every importer.
export { clusterByteRange, clusterOfSector };

/** How FAT names its allocation unit: the shell writes `c:N`, and data clusters start at 2. */
export const FAT_UNIT: UnitVocab = { singular: "cluster", plural: "clusters", letter: "c", first: 2 };

/** Region colours by kind, with FAT's one exception: the mirror copy ("FAT 1") gets its own
 *  lighter violet so "the FAT is written twice" is visible in the ribbon and the dump's
 *  owner stripe. */
export function fatColorForRegion(region: Region): ColorIndex {
  switch (region.kind) {
    case "allocationTable": return region.name === "FAT 1" ? COLOR_FAT_ALT : COLOR_FAT;
    case "directory": return COLOR_DIR;
    case "data": return COLOR_FREE;
    default: return COLOR_BOOT;
  }
}

/** Pure cluster arithmetic over one geometry; buildable from a fixture without a Volume. */
export function fat16Space(g: Geometry): UnitSpace {
  return {
    unit: FAT_UNIT,
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
