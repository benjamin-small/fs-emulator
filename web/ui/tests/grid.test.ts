import { describe, expect, it } from "vitest";
import { cellAt, cellRect, gridCols, gridRows } from "../src/core/grid";

describe("the canvas cell grid", () => {
  it("fits whole cells and their gaps to the width, never fewer than one column or row", () => {
    expect(gridCols(198, 6, 1)).toBe(28); // the FAT map's 6 px cells in the sidebar's 198 px
    expect(gridCols(198, 4, 1)).toBe(39); // 4 px cells
    expect(gridCols(3, 4, 1)).toBe(1);
    expect(gridCols(0, 6, 1)).toBe(1); // before the first measurement
    expect(gridRows(8167, 28)).toBe(292); // the default FAT16 disk's clusters
    expect(gridRows(1024, 39)).toBe(27);
    expect(gridRows(0, 39)).toBe(1);
  });

  it("places cell i left to right, wrapping at the column count", () => {
    expect(cellRect(0, 39, 4, 1)).toEqual({ x: 0, y: 0 });
    expect(cellRect(38, 39, 4, 1)).toEqual({ x: 190, y: 0 });
    expect(cellRect(39, 39, 4, 1)).toEqual({ x: 0, y: 5 });
    expect(cellRect(1023, 39, 4, 1)).toEqual({ x: 45, y: 130 }); // 1023 = 26 * 39 + 9
    expect(cellRect(29, 28, 6, 1)).toEqual({ x: 7, y: 7 });
  });

  it("finds the cell under a point, the gap after a cell counting as the cell", () => {
    const at = (px: number, py: number) => cellAt(px, py, 39, 1024, 4, 1);
    expect(at(0, 0)).toBe(0);
    expect(at(194, 4)).toBe(38);
    expect(at(2, 7)).toBe(39);
    expect(at(47, 132)).toBe(1023);
    expect(at(50, 130)).toBeNull(); // past the last cell on the last row
    expect(at(196, 0)).toBeNull(); // right of the last column
    expect(at(0, 135)).toBeNull(); // below the last row
    expect(at(-1, 0)).toBeNull();
    expect(at(0, -1)).toBeNull();
    for (const i of [0, 1, 38, 39, 500, 1023]) {
      const { x, y } = cellRect(i, 39, 4, 1);
      expect(at(x, y), `cell ${i}`).toBe(i);
      expect(at(x + 4, y + 4), `cell ${i}'s gap`).toBe(i);
    }
  });
});
