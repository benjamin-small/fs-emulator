import { describe, expect, it } from "vitest";
import { DEFAULT_FAMILY, FAMILIES, FAMILY_IDS, ROUTE_ALIASES, adapterFor, familyIdOf } from "../../src/fs";
import { formatRoute, parseRoute } from "../../src/core/route";
import { fat16 } from "../../src/fs/fat16";
import { ext } from "../../src/fs/ext";
import { CRASH_PHASES, CRASH_PHASE_LABELS, unitIsSector, type FsFamily } from "../../src/fs/adapter";

describe("the family registry", () => {
  it("maps Volume.fsType() to a family id, and refuses a type no adapter handles", () => {
    expect(familyIdOf("FAT16")).toBe("fat16");
    expect(() => familyIdOf("EXT3")).toThrow("no adapter for EXT3");
    expect(() => familyIdOf("")).toThrow("no adapter for ");
  });

  it("maps both ext types to the one ext family, matching the type string exactly", () => {
    expect(familyIdOf("ext2")).toBe("ext");
    expect(familyIdOf("ext3")).toBe("ext");
    expect(() => familyIdOf("EXT3")).toThrow("no adapter for EXT3"); // fsType() strings are lower-case
    expect(() => familyIdOf("ext4")).toThrow("no adapter for ext4");
    expect(() => familyIdOf("ntfs")).toThrow("no adapter for ntfs");
  });

  it("registers fat16 then ext, by id", () => {
    expect(Object.keys(FAMILIES)).toEqual(["fat16", "ext"]);
    expect(FAMILIES.ext).toBe(ext);
    expect(Object.values(FAMILIES).map((f) => [f.id, f.name])).toEqual([["fat16", "FAT16"], ["ext", "ext"]]);
  });

  it("lists the family ids in registry order, the order of the tabs", () => {
    expect(FAMILY_IDS).toEqual(["fat16", "ext"]);
    expect(FAMILY_IDS).toEqual(Object.keys(FAMILIES));
  });

  it("routes the short and the type names to a family's tab, and each id to its own", () => {
    expect(ROUTE_ALIASES).toEqual({ fat: "fat16", ext2: "ext", ext3: "ext" });
    for (const id of FAMILY_IDS) expect(parseRoute(formatRoute(id), FAMILY_IDS, ROUTE_ALIASES)).toBe(id);
    expect(parseRoute("#ext3", FAMILY_IDS, ROUTE_ALIASES)).toBe("ext");
    expect(parseRoute("#FAT", FAMILY_IDS, ROUTE_ALIASES)).toBe("fat16");
    expect(parseRoute("", FAMILY_IDS, ROUTE_ALIASES)).toBeNull();
  });

  it("registers FAT16 as the default family, whose format() yields a volume of its own name", () => {
    expect(DEFAULT_FAMILY).toBe("fat16");
    expect(FAMILIES[DEFAULT_FAMILY]).toBe(fat16);
    expect(FAMILIES.fat16.name).toBe("FAT16");
    const vol = FAMILIES.fat16.format({ rootEntries: 16 });
    expect(vol.fsType()).toBe("FAT16");
    expect(vol.geometry().rootEntries).toBe(16);
    expect(FAMILIES.fat16.format().geometry().rootEntries).toBe(512); // no options: the default disk
  });

  it("matches a type against each family's fsTypes, not its display name", () => {
    expect(fat16.fsTypes).toEqual(["FAT16"]);
    // One family may bind several types (ext: ext2 and ext3) under a display name that is
    // none of them; swap in such a family for the length of the check.
    const table = FAMILIES as Record<string, FsFamily>;
    table.fat16 = { ...fat16, name: "FAT", fsTypes: ["FAT16", "FAT16B"] };
    try {
      expect(familyIdOf("FAT16")).toBe("fat16");
      expect(familyIdOf("FAT16B")).toBe("fat16");
      expect(() => familyIdOf("FAT")).toThrow("no adapter for FAT");
    } finally {
      table.fat16 = fat16;
    }
    expect(FAMILIES.fat16).toBe(fat16);
  });

  it("adapterFor binds the family whose name is the volume's fsType", () => {
    const vol = FAMILIES.fat16.format();
    const fs = adapterFor(vol);
    expect(fs.id).toBe("fat16");
    expect(fs.name).toBe("FAT16");
    expect(fs.family).toBe(fat16);
    expect(fs.vol).toBe(vol);
  });

  it("adapterFor binds the ext family to an ext3 volume and to an ext2 one", () => {
    const vol = FAMILIES.ext.format();
    const fs = adapterFor(vol);
    expect(fs.id).toBe("ext");
    expect(fs.name).toBe("ext3");
    expect(fs.family).toBe(ext);
    expect(fs.vol).toBe(vol);
    expect(fs.journal).toBeDefined();
    const two = adapterFor(FAMILIES.ext.format({ variant: "ext2" }));
    expect([two.id, two.name, two.journal]).toEqual(["ext", "ext2", undefined]);
  });
});

describe("the seam's family-neutral helpers", () => {
  it("unitIsSector compares the two nouns, so a family whose unit is its sector shows one name", () => {
    const sector = { singular: "sector", plural: "sectors", letter: "s" };
    const block = { singular: "block", plural: "blocks", letter: "b" };
    expect(unitIsSector({ unit: { ...block, first: 1, fileParts: "its inode, block map, and blocks" }, sector: block })).toBe(true);
    expect(unitIsSector({ unit: { singular: "cluster", plural: "clusters", letter: "c", first: 2, fileParts: "its entry, chain, and clusters" }, sector })).toBe(false);
  });

  it("lists the crash phases in order, each with its label", () => {
    expect(CRASH_PHASES).toEqual(["before_commit", "after_commit", "during_checkpoint"]);
    expect(CRASH_PHASES.map((p) => CRASH_PHASE_LABELS[p])).toEqual(["before commit", "after commit", "during checkpoint"]);
  });
});
