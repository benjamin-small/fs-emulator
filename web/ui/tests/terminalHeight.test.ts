import { describe, expect, it } from "vitest";
import { clampHeight, TERM_DEFAULT_PX, TERM_MAX_FRACTION, TERM_MIN_PX } from "../src/core/terminalHeight";

describe("clampHeight", () => {
  it("passes an in-range height through", () => {
    expect(clampHeight(220, 1000)).toBe(220);
    expect(clampHeight(TERM_MIN_PX, 1000)).toBe(TERM_MIN_PX);
    expect(clampHeight(600, 1000)).toBe(600);
  });

  it("floors at the minimum", () => {
    expect(clampHeight(50, 1000)).toBe(TERM_MIN_PX);
    expect(clampHeight(-10, 1000)).toBe(TERM_MIN_PX);
    expect(clampHeight(0, 1000)).toBe(TERM_MIN_PX);
  });

  it("caps at 60% of the viewport, rounded down", () => {
    expect(clampHeight(900, 1000)).toBe(600);
    expect(clampHeight(900, 999)).toBe(Math.floor(999 * TERM_MAX_FRACTION));
  });

  it("lets the floor win when the viewport is shorter than 200px", () => {
    // 60% of 150 is 90, below the 120px floor; the floor keeps the prompt usable.
    expect(clampHeight(300, 150)).toBe(TERM_MIN_PX);
  });

  it("falls back to the default for a non-finite request (corrupt localStorage)", () => {
    expect(clampHeight(Number.NaN, 1000)).toBe(TERM_DEFAULT_PX);
    expect(clampHeight(Number.POSITIVE_INFINITY, 1000)).toBe(TERM_DEFAULT_PX);
  });

  it("returns an integer", () => {
    expect(clampHeight(200.4, 1000)).toBe(200);
    expect(clampHeight(200.6, 1000)).toBe(201);
  });
});
