import { describe, expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";
import { ScenarioCursor } from "../src/core/scenarioCursor";
import { all } from "../src/scenarios";

// Steps titled "Expect: ..." are the scenario's deliberate failure demonstrations; each
// one names the error code its action must throw.
const EXPECTED_ERROR_CODE: Record<string, string> = {
  "Expect: disk full": "DiskFull",
  "Expect: not empty": "DirectoryNotEmpty",
};

describe("scenario scripts", () => {
  it("every scenario has at least 3 steps, each with non-empty text", () => {
    for (const s of all) {
      expect(s.steps.length).toBeGreaterThanOrEqual(3);
      for (const step of s.steps) expect(step.text.trim().length).toBeGreaterThan(0);
    }
  });

  // The runner itself needs runes, so the step bookkeeping it drives lives in a plain
  // class (src/core/scenarioCursor.ts) that can be exercised here.
  describe("step cursor", () => {
    it("runs each step once; going back and forward again only seeks", () => {
      const c = new ScenarioCursor(3);
      expect(c.canRunNext()).toBe(true);
      expect(c.next()).toEqual({ run: true });   // step 0
      c.advance(0);                              // volume.cursor after step 0
      expect(c.next()).toEqual({ run: true });   // step 1
      c.advance(1);                              // volume.cursor after step 1
      expect(c.index).toBe(1);

      expect(c.back()).toBe(0);                  // prev -> step 0's disk
      expect(c.index).toBe(0);
      expect(c.canRunNext()).toBe(false);
      expect(c.next()).toEqual({ seekTo: 1 });   // next -> step 1's disk, not a third run
      expect(c.index).toBe(1);
    });

    it("clamps at both ends", () => {
      const c = new ScenarioCursor(1);
      expect(c.back()).toBeNull();
      expect(c.next()).toEqual({ run: true });
      c.advance(7);
      expect(c.next()).toBeNull();               // past the last step
      expect(c.index).toBe(0);
      expect(c.back()).toBeNull();               // already at the first step
      expect(c.index).toBe(0);
    });
  });

  for (const s of all) {
    it(`runs "${s.title}" end to end against a fresh Volume`, () => {
      let vol = Volume.formatFat16(undefined);
      for (const step of s.steps) {
        const run = () => {
          if (step.format) vol = Volume.formatFat16(step.format);
          if (step.action) step.action(vol);
          if (typeof step.focus === "function") step.focus(vol);
        };
        const expectedCode = EXPECTED_ERROR_CODE[step.title];
        if (step.title.startsWith("Expect:")) {
          expect(expectedCode, `unrecognized "Expect:" step title: ${step.title}`).toBeDefined();
          expect(run).toThrow(expect.objectContaining({ code: expectedCode }));
        } else {
          expect(run).not.toThrow();
        }
      }
    });
  }
});
