import { describe, expect, it } from "vitest";
import { BYTES_HELP, decodeText, fromBytes, hasInput, hex, isBlob, toBytes, unhex } from "../../src/shell/bytes";
import { ShellError } from "../../src/shell/errors";

const u8 = (...b: number[]) => new Uint8Array(b);

describe("hex and blobs", () => {
  it("hex/unhex round trip including 0x00 and 0xff", () => {
    expect(hex(u8(0, 255, 16))).toBe("00ff10");
    expect(unhex("00ff10")).toEqual(u8(0, 255, 16));
    expect(unhex("")).toEqual(u8());
    expect(hex(u8())).toBe("");
  });
  it("fromBytes makes a lowercase-hex blob and toBytes reads it back", () => {
    const blob = fromBytes(u8(0, 255, 16));
    expect(blob).toEqual({ bytes: "00ff10", length: 3 });
    expect(isBlob(blob)).toBe(true);
    expect(toBytes(blob)).toEqual(u8(0, 255, 16));
  });
  it("isBlob rejects odd hex, uppercase, a mismatched length, and non-records", () => {
    expect(isBlob({ bytes: "abc", length: 1 })).toBe(false);
    expect(isBlob({ bytes: "AB", length: 1 })).toBe(false);
    expect(isBlob({ bytes: "ab", length: 2 })).toBe(false);
    expect(isBlob({ bytes: "", length: 0 })).toBe(true);
    expect(isBlob(null)).toBe(false);
    expect(isBlob("ab")).toBe(false);
    expect(isBlob(["ab", 1])).toBe(false);
    expect(isBlob({ bytes: 12, length: 1 })).toBe(false);
  });
});

describe("toBytes (the pipe convention)", () => {
  it("strings are UTF-8", () => {
    expect(toBytes("héllo")).toEqual(u8(104, 195, 169, 108, 108, 111));
    expect(toBytes("")).toEqual(u8());
  });
  it("numbers and booleans are their String() form", () => {
    expect(toBytes(12345)).toEqual(new TextEncoder().encode("12345"));
    expect(toBytes(true)).toEqual(new TextEncoder().encode("true"));
  });
  it("a list of scalars is joined by one space, exactly what `echo a b` produces", () => {
    expect(decodeText(toBytes(["hello", "world"]))).toBe("hello world");
    expect(decodeText(toBytes([72, 105]))).toBe("72 105"); // never a byte list
    expect(decodeText(toBytes(["a", 1, true, null]))).toBe("a 1 true ");
    expect(toBytes([])).toEqual(u8());
  });
  it("null (nothing piped) is empty", () => {
    expect(toBytes(null)).toEqual(u8());
  });
  it("records and nested lists throw a ShellError with help", () => {
    for (const v of [{ a: 1 }, [["x"]], [{ a: 1 }]]) {
      let caught: unknown;
      try { toBytes(v as never); } catch (e) { caught = e; }
      expect(caught).toBeInstanceOf(ShellError);
      expect((caught as ShellError).message).toMatch(/^expected text or bytes, found /);
      expect((caught as ShellError).help).toBe(BYTES_HELP);
    }
    let caught: unknown;
    try { toBytes({ a: 1 }); } catch (e) { caught = e; }
    expect((caught as ShellError).message).toBe("expected text or bytes, found a record");
    try { toBytes([["x"]]); } catch (e) { caught = e; }
    expect((caught as ShellError).message).toBe("expected text or bytes, found a list with nested values");
  });
});

describe("hasInput", () => {
  it("an empty list, null, and undefined are all 'nothing piped'", () => {
    expect(hasInput([])).toBe(false);
    expect(hasInput(null)).toBe(false);
    expect(hasInput(undefined)).toBe(false);
  });
  it("anything else, including an empty string, is real input", () => {
    expect(hasInput("")).toBe(true);
    expect(hasInput([0])).toBe(true);
    expect(hasInput(["a"])).toBe(true);
    expect(hasInput(fromBytes(new Uint8Array()))).toBe(true);
  });
});

describe("decodeText", () => {
  it("decodes UTF-8 and never throws on bad sequences", () => {
    expect(decodeText(u8(104, 195, 169))).toBe("hé");
    expect(decodeText(u8(0xff, 0x41))).toBe("�A");
  });
});
