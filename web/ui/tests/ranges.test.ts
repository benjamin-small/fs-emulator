import { describe, expect, it } from "vitest";
import { formatRanges } from "../src/core/ranges";
import { changedSectors } from "../src/core/patch";
import { Volume } from "../src/lib/wasm";

const enc = (s: string) => new TextEncoder().encode(s);

describe("formatRanges", () => {
  it("folds runs of consecutive numbers into one range, a lone number into itself", () => {
    expect(formatRanges([1, 2, 3, 4, 5, 6, 69, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91])).toEqual([
      { label: "1–6", first: 1, last: 6 },
      { label: "69", first: 69, last: 69 },
      { label: "82–91", first: 82, last: 91 },
    ]);
    // The dash is an en dash, not a hyphen.
    expect(formatRanges([7, 8])[0].label).toBe("7–8");
  });

  it("gives nothing for nothing, and sector 0 its own label", () => {
    expect(formatRanges([])).toEqual([]);
    expect(formatRanges([0])).toEqual([{ label: "0", first: 0, last: 0 }]);
    expect(formatRanges([0, 2])).toEqual([{ label: "0", first: 0, last: 0 }, { label: "2", first: 2, last: 2 }]);
  });

  it("reads a real ext3 create as the What changed panel shows it", () => {
    // The default ext3 disk's Add file: the superblock and group descriptors, the bitmaps and
    // inode table (1–6), the root directory (69), the journal (82–91), and the data block.
    const vol = Volume.formatExt3(undefined);
    const rec = vol.createFile("/hello.txt", enc("Hello world\n"));
    const sectors = changedSectors(rec.changes, vol.sectorSize());
    expect(sectors).toHaveLength(18);
    expect(formatRanges(sectors).map((r) => r.label)).toEqual(["1–6", "69", "82–91", "1111"]);
  });

  it("reads a real FAT16 create: both FATs, the root directory, the data cluster", () => {
    const vol = Volume.formatFat16(undefined);
    const rec = vol.createFile("/Hello world.txt", enc("Hello, world!\n"));
    expect(formatRanges(changedSectors(rec.changes, vol.sectorSize())).map((r) => r.label)).toEqual(["1", "33", "65", "97–100"]);
  });
});
