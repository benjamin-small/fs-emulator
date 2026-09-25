import { describe, expect, it } from "vitest";
import { describeRange, formatRange, formatValue, offsetDigits } from "../src/core/annotationFormat";

describe("offsetDigits", () => {
  it("is 3 for the usual sector sizes and grows with them", () => {
    expect(offsetDigits(512)).toBe(3); // 0x1ff
    expect(offsetDigits(4096)).toBe(3); // 0xfff
    expect(offsetDigits(65536)).toBe(4); // 0xffff
  });

  it("never drops below two digits", () => {
    expect(offsetDigits(256)).toBe(2); // 0xff
    expect(offsetDigits(16)).toBe(2);
    expect(offsetDigits(0)).toBe(2);
  });
});

describe("formatRange", () => {
  it("prints both bounds in padded hex, end exclusive", () => {
    expect(formatRange({ start: 11, end: 13 }, 512)).toBe("0x00b..0x00d");
    expect(formatRange({ start: 0, end: 3 }, 512)).toBe("0x000..0x003");
    expect(formatRange({ start: 510, end: 512 }, 512)).toBe("0x1fe..0x200");
  });

  it("pads to the sector's width", () => {
    expect(formatRange({ start: 11, end: 13 }, 65536)).toBe("0x000b..0x000d");
  });
});

describe("describeRange", () => {
  it("gives the decimal bounds for the tooltip", () => {
    expect(describeRange({ start: 11, end: 13 })).toBe("bytes 11..13 of the sector (decimal)");
  });

  it("names the family's sector when given one", () => {
    expect(describeRange({ start: 0x38, end: 0x3a }, "block")).toBe("bytes 56..58 of the block (decimal)");
  });
});

describe("formatValue", () => {
  it("shows a decimal integer in hex with the decimal as the tooltip", () => {
    expect(formatValue("512")).toEqual({ text: "0x200", title: "512 (decimal)" });
    expect(formatValue("2")).toEqual({ text: "0x2", title: "2 (decimal)" });
    expect(formatValue("0")).toEqual({ text: "0x0", title: "0 (decimal)" });
  });

  it("keeps 64-bit values exact", () => {
    expect(formatValue("18446744073709551615").text).toBe("0xffffffffffffffff");
  });

  it("leaves everything that is not a plain decimal integer alone, without a tooltip", () => {
    for (const v of ["FAT16   ", "EB 3C 90", "0xF8", "0x1234ABCD", "unused", "next → 5", "free", "2026-09-23 10:42:11", "", "-1", "1.5"]) {
      expect(formatValue(v)).toEqual({ text: v, title: null });
    }
  });
});
