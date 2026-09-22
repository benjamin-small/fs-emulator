import { describe, expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";
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
