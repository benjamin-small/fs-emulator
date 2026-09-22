import { describe, expect, it } from "vitest";
import { attrAtOffset, attrAtSector, buildAttribution, clusterByteRange, clusterOfSector } from "../src/core/attribution";
import { COLOR_FAT_ALT, colorIndexForPath } from "../src/core/palette";
import type { ClusterOwner, Geometry, Region } from "../src/lib/wasm";

export const geo: Geometry = { variant: "fat16", bytesPerSector: 512, sectorsPerCluster: 4, reservedSectors: 1, fatCount: 2, sectorsPerFat: 32, rootEntries: 512, rootDirSectors: 32, firstRootDirSector: 65, firstDataSector: 97, totalSectors: 32768, clusterCount: 8167 };
export const layout: Region[] = [
  { name: "reserved (boot sector)", sectors: { start: 0, end: 1 }, kind: "boot" },
  { name: "FAT 0", sectors: { start: 1, end: 33 }, kind: "allocationTable" },
  { name: "FAT 1", sectors: { start: 33, end: 65 }, kind: "allocationTable" },
  { name: "root directory", sectors: { start: 65, end: 97 }, kind: "directory" },
  { name: "data", sectors: { start: 97, end: 32768 }, kind: "data" },
];
const owners: ClusterOwner[] = [
  { cluster: 2, path: "/DOCS", isDir: true, firstCluster: 2 },
  { cluster: 3, path: "/DOCS/N.TXT", isDir: false, firstCluster: 3 },
  { cluster: 4, path: "/DOCS/N.TXT", isDir: false, firstCluster: 3 },
];

describe("attribution", () => {
  const t = buildAttribution(geo, layout, owners);
  it("maps sectors to clusters like the core", () => {
    expect(clusterOfSector(geo, 96)).toBeUndefined();
    expect(clusterOfSector(geo, 97)).toBe(2);
    expect(clusterOfSector(geo, 100)).toBe(2);
    expect(clusterOfSector(geo, 101)).toBe(3);
    expect(clusterOfSector(geo, 32764)).toBe(8168);
    expect(clusterOfSector(geo, 32767)).toBeUndefined(); // past the last full cluster
    expect(clusterByteRange(geo, 2)).toEqual({ start: 97 * 512, end: 97 * 512 + 2048 });
  });
  it("attributes metadata regions", () => {
    expect(attrAtSector(t, 0)).toMatchObject({ regionKind: "boot", colorIndex: 1, free: false });
    expect(attrAtSector(t, 1)).toMatchObject({ regionName: "FAT 0", colorIndex: 2 });
    expect(attrAtSector(t, 64)).toMatchObject({ regionName: "FAT 1", colorIndex: COLOR_FAT_ALT }); // the mirror copy gets its own lighter hue
    expect(attrAtSector(t, 65)).toMatchObject({ regionKind: "directory", colorIndex: 3 });
  });
  it("attributes data clusters to owners and free space", () => {
    expect(attrAtOffset(t, 97 * 512)).toMatchObject({ cluster: 2, ownerPath: "/DOCS", isDir: true, colorIndex: 3, free: false });
    expect(attrAtOffset(t, 101 * 512 + 7)).toMatchObject({ cluster: 3, ownerPath: "/DOCS/N.TXT", isDir: false, colorIndex: colorIndexForPath("/DOCS/N.TXT") });
    expect(attrAtOffset(t, 105 * 512)).toMatchObject({ cluster: 4, ownerPath: "/DOCS/N.TXT" });
    expect(attrAtOffset(t, 109 * 512)).toMatchObject({ cluster: 5, free: true, colorIndex: 0 });
    expect(attrAtOffset(t, 109 * 512).ownerPath).toBeUndefined();
  });
  it("file hues are stable and in range", () => {
    const i = colorIndexForPath("/A.TXT");
    expect(i).toBeGreaterThanOrEqual(4);
    expect(i).toBeLessThanOrEqual(9);
    expect(colorIndexForPath("/A.TXT")).toBe(i);
  });
});
