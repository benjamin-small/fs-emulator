import { describe, expect, it } from "vitest";
import { BYTES_PER_ROW, buildSegments, offsetToRow, rowAt, rowToOffset, rowsPerSector, totalRows } from "../src/core/segments";

const bits = (s: string) => Uint8Array.from(s, (c) => (c === "0" ? 1 : 0)); // "0" = zero sector, "x" = data
const RPS = rowsPerSector(512); // 32

describe("segments", () => {
  it("collapses runs of at least minRun, keeps shorter runs as rows", () => {
    const segs = buildSegments(bits("xx000x00000000xx"), { minRun: 4, pinned: new Set() });
    expect(segs.map((s) => [s.kind, s.startSector, s.sectorCount, s.rowCount])).toEqual([
      ["rows", 0, 6, 6 * RPS],   // xx000x (the 3-run is shorter than minRun)
      ["gap", 6, 8, 1],
      ["rows", 14, 2, 2 * RPS],
    ]);
    expect(totalRows(segs)).toBe(8 * RPS + 1);
    expect(segs[1].firstRow).toBe(6 * RPS);
  });
  it("collapses at the start and end of the disk", () => {
    const segs = buildSegments(bits("0000x0000"), { minRun: 4, pinned: new Set() });
    expect(segs.map((s) => s.kind)).toEqual(["gap", "rows", "gap"]);
  });
  it("a pinned sector splits a run and is never collapsed", () => {
    const segs = buildSegments(bits("0000000000"), { minRun: 4, pinned: new Set([5]) });
    expect(segs.map((s) => [s.kind, s.startSector, s.sectorCount])).toEqual([["gap", 0, 5], ["rows", 5, 1], ["gap", 6, 4]]);
  });
  it("maps rows to offsets and back", () => {
    const segs = buildSegments(bits("xx0000000000xx"), { minRun: 4, pinned: new Set() });
    for (const row of [0, 1, RPS, 2 * RPS - 1, 2 * RPS, 2 * RPS + 1, totalRows(segs) - 1]) {
      const off = rowToOffset(segs, 512, row);
      expect(offsetToRow(segs, 512, off)).toBe(row);
    }
    expect(rowToOffset(segs, 512, 3)).toBe(3 * BYTES_PER_ROW);
    expect(rowAt(segs, 2 * RPS).segment.kind).toBe("gap");
    expect(rowToOffset(segs, 512, 2 * RPS)).toBe(2 * 512);
    expect(offsetToRow(segs, 512, 7 * 512 + 100)).toBe(2 * RPS);        // inside the gap -> the gap row
    expect(offsetToRow(segs, 512, 12 * 512 + 16)).toBe(2 * RPS + 2);   // first row after the gap is 2*RPS+1
  });
});
