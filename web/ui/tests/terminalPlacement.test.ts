import { describe, expect, it } from "vitest";
import {
  clampWidth,
  parseStoredWidth,
  placementFor,
  TERM_SIDE_MIN_VIEWPORT,
  TERM_W_DEFAULT_PX,
  TERM_W_MAX_FRACTION,
  TERM_W_MIN_PX,
} from "../src/core/terminalPlacement";

describe("placementFor", () => {
  it("puts the terminal down the right side of a wide viewport", () => {
    expect(placementFor(TERM_SIDE_MIN_VIEWPORT)).toBe("side");
    expect(placementFor(1920)).toBe("side");
    expect(placementFor(2560)).toBe("side");
  });

  it("keeps the bottom drawer below the threshold", () => {
    expect(placementFor(TERM_SIDE_MIN_VIEWPORT - 1)).toBe("bottom");
    expect(placementFor(1280)).toBe("bottom");
    expect(placementFor(0)).toBe("bottom");
  });
});

describe("clampWidth", () => {
  it("passes an in-range width through", () => {
    expect(clampWidth(500, 1920)).toBe(500);
    expect(clampWidth(TERM_W_MIN_PX, 1920)).toBe(TERM_W_MIN_PX);
    expect(clampWidth(960, 1920)).toBe(960);
  });

  it("floors at the minimum", () => {
    expect(clampWidth(100, 1920)).toBe(TERM_W_MIN_PX);
    expect(clampWidth(-10, 1920)).toBe(TERM_W_MIN_PX);
    expect(clampWidth(0, 1920)).toBe(TERM_W_MIN_PX);
  });

  it("caps at half the viewport, rounded down", () => {
    expect(clampWidth(1500, 1920)).toBe(960);
    expect(clampWidth(1500, 1919)).toBe(Math.floor(1919 * TERM_W_MAX_FRACTION));
  });

  it("falls back to the default for a non-finite request", () => {
    expect(clampWidth(Number.NaN, 1920)).toBe(TERM_W_DEFAULT_PX);
    expect(clampWidth(Number.POSITIVE_INFINITY, 1920)).toBe(TERM_W_DEFAULT_PX);
  });

  it("returns an integer", () => {
    expect(clampWidth(500.4, 1920)).toBe(500);
    expect(clampWidth(500.6, 1920)).toBe(501);
  });
});

describe("parseStoredWidth", () => {
  it("returns the stored number, unclamped", () => {
    expect(parseStoredWidth("640")).toBe(640);
    expect(parseStoredWidth(" 640 ")).toBe(640);
    expect(parseStoredWidth("1200")).toBe(1200);
    expect(clampWidth(parseStoredWidth("1200"), 1600)).toBe(800);
    expect(clampWidth(parseStoredWidth("1200"), 2560)).toBe(1200);
  });

  it("treats an absent, empty, or non-numeric value as unset", () => {
    expect(parseStoredWidth(null)).toBe(TERM_W_DEFAULT_PX);
    expect(parseStoredWidth("")).toBe(TERM_W_DEFAULT_PX);
    expect(parseStoredWidth("wide")).toBe(TERM_W_DEFAULT_PX);
    expect(parseStoredWidth("NaN")).toBe(TERM_W_DEFAULT_PX);
  });
});
