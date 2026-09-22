import type { Geometry, Region } from "../../src/lib/wasm";

// The default `Volume.formatFat16(undefined)` disk: 16 MiB, 512-byte sectors, 4 sectors per
// cluster, two 32-sector FATs, a 32-sector root directory, data from sector 97.
export const geo: Geometry = { variant: "fat16", bytesPerSector: 512, sectorsPerCluster: 4, reservedSectors: 1, fatCount: 2, sectorsPerFat: 32, rootEntries: 512, rootDirSectors: 32, firstRootDirSector: 65, firstDataSector: 97, totalSectors: 32768, clusterCount: 8167 };
export const layout: Region[] = [
  { name: "reserved (boot sector)", sectors: { start: 0, end: 1 }, kind: "boot" },
  { name: "FAT 0", sectors: { start: 1, end: 33 }, kind: "allocationTable" },
  { name: "FAT 1", sectors: { start: 33, end: 65 }, kind: "allocationTable" },
  { name: "root directory", sectors: { start: 65, end: 97 }, kind: "directory" },
  { name: "data", sectors: { start: 97, end: 32768 }, kind: "data" },
];
