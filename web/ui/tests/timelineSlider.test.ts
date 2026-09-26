import { describe, expect, it } from "vitest";
import { stepAtPointer } from "../src/core/timelineSlider";

describe("stepAtPointer", () => {
  it("picks the nearest of the ticks spread across the width", () => {
    // Seven steps over 600 px: a tick every 100 px, at 0, 100, …, 600.
    expect(stepAtPointer(0, 600, 7)).toBe(0);
    expect(stepAtPointer(49, 600, 7)).toBe(0);
    expect(stepAtPointer(51, 600, 7)).toBe(1);
    expect(stepAtPointer(149, 600, 7)).toBe(1);
    expect(stepAtPointer(151, 600, 7)).toBe(2);
    expect(stepAtPointer(600, 600, 7)).toBe(6);
    // Two steps: the halfway point is where the second takes over.
    expect(stepAtPointer(99, 200, 2)).toBe(0);
    expect(stepAtPointer(101, 200, 2)).toBe(1);
  });

  it("clamps a pointer past either end of the slider to the first or last step", () => {
    expect(stepAtPointer(-20, 600, 7)).toBe(0);
    expect(stepAtPointer(640, 600, 7)).toBe(6);
  });

  it("is step 0 with one step or none, or before the slider has a width", () => {
    expect(stepAtPointer(300, 600, 1)).toBe(0);
    expect(stepAtPointer(300, 600, 0)).toBe(0);
    expect(stepAtPointer(10, 0, 7)).toBe(0);
  });
});
