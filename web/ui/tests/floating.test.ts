import { describe, expect, it } from "vitest";
import { clampPosition, defaultPosition, FLOAT_MARGIN, NUDGE_FAST_PX, NUDGE_PX, nudge, parseStoredPosition } from "../src/core/floating";

const card = { w: 340, h: 260 };
const viewport = { w: 1400, h: 900 };

describe("clampPosition", () => {
  it("passes an on-screen position through, rounded", () => {
    expect(clampPosition({ x: 100.4, y: 50.6 }, card, viewport)).toEqual({ x: 100, y: 51 });
  });

  it("keeps the margin on every side", () => {
    expect(clampPosition({ x: -50, y: -50 }, card, viewport)).toEqual({ x: FLOAT_MARGIN, y: FLOAT_MARGIN });
    expect(clampPosition({ x: 5000, y: 5000 }, card, viewport)).toEqual({ x: 1400 - 340 - FLOAT_MARGIN, y: 900 - 260 - FLOAT_MARGIN });
  });

  it("keeps the top-left corner reachable when the card is bigger than the viewport", () => {
    expect(clampPosition({ x: 300, y: 300 }, { w: 500, h: 700 }, { w: 400, h: 600 })).toEqual({ x: FLOAT_MARGIN, y: FLOAT_MARGIN });
  });
});

describe("defaultPosition", () => {
  it("starts at the top right, inset from the edge, at the given top", () => {
    expect(defaultPosition(card, viewport, 140)).toEqual({ x: 1400 - 340 - 16, y: 140 });
  });

  it("is clamped like any other position", () => {
    expect(defaultPosition(card, { w: 300, h: 900 }, 140)).toEqual({ x: FLOAT_MARGIN, y: 140 });
  });
});

describe("parseStoredPosition", () => {
  it("returns a stored {x, y}", () => {
    expect(parseStoredPosition('{"x":120,"y":80}')).toEqual({ x: 120, y: 80 });
  });

  it("treats anything else as not moved yet", () => {
    for (const raw of [null, "", "nope", "[1,2]", "null", '{"x":"120","y":80}', '{"x":120}', '{"x":null,"y":1}', '{"x":1e999,"y":1}']) {
      expect(parseStoredPosition(raw)).toBeNull();
    }
  });
});

describe("nudge", () => {
  const at = { x: 100, y: 100 };
  it("moves by the small step on an arrow key", () => {
    expect(nudge(at, "ArrowLeft", false)).toEqual({ x: 100 - NUDGE_PX, y: 100 });
    expect(nudge(at, "ArrowRight", false)).toEqual({ x: 100 + NUDGE_PX, y: 100 });
    expect(nudge(at, "ArrowUp", false)).toEqual({ x: 100, y: 100 - NUDGE_PX });
    expect(nudge(at, "ArrowDown", false)).toEqual({ x: 100, y: 100 + NUDGE_PX });
  });

  it("moves by the big step with Shift", () => {
    expect(nudge(at, "ArrowDown", true)).toEqual({ x: 100, y: 100 + NUDGE_FAST_PX });
  });

  it("ignores every other key", () => {
    for (const key of ["Enter", "Escape", "a", "n", "Home"]) expect(nudge(at, key, false)).toBeNull();
  });
});
