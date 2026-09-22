import { describe, expect, it } from "vitest";
import { findStrings } from "../src/core/strings";

const bytes = (s: string) => new TextEncoder().encode(s);

describe("findStrings", () => {
  it("finds printable runs of at least minRun", () => {
    const buf = new Uint8Array([...bytes("ab"), 0, ...bytes("hello"), 0, 0, ...bytes("wor"), 1, ...bytes("READMETXT")]);
    expect(findStrings(buf, 0, buf.length)).toEqual([
      { offset: 3, length: 5, text: "hello" },
      { offset: 14, length: 9, text: "READMETXT" },
    ]);
  });
  it("respects the range, minRun, and limit", () => {
    const buf = bytes("aaaa bbbb cccc");
    expect(findStrings(buf, 5, 9)).toEqual([{ offset: 5, length: 4, text: "bbbb" }]);
    expect(findStrings(buf, 0, buf.length, 15)).toEqual([]);
    expect(findStrings(buf, 0, buf.length, 4, 1).length).toBe(1);
  });
  it("a run touching the range end is included", () => {
    const buf = bytes("xxxxyyyy");
    expect(findStrings(buf, 0, 8, 4)).toEqual([{ offset: 0, length: 8, text: "xxxxyyyy" }]);
  });
});
