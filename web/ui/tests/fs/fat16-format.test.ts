import { describe, expect, it } from "vitest";
import { Volume, type FormatOptions } from "../../src/lib/wasm";
import { CLUSTER_SIZES, DEFAULTS, MKFS, SIZES, checkFormat, clusterCountFor } from "../../src/fs/fat16";
import { FAT16_MAX_CLUSTERS, FAT16_MIN_CLUSTERS } from "../../src/fs/fat16/format";

describe("clusterCountFor", () => {
  it("reproduces the core's cluster count for the default disk and for every form choice", () => {
    expect(clusterCountFor(32768, 4)).toBe(8167);
    for (const { totalSectors } of SIZES) {
      for (const sectorsPerCluster of CLUSTER_SIZES) {
        const { clusters, problem } = checkFormat({ totalSectors, sectorsPerCluster });
        expect(clusters).toBe(clusterCountFor(totalSectors, sectorsPerCluster));
        // The form's rule and the core's agree: the button is enabled exactly when formatFat16 succeeds.
        let formatted: number | null = null;
        try { formatted = Volume.formatFat16({ totalSectors, sectorsPerCluster }).geometry().clusterCount; } catch { formatted = null; }
        expect(problem === null, `${totalSectors} sectors, ${sectorsPerCluster} per cluster`).toBe(formatted !== null);
        if (formatted !== null) expect(clusters).toBe(formatted);
      }
    }
  });
});

describe("checkFormat", () => {
  it("names the problem at both FAT16 bounds and none inside them", () => {
    const few = checkFormat({ totalSectors: 8192, sectorsPerCluster: 8 });
    expect(few.clusters).toBeLessThan(FAT16_MIN_CLUSTERS);
    expect(few.problem).toBe("too few for FAT16");
    const many = checkFormat({ totalSectors: 131072, sectorsPerCluster: 1 });
    expect(many.clusters).toBeGreaterThan(FAT16_MAX_CLUSTERS);
    expect(many.problem).toBe("too many for FAT16");
    expect(checkFormat({ totalSectors: 32768, sectorsPerCluster: 4 })).toEqual({ clusters: 8167, problem: null });
    expect(checkFormat({})).toEqual({ clusters: 8167, problem: null }); // absent options take the form's defaults
  });
});

describe("DEFAULTS and MKFS", () => {
  it("DEFAULTS reproduces the default disk's geometry", () => {
    expect(DEFAULTS).toEqual({ totalSectors: 32768, sectorsPerCluster: 4, volumeLabel: "" });
    expect(Volume.formatFat16(DEFAULTS).geometry()).toEqual(Volume.formatFat16(undefined).geometry());
  });

  it("keeps the shell's mkfs strings", () => {
    expect(MKFS.summary).toBe("Format /dev/hda as FAT16 (clears the timeline)");
    expect(MKFS.done).toBe("formatted /dev/hda as FAT16; the timeline was cleared");
    expect(MKFS.flags.map((f) => [f.long, f.kind])).toEqual([
      ["sectors", "int"], ["spc", "int"], ["label", "str"], ["root-entries", "int"], ["fats", "int"], ["reserved", "int"],
    ]);
  });

  it("every flag's option is a FormatOptions key the wasm formatter accepts", () => {
    // Listing every key of the type here is the compile-time half: a key added to or
    // dropped from FormatOptions fails this object literal.
    const keys: Record<keyof FormatOptions, true> = {
      bytesPerSector: true, sectorsPerCluster: true, totalSectors: true, fatCount: true, rootEntries: true,
      reservedSectors: true, volumeLabel: true, volumeId: true, enforceFat16Range: true,
    };
    const sample: Record<string, number | string> = { totalSectors: 32768, sectorsPerCluster: 4, volumeLabel: "X", rootEntries: 512, fatCount: 2, reservedSectors: 1 };
    expect(MKFS.flags.map((f) => f.option)).toEqual(["totalSectors", "sectorsPerCluster", "volumeLabel", "rootEntries", "fatCount", "reservedSectors"]);
    for (const f of MKFS.flags) {
      expect(keys[f.option as keyof FormatOptions], f.option).toBe(true);
      // formatFat16 rejects unknown keys, so a misspelt option would throw here.
      expect(() => Volume.formatFat16({ [f.option]: sample[f.option] }), f.option).not.toThrow();
    }
  });
});
