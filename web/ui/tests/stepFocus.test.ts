import { describe, expect, it } from "vitest";
import { stepFocusOffset } from "../src/core/stepFocus";

describe("stepFocusOffset", () => {
  it("returns the first change's offset", () => {
    expect(stepFocusOffset({ changes: [{ offset: 0x8200 }, { offset: 0x200 }] })).toBe(0x8200);
  });

  it("returns null for a record that changed nothing", () => {
    expect(stepFocusOffset({ changes: [] })).toBeNull();
  });

  it("returns null when there is no record (an empty or rewound-past-the-start timeline)", () => {
    expect(stepFocusOffset(null)).toBeNull();
    expect(stepFocusOffset(undefined)).toBeNull();
  });
});
