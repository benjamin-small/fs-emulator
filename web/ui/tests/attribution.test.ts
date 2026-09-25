import { describe, expect, it } from "vitest";
import { attrAtOffset, attrAtSector, buildAttribution, defaultColorForRegion } from "../src/core/attribution";
import { COLOR_BOOT, COLOR_DIR, COLOR_FREE, COLOR_JOURNAL, COLOR_TABLE, COLOR_TABLE_ALT, colorIndexForPath } from "../src/core/palette";
import type { UnitOwner } from "../src/fs/adapter";
import { clusterByteRange, clusterOfSector } from "../src/fs/fat16/geometry";
import { geo, layout, space } from "./fixtures/geometry";
import { geo as extGeo, layout as extLayout, space as extSpace } from "./fixtures/extGeometry";

const owners: UnitOwner[] = [
  { unit: 2, path: "/DOCS", isDir: true, firstUnit: 2 },
  { unit: 3, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3 },
  { unit: 4, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3 },
];

/** Rows as the ext adapter makes them: the root directory, and a file with one data block and
 *  its single-indirect block (the real /bigger.txt has 13 data blocks, 1113..1125). */
const extOwners: UnitOwner[] = [
  { unit: 69, path: "/", isDir: true, firstUnit: 69, role: "directory" },
  { unit: 1113, path: "/bigger.txt", isDir: false, firstUnit: 1113, role: "data" },
  { unit: 1126, path: "/bigger.txt", isDir: false, firstUnit: 1113, role: "indirect" },
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
  it("colours a journal region COLOR_JOURNAL and never treats its sectors as units", () => {
    expect(COLOR_JOURNAL).toBe(11);
    const journal = { name: "journal", sectors: { start: 200, end: 300 }, kind: "journal" as const };
    expect(defaultColorForRegion(journal)).toBe(COLOR_JOURNAL);
    // The same disk with a journal carved out of the data region: its sectors map to clusters
    // in the unit arithmetic, but attribution keys units off `data` regions only.
    const split = [...layout.slice(0, 4), { name: "data", sectors: { start: 97, end: 200 }, kind: "data" as const }, journal, { name: "data", sectors: { start: 300, end: 32768 }, kind: "data" as const }];
    const tj = buildAttribution(space, split, owners);
    expect(space.unitOfSector(250)).toBeDefined();
    expect(attrAtSector(tj, 250)).toEqual({ regionKind: "journal", regionName: "journal", sector: 250, free: false, colorIndex: COLOR_JOURNAL });
  });
  it("gives an indirect owner row its path's hue and reports the role", () => {
    const withRole: UnitOwner[] = [
      { unit: 5, path: "/BIG", isDir: false, firstUnit: 5, role: "data" },
      { unit: 6, path: "/BIG", isDir: false, firstUnit: 5, role: "indirect" },
    ];
    const tr = buildAttribution(space, layout, withRole);
    expect(attrAtSector(tr, 113)).toMatchObject({ unit: 6, ownerPath: "/BIG", role: "indirect", colorIndex: colorIndexForPath("/BIG") });
    expect(attrAtSector(tr, 109)).toMatchObject({ unit: 5, role: "data", colorIndex: colorIndexForPath("/BIG") });
    expect(attrAtSector(t, 97).role).toBeUndefined(); // FAT rows carry no role
  });
  it("on ext: journal regions are never units, and metadata blocks are coloured by kind", () => {
    const te = buildAttribution(extSpace, extLayout, extOwners);
    expect(te.ownerByUnit.length).toBe(extGeo.totalBlocks); // unit.first (1) + unitCount (totalBlocks - 1)
    expect(attrAtSector(te, 0)).toEqual({ regionKind: "boot", regionName: "boot block", sector: 0, free: false, colorIndex: COLOR_BOOT });
    expect(attrAtSector(te, 3)).toMatchObject({ regionName: "block bitmap (group 0)", colorIndex: COLOR_TABLE });
    expect(attrAtSector(te, 6)).toMatchObject({ regionName: "inode table (group 0)", colorIndex: COLOR_BOOT });
    expect(attrAtSector(te, 8193)).toMatchObject({ regionName: "backup superblock (group 1)", colorIndex: COLOR_BOOT });
    // Block 90 is a unit by arithmetic, and even an owner row claiming it would not make it one.
    expect(extSpace.unitOfSector(90)).toBe(90);
    const claimed = buildAttribution(extSpace, extLayout, [...extOwners, { unit: 90, path: "/x", isDir: false, firstUnit: 90, role: "data" }]);
    for (const table of [te, claimed]) {
      expect(attrAtSector(table, 90)).toEqual({ regionKind: "journal", regionName: "journal", sector: 90, free: false, colorIndex: COLOR_JOURNAL });
    }
    // The journal's pointer blocks 1106..1110 lie in the data region after it. A table without
    // their owner rows would call them free; the adapter keeps them in `owners` (next test).
    expect(attrAtSector(te, 1107)).toMatchObject({ regionName: "data (group 0)", unit: 1107, free: true, colorIndex: COLOR_FREE });
  });
  it("on ext: the journal's pointer blocks are owned by <journal> in the journal colour", () => {
    const pointers: UnitOwner[] = [1106, 1107, 1108, 1109, 1110].map((unit) => ({ unit, path: "<journal>", isDir: false, firstUnit: unit, role: "indirect", color: COLOR_JOURNAL }));
    const te = buildAttribution(extSpace, extLayout, [...extOwners.slice(0, 1), ...pointers, ...extOwners.slice(1)]);
    expect(attrAtSector(te, 1107)).toEqual({ regionKind: "data", regionName: "data (group 0)", sector: 1107, unit: 1107, ownerPath: "<journal>", isDir: false, role: "indirect", free: false, colorIndex: COLOR_JOURNAL });
    expect(attrAtSector(te, 1113)).toMatchObject({ ownerPath: "/bigger.txt", colorIndex: colorIndexForPath("/bigger.txt") }); // rows without `color` are unchanged
  });
  it("on ext: an indirect block takes its file's hue and reports its role", () => {
    const te = buildAttribution(extSpace, extLayout, extOwners);
    expect(attrAtSector(te, 69)).toMatchObject({ unit: 69, ownerPath: "/", isDir: true, role: "directory", colorIndex: COLOR_DIR, free: false });
    expect(attrAtSector(te, 1113)).toMatchObject({ unit: 1113, ownerPath: "/bigger.txt", role: "data", colorIndex: colorIndexForPath("/bigger.txt") });
    expect(attrAtSector(te, 1126)).toMatchObject({ unit: 1126, ownerPath: "/bigger.txt", isDir: false, role: "indirect", colorIndex: colorIndexForPath("/bigger.txt"), free: false });
    expect(attrAtOffset(te, 1127 * 1024 + 5)).toMatchObject({ unit: 1127, free: true, colorIndex: COLOR_FREE });
    expect(attrAtOffset(te, 1127 * 1024 + 5).role).toBeUndefined();
  });
  it("uses an owner row's fixed colour verbatim, over the directory colour and the path's hue", () => {
    const fixed: UnitOwner[] = [
      { unit: 2, path: "/DOCS", isDir: true, firstUnit: 2, color: COLOR_BOOT },
      { unit: 3, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3, color: COLOR_JOURNAL },
      { unit: 4, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3 },
    ];
    const tf = buildAttribution(space, layout, fixed);
    expect(tf.colorByUnit[2]).toBe(COLOR_BOOT);
    expect(tf.colorByUnit[3]).toBe(COLOR_JOURNAL);
    expect(tf.colorByUnit[4]).toBe(colorIndexForPath("/DOCS/N.TXT"));
    expect(attrAtSector(tf, 101)).toMatchObject({ unit: 3, ownerPath: "/DOCS/N.TXT", colorIndex: COLOR_JOURNAL });
    expect(attrAtSector(tf, 97)).toMatchObject({ unit: 2, isDir: true, colorIndex: COLOR_BOOT });
  });

  it("file hues are stable and in range", () => {
    const i = colorIndexForPath("/A.TXT");
    expect(i).toBeGreaterThanOrEqual(4);
    expect(i).toBeLessThanOrEqual(9);
    expect(colorIndexForPath("/A.TXT")).toBe(i);
  });
});
