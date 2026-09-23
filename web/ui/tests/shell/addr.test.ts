import { describe, expect, it } from "vitest";
import type { AddrSpace } from "../../src/shell/addr";
import { SIZE_HELP, addrHelp, parseAddr, parseSize } from "../../src/shell/addr";
import { ShellError } from "../../src/shell/errors";
import { space } from "../fixtures/geometry";

function thrown(fn: () => unknown): ShellError {
  try { fn(); } catch (e) { return e as ShellError; }
  throw new Error("expected a throw");
}

/** Yesterday's `ADDR_HELP`, byte for byte: what every FAT address error and flag description shows. */
const FAT_HELP = "addresses: 0x1f (hex), 512 (decimal), s:65 (sector), c:3 (cluster)";

/** A made-up family with 1 KiB units counted from 0 and the letter `b`, to prove the noun and
 *  letter come from the space rather than from the parser. */
const blocks: AddrSpace = {
  sectorSize: 1024,
  unit: { singular: "block", plural: "blocks", letter: "b", first: 0 },
  unitByteRange: (u) => ({ start: u * 1024, end: (u + 1) * 1024 }),
};

describe("addrHelp", () => {
  it("composes the FAT help exactly as before, and another family's from its noun and letter", () => {
    expect(addrHelp(space)).toBe(FAT_HELP);
    expect(addrHelp(blocks)).toBe("addresses: 0x1f (hex), 512 (decimal), s:65 (sector), b:3 (block)");
  });
});

describe("parseAddr", () => {
  it("accepts sectors, clusters, hex, decimal strings, and integers (HexView's g prompt forms)", () => {
    expect(parseAddr("s:65", space)).toBe(65 * 512);
    expect(parseAddr("S:0", space)).toBe(0);
    expect(parseAddr("c:2", space)).toBe(97 * 512);
    expect(parseAddr("C:2", space)).toBe(97 * 512);
    expect(parseAddr("c:3", space)).toBe(97 * 512 + 2048);
    expect(parseAddr("0x1F", space)).toBe(31);
    expect(parseAddr("0X1f", space)).toBe(31);
    expect(parseAddr("512", space)).toBe(512);
    expect(parseAddr(512, space)).toBe(512);
    expect(parseAddr(0, space)).toBe(0);
  });
  it("throws a ShellError naming the input, with the composed help", () => {
    for (const bad of ["-1", "s:x", "1k", "", "c:", "0x", "12 34", "i:1"]) {
      const e = thrown(() => parseAddr(bad, space));
      expect(e).toBeInstanceOf(ShellError);
      expect(e.message).toBe(`bad address '${bad}'`);
      expect(e.help).toBe(FAT_HELP);
    }
    expect(thrown(() => parseAddr(-3, space)).message).toBe("bad address '-3'");
    expect(thrown(() => parseAddr(1.5, space)).message).toBe("bad address '1.5'");
    // c:0 lands before the data region on this geometry: 97*512 - 2*2048 is still >= 0, but a
    // tiny geometry (data from sector 4, 8 sectors per cluster) goes negative, and negative
    // is never an address.
    const tiny: AddrSpace = {
      sectorSize: 512,
      unit: space.unit,
      unitByteRange: (u) => { const start = 4 * 512 + (u - 2) * 8 * 512; return { start, end: start + 8 * 512 }; },
    };
    expect(thrown(() => parseAddr("c:0", tiny)).message).toBe("bad address 'c:0'");
  });
  it("hands anything else to the family's own parser, which can add forms such as i:N", () => {
    const withInodes: AddrSpace = {
      sectorSize: space.sectorSize,
      unit: space.unit,
      unitByteRange: (u) => space.unitByteRange(u),
      parseAddr: (v) => (v === "i:1" ? 1234 : undefined),
    };
    expect(parseAddr("i:1", withInodes)).toBe(1234);
    expect(parseAddr("c:2", withInodes)).toBe(97 * 512); // the generic forms still come first
    const e = thrown(() => parseAddr("i:2", withInodes));
    expect(e.message).toBe("bad address 'i:2'");
    expect(e.help).toBe(FAT_HELP); // the hook adds no help of its own
  });
  it("reads the unit letter from the space, so another family's b:N is a unit address", () => {
    expect(parseAddr("b:3", blocks)).toBe(3 * 1024);
    expect(parseAddr("B:0", blocks)).toBe(0);
    expect(parseAddr("s:2", blocks)).toBe(2048);
    const e = thrown(() => parseAddr("c:3", blocks));
    expect(e.message).toBe("bad address 'c:3'");
    expect(e.help).toBe("addresses: 0x1f (hex), 512 (decimal), s:65 (sector), b:3 (block)");
  });
});

describe("parseSize", () => {
  it("accepts plain, k/M-suffixed, and hex sizes, and integers", () => {
    expect(parseSize("512")).toBe(512);
    expect(parseSize("1k")).toBe(1024);
    expect(parseSize("2K")).toBe(2048);
    expect(parseSize("4M")).toBe(4 * 1048576);
    expect(parseSize("1m")).toBe(1048576);
    expect(parseSize("0x200")).toBe(512);
    expect(parseSize(512)).toBe(512);
    expect(parseSize("0")).toBe(0);
  });
  it("throws a ShellError with SIZE_HELP", () => {
    for (const bad of ["abc", "1kb", "-1", "", "k"]) {
      const e = thrown(() => parseSize(bad));
      expect(e).toBeInstanceOf(ShellError);
      expect(e.message).toBe(`bad size '${bad}'`);
      expect(e.help).toBe(SIZE_HELP);
    }
    expect(thrown(() => parseSize(-1)).message).toBe("bad size '-1'");
    expect(thrown(() => parseSize(2.5)).message).toBe("bad size '2.5'");
  });
});
