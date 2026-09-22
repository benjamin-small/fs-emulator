import { describe, expect, it } from "vitest";
import { rescanSectors, scanZeroSectors } from "../src/core/zeros";

describe("zero sectors", () => {
  it("marks all-zero sectors and rescans only the sectors asked", () => {
    const image = new Uint8Array(512 * 6);
    image[512 * 1 + 100] = 1;
    image[512 * 4 + 511] = 0xff;
    const bits = scanZeroSectors(image, 512);
    expect(Array.from(bits)).toEqual([1, 0, 1, 1, 0, 1]);
    image[512 * 1 + 100] = 0;
    image[512 * 2] = 5;
    rescanSectors(bits, image, 512, [1]);
    expect(Array.from(bits)).toEqual([1, 1, 1, 1, 0, 1]);
    rescanSectors(bits, image, 512, [2]);
    expect(Array.from(bits)).toEqual([1, 1, 0, 1, 0, 1]);
  });
});
