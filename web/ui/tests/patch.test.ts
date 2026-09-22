import { describe, expect, it } from "vitest";
import { applyChanges, changedSectors, touchesBootSector } from "../src/core/patch";

const c = (offset: number, before: number[], after: number[]) => ({ offset, before: new Uint8Array(before), after: new Uint8Array(after) });

describe("applyChanges", () => {
  it("forward then reverse is the identity, including overlapping writes", () => {
    const buf = new Uint8Array(32);
    const changes = [c(4, [0, 0, 0], [1, 2, 3]), c(5, [2, 3], [9, 9]), c(30, [0, 0], [7, 7])];
    applyChanges(buf, changes, "forward");
    expect(Array.from(buf.slice(4, 8))).toEqual([1, 9, 9, 0]);
    expect(Array.from(buf.slice(30))).toEqual([7, 7]);
    applyChanges(buf, changes, "reverse");
    expect(buf.every((b) => b === 0)).toBe(true);
  });
  it("lists changed sectors sorted and unique", () => {
    const changes = [c(1000, [0], [1]), c(20, [0, 0], [1, 1]), c(1020, [0], [1]), c(511, [0, 0], [1, 1])];
    expect(changedSectors(changes, 512)).toEqual([0, 1]);
  });
  it("skips changes with an empty `after`: a zero-length raw write at the end of the disk names no sector", () => {
    // 4096 is the disk length of an 8-sector disk; sector 8 does not exist.
    expect(changedSectors([c(4096, [], [])], 512)).toEqual([]);
    expect(changedSectors([c(4096, [], []), c(20, [0], [1])], 512)).toEqual([0]);
  });
});

describe("touchesBootSector", () => {
  it("is true when any change starts inside the first 512 bytes", () => {
    expect(touchesBootSector([c(17, [16, 0], [32, 0])])).toBe(true);
    expect(touchesBootSector([c(1000, [0], [1]), c(511, [0], [1])])).toBe(true);
    expect(touchesBootSector([c(512, [0], [1]), c(4096, [0], [1])])).toBe(false);
    expect(touchesBootSector([])).toBe(false);
  });
});
