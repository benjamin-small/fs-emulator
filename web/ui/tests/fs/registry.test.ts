import { describe, expect, it } from "vitest";
import { DEFAULT_FAMILY, FAMILIES, adapterFor, familyIdOf } from "../../src/fs";
import { fat16 } from "../../src/fs/fat16";

describe("the family registry", () => {
  it("maps Volume.fsType() to a family id, and refuses a type no adapter handles", () => {
    expect(familyIdOf("FAT16")).toBe("fat16");
    expect(() => familyIdOf("EXT3")).toThrow("no adapter for EXT3");
    expect(() => familyIdOf("")).toThrow("no adapter for ");
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

  it("adapterFor binds the family whose name is the volume's fsType", () => {
    const vol = FAMILIES.fat16.format();
    const fs = adapterFor(vol);
    expect(fs.id).toBe("fat16");
    expect(fs.name).toBe("FAT16");
    expect(fs.family).toBe(fat16);
    expect(fs.vol).toBe(vol);
  });
});
