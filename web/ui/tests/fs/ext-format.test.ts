import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { DEFAULTS, MKFS, SIZES, checkFormat, defaultInodesPerGroup, defaultJournalBlocks, formOptions, toWasmOptions, type ExtFamilyOptions, type ExtFormFields } from "../../src/fs/ext/format";

/** What `ext.format` does with the options, without the family object. */
function formatWith(o: ExtFamilyOptions): Volume {
  const w = toWasmOptions(o);
  return w.variant === "ext2" ? Volume.formatExt2(w.options) : Volume.formatExt3(w.options);
}

describe("defaultInodesPerGroup", () => {
  it("is the core's rule: one inode per 16 KiB of each group, a multiple of 8 in 16..8192", () => {
    expect(defaultInodesPerGroup(16384)).toBe(512);
    expect(defaultInodesPerGroup(262144)).toBe(512);
    expect(defaultInodesPerGroup(64)).toBe(16);
    expect(defaultInodesPerGroup(8200)).toBe(256); // two groups, the second a runt: the volume's rate split evenly
    expect(defaultInodesPerGroup(1100)).toBe(72);
  });

  it("agrees with the formatter for every size the form offers and a few odd ones", () => {
    for (const totalBlocks of [...SIZES.map((s) => s.totalBlocks), 64, 1100, 2047, 2048, 32767]) {
      expect(Volume.formatExt2({ totalBlocks }).extGeometry().inodesPerGroup, `${totalBlocks} blocks`).toBe(defaultInodesPerGroup(totalBlocks));
    }
  });
});

describe("defaultJournalBlocks", () => {
  it("is mke2fs's table: none below 2048 blocks, then 1024, 4096, 8192", () => {
    expect(defaultJournalBlocks(2047)).toBeNull();
    expect(defaultJournalBlocks(2048)).toBe(1024);
    expect(defaultJournalBlocks(32767)).toBe(1024);
    expect(defaultJournalBlocks(32768)).toBe(4096);
    expect(defaultJournalBlocks(262143)).toBe(4096);
    expect(defaultJournalBlocks(262144)).toBe(8192);
  });

  it("agrees with the journal formatExt3 lays down", () => {
    for (const totalBlocks of [2048, ...SIZES.map((s) => s.totalBlocks)]) {
      expect(Volume.formatExt3({ totalBlocks }).journalInfo()?.maxlen, `${totalBlocks} blocks`).toBe(defaultJournalBlocks(totalBlocks));
    }
  });
});

describe("checkFormat", () => {
  it("names each problem the Format form disables its button on", () => {
    expect(checkFormat({ totalBlocks: 63 }).problem).toBe("too few blocks (minimum 64)");
    expect(checkFormat({ totalBlocks: 262145 }).problem).toBe("too many blocks (maximum 262,144)");
    expect(checkFormat({ totalBlocks: 2047 }).problem).toBe("a journal needs at least 2048 blocks"); // ext3 is the default variant
    expect(checkFormat({ variant: "ext2", totalBlocks: 2047 }).problem).toBeNull();
    expect(checkFormat({ journalBlocks: 1023 }).problem).toBe("the journal must be at least 1024 blocks");
    expect(checkFormat({ label: "a".repeat(17) }).problem).toBe("label is longer than 16 bytes");
    expect(checkFormat({ label: "é".repeat(8) }).problem).toBeNull(); // 16 bytes of UTF-8
    expect(checkFormat({ label: "é".repeat(9) }).problem).toBe("label is longer than 16 bytes"); // counted in bytes, not characters
  });

  it("finds nothing wrong with the defaults, and every problem it names the formatter refuses too", () => {
    expect(checkFormat({})).toEqual({ problem: null });
    expect(checkFormat(DEFAULTS)).toEqual({ problem: null });
    const bad: ExtFamilyOptions[] = [{ totalBlocks: 63 }, { totalBlocks: 262145 }, { totalBlocks: 2047 }, { journalBlocks: 1023 }, { label: "a".repeat(17) }];
    for (const o of bad) {
      expect(checkFormat(o).problem, JSON.stringify(o)).not.toBeNull();
      expect(() => formatWith(o), JSON.stringify(o)).toThrow();
    }
  });

  it("passes every size and variant the form offers, and the formatter agrees", () => {
    for (const { totalBlocks } of SIZES) {
      for (const variant of ["ext2", "ext3"] as const) {
        expect(checkFormat({ variant, totalBlocks }).problem).toBeNull();
        expect(formatWith({ variant, totalBlocks }).fsType()).toBe(variant);
      }
    }
  });
});

describe("the form's choices and the shell's mkfs", () => {
  it("offers four sizes and defaults to the lessons' disk", () => {
    expect(SIZES).toEqual([
      { label: "4 MB", totalBlocks: 4096 },
      { label: "16 MB", totalBlocks: 16384 },
      { label: "64 MB", totalBlocks: 65536 },
      { label: "256 MB", totalBlocks: 262144 },
    ]);
    expect(DEFAULTS).toEqual({ variant: "ext3", totalBlocks: 16384, label: "" });
    const vol = formatWith(DEFAULTS);
    expect(vol.fsType()).toBe("ext3");
    expect(vol.extGeometry()).toEqual(Volume.formatExt3(undefined).extGeometry());
  });

  it("keeps the six mkfs flags with their descriptions", () => {
    expect(MKFS).not.toHaveProperty("summary"); // the shell's mkfs has one summary for every family
    expect(MKFS.flags).toEqual([
      { long: "blocks", desc: "total 1 KiB blocks (default 16384 = 16 MB)", kind: "int", option: "totalBlocks" },
      { long: "inodes-per-group", desc: "inodes per block group (default one per 16 KiB)", kind: "int", option: "inodesPerGroup" },
      { long: "label", desc: "volume label, up to 16 bytes", kind: "str", option: "label" },
      { long: "uuid", desc: "32 hex digits, hyphens optional", kind: "str", option: "uuid" },
      { long: "journal-blocks", desc: "journal size in blocks, ext3 only", kind: "int", option: "journalBlocks" },
      { long: "journal-mode", desc: "ordered or data, ext3 only", kind: "str", option: "journalMode" },
    ]);
  });

  it("every flag's option reaches the formatter", () => {
    const sample: Record<string, number | string> = {
      totalBlocks: 4096, inodesPerGroup: 256, label: "disk", uuid: "0123456789abcdef0123456789abcdef", journalBlocks: 1024, journalMode: "data",
    };
    for (const f of MKFS.flags) {
      // formatExt3 rejects unknown keys, so a misspelt option would throw here.
      expect(() => formatWith({ [f.option]: sample[f.option] }), f.option).not.toThrow();
    }
    const vol = formatWith({ label: "disk", uuid: "0123456789abcdef0123456789abcdef", journalMode: "data" });
    expect(vol.extSuperblock()).toMatchObject({ label: "disk", uuid: "01234567-89ab-cdef-0123-456789abcdef" });
    expect(vol.journalInfo()?.mode).toBe("data");
  });
});

describe("toWasmOptions", () => {
  it("splits the variant from the formatter's options and drops absent keys", () => {
    expect(toWasmOptions({})).toEqual({ variant: "ext3", options: {} });
    expect(toWasmOptions({ variant: "ext2", totalBlocks: 4096, label: undefined })).toEqual({ variant: "ext2", options: { totalBlocks: 4096 } });
    expect(toWasmOptions({ journalBlocks: 2048, journalMode: "data", uuid: "x" })).toEqual({ variant: "ext3", options: { journalBlocks: 2048, journalMode: "data", uuid: "x" } });
  });

  it("refuses journal options on ext2", () => {
    expect(() => toWasmOptions({ variant: "ext2", journalMode: "data" })).toThrow("journalBlocks and journalMode need variant ext3");
    expect(() => toWasmOptions({ variant: "ext2", journalBlocks: 1024 })).toThrow("journalBlocks and journalMode need variant ext3");
  });
});

describe("formOptions (the Format form's fields as family options)", () => {
  const fields: ExtFormFields = { variant: "ext3", totalBlocks: 16384, inodesPerGroup: undefined, label: "", journalMode: "ordered", journalBlocks: undefined };

  it("reads a blank number input as the default and keeps the rest as typed", () => {
    expect(formOptions(fields)).toEqual({ variant: "ext3", totalBlocks: 16384, inodesPerGroup: undefined, label: "", journalMode: "ordered", journalBlocks: undefined });
    expect(toWasmOptions(formOptions(fields))).toEqual({ variant: "ext3", options: { totalBlocks: 16384, label: "", journalMode: "ordered" } });
    expect(formOptions({ ...fields, inodesPerGroup: null, journalBlocks: null })).toEqual(formOptions(fields));
    expect(formOptions({ ...fields, inodesPerGroup: Number.NaN }).inodesPerGroup).toBeUndefined();
    expect(formOptions({ ...fields, inodesPerGroup: 256, journalBlocks: 2048, journalMode: "data", label: "disk" })).toEqual({ variant: "ext3", totalBlocks: 16384, inodesPerGroup: 256, label: "disk", journalMode: "data", journalBlocks: 2048 });
  });

  it("formats the default disk from the untouched form", () => {
    const vol = formatWith(formOptions(fields));
    expect(vol.fsType()).toBe("ext3");
    expect(vol.extGeometry()).toEqual(Volume.formatExt3(undefined).extGeometry());
  });

  it("drops the journal fields on ext2, so switching the variant never trips the journal-on-ext2 error", () => {
    const two = formOptions({ ...fields, variant: "ext2", journalMode: "data", journalBlocks: 512 });
    expect(two).toEqual({ variant: "ext2", totalBlocks: 16384, inodesPerGroup: undefined, label: "" });
    expect(checkFormat(two).problem).toBeNull();
    expect(formatWith(two).fsType()).toBe("ext2");
    expect(checkFormat(formOptions({ ...fields, journalBlocks: 512 })).problem).toBe("the journal must be at least 1024 blocks");
  });
});
