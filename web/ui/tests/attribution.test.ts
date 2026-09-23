import { describe, expect, it } from "vitest";
import { attrAtOffset, attrAtSector, buildAttribution, defaultColorForRegion } from "../src/core/attribution";
import { COLOR_TABLE, COLOR_TABLE_ALT, colorIndexForPath } from "../src/core/palette";
import type { UnitOwner } from "../src/fs/adapter";
import { clusterByteRange, clusterOfSector } from "../src/fs/fat16/geometry";
import { geo, layout, space } from "./fixtures/geometry";

const owners: UnitOwner[] = [
  { unit: 2, path: "/DOCS", isDir: true, firstUnit: 2 },
  { unit: 3, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3 },
  { unit: 4, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3 },
];

describe("attribution", () => {
  const t = buildAttribution(space, layout, owners);
  it("maps sectors to clusters like the core", () => {
    expect(clusterOfSector(geo, 96)).toBeUndefined();
    expect(clusterOfSector(geo, 97)).toBe(2);
    expect(clusterOfSector(geo, 100)).toBe(2);
    expect(clusterOfSector(geo, 101)).toBe(3);
    expect(clusterOfSector(geo, 32764)).toBe(8168);
    expect(clusterOfSector(geo, 32767)).toBeUndefined(); // past the last full cluster
    expect(clusterByteRange(geo, 2)).toEqual({ start: 97 * 512, end: 97 * 512 + 2048 });
  });
  it("sizes its tables from the space: unit.first + unitCount rows, the FAT's clusterCount + 2", () => {
    expect(t.space).toBe(space);
    expect(t.ownerByUnit.length).toBe(geo.clusterCount + 2);
    expect(t.colorByUnit.length).toBe(geo.clusterCount + 2);
    expect(t.ownerByUnit[2]).toBe(0); // index into owners
    expect(t.ownerByUnit[5]).toBe(-1);
  });
  it("attributes metadata regions", () => {
    expect(attrAtSector(t, 0)).toMatchObject({ regionKind: "boot", colorIndex: 1, free: false });
    expect(attrAtSector(t, 1)).toMatchObject({ regionName: "FAT 0", colorIndex: 2 });
    expect(attrAtSector(t, 64)).toMatchObject({ regionName: "FAT 1", colorIndex: COLOR_TABLE_ALT }); // the mirror copy gets its own lighter hue
    expect(attrAtSector(t, 65)).toMatchObject({ regionKind: "directory", colorIndex: 3 });
  });
  it("colours a region by kind alone by default; the space adds what only the family knows", () => {
    const fat1 = layout[2];
    expect(fat1.name).toBe("FAT 1");
    expect(defaultColorForRegion(fat1)).toBe(COLOR_TABLE);
    expect(space.colorForRegion(fat1)).toBe(COLOR_TABLE_ALT);
    expect(layout.map((r) => defaultColorForRegion(r))).toEqual([1, 2, 2, 3, 0]); // boot, FAT 0, FAT 1, root directory, data
  });
  it("attributes data clusters to owners and free space", () => {
    expect(attrAtOffset(t, 97 * 512)).toMatchObject({ unit: 2, ownerPath: "/DOCS", isDir: true, colorIndex: 3, free: false });
    expect(attrAtOffset(t, 97 * 512)).not.toHaveProperty("cluster");
    expect(attrAtOffset(t, 101 * 512 + 7)).toMatchObject({ unit: 3, ownerPath: "/DOCS/N.TXT", isDir: false, colorIndex: colorIndexForPath("/DOCS/N.TXT") });
    expect(attrAtOffset(t, 105 * 512)).toMatchObject({ unit: 4, ownerPath: "/DOCS/N.TXT" });
    expect(attrAtOffset(t, 109 * 512)).toMatchObject({ unit: 5, free: true, colorIndex: 0 });
    expect(attrAtOffset(t, 109 * 512).ownerPath).toBeUndefined();
  });
  it("file hues are stable and in range", () => {
    const i = colorIndexForPath("/A.TXT");
    expect(i).toBeGreaterThanOrEqual(4);
    expect(i).toBeLessThanOrEqual(9);
    expect(colorIndexForPath("/A.TXT")).toBe(i);
  });
});
