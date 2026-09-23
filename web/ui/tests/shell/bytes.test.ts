import { describe, expect, it } from "vitest";
import { BYTES_HELP, decodeText, hasInput, toBytes } from "../../src/shell/bytes";
import { ShellError } from "../../src/shell/errors";

const u8 = (...b: number[]) => new Uint8Array(b);

describe("toBytes (the pipe convention)", () => {
  it("a Uint8Array is the bytes themselves, 0x00 and 0xff included", () => {
    const bytes = u8(0, 255, 16);
    expect(toBytes(bytes)).toEqual(bytes);
    expect(toBytes(u8())).toEqual(u8());
  });
  it("a subarray keeps only its own window, not the buffer behind it", () => {
    const view = u8(1, 2, 3, 4, 5).subarray(1, 3);
    expect(Array.from(toBytes(view))).toEqual([2, 3]);
  });
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
  it("a list holding a byte buffer is refused rather than flattened", () => {
    let caught: unknown;
    try { toBytes([u8(1, 2)] as never); } catch (e) { caught = e; }
    expect(caught).toBeInstanceOf(ShellError);
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
  });
  it("an empty byte buffer is input: `cat --bytes /dev/null > f` writes an empty file", () => {
    // Deliberately not folded in with `[]`: an empty list means the pipe was never written
    // to, while zero bytes is a value a command chose to produce.
    expect(hasInput(new Uint8Array(0))).toBe(true);
    expect(hasInput(u8(0))).toBe(true);
  });
});

describe("decodeText", () => {
  it("decodes UTF-8 and never throws on bad sequences", () => {
    expect(decodeText(u8(104, 195, 169))).toBe("hé");
    expect(decodeText(u8(0xff, 0x41))).toBe("�A");
  });
});
