import type { ClusterOwner, Geometry, Region, RegionKind } from "../lib/wasm";
import { COLOR_BOOT, COLOR_DIR, COLOR_FAT, COLOR_FAT_ALT, COLOR_FREE, colorIndexForPath, type ColorIndex } from "./palette";

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

function colorForRegion(region: Region): ColorIndex {
  switch (region.kind) {
    // The mirror copy gets its own lighter violet so "the FAT is written twice"
    // is visible in the ribbon and the dump's owner stripe.
    case "allocationTable": return region.name === "FAT 1" ? COLOR_FAT_ALT : COLOR_FAT;
    case "directory": return COLOR_DIR;
    case "data": return COLOR_FREE;
    default: return COLOR_BOOT;
  }
}

export function attrAtSector(t: AttributionTable, sector: number): Attr {
  const region = regionOf(t.regions, sector);
  const base: Attr = { regionKind: region.kind, regionName: region.name, sector, free: false, colorIndex: colorForRegion(region) };
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
