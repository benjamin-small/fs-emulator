import { describe, expect, it } from "vitest";
import { contains, intervalsToSectors, normalize } from "../src/core/intervals";

describe("intervals", () => {
  it("normalizes overlapping and adjacent ranges", () => {
    expect(normalize([{ start: 10, end: 20 }, { start: 0, end: 5 }, { start: 15, end: 25 }, { start: 25, end: 30 }])).toEqual([{ start: 0, end: 5 }, { start: 10, end: 30 }]);
  });
  it("contains uses half-open ranges", () => {
    const l = normalize([{ start: 10, end: 20 }, { start: 40, end: 41 }]);
    expect(contains(l, 9)).toBe(false); expect(contains(l, 10)).toBe(true); expect(contains(l, 19)).toBe(true);
    expect(contains(l, 20)).toBe(false); expect(contains(l, 40)).toBe(true); expect(contains([], 0)).toBe(false);
  });
  it("maps to sectors", () => {
    expect([...intervalsToSectors([{ start: 500, end: 1030 }], 512)]).toEqual([0, 1, 2]);
  });
});
