import { describe, expect, it } from "vitest";
import { ADDR_HELP, SIZE_HELP, parseAddr, parseSize } from "../../src/shell/addr";
import { ShellError } from "../../src/shell/errors";
import { geo } from "../fixtures/geometry";

function thrown(fn: () => unknown): ShellError {
  try { fn(); } catch (e) { return e as ShellError; }
  throw new Error("expected a throw");
}

describe("parseAddr", () => {
  it("accepts sectors, clusters, hex, decimal strings, and integers (HexView's g prompt forms)", () => {
    expect(parseAddr("s:65", geo)).toBe(65 * 512);
    expect(parseAddr("S:0", geo)).toBe(0);
    expect(parseAddr("c:2", geo)).toBe(97 * 512);
    expect(parseAddr("c:3", geo)).toBe(97 * 512 + 2048);
    expect(parseAddr("0x1F", geo)).toBe(31);
    expect(parseAddr("0X1f", geo)).toBe(31);
    expect(parseAddr("512", geo)).toBe(512);
    expect(parseAddr(512, geo)).toBe(512);
    expect(parseAddr(0, geo)).toBe(0);
  });
  it("throws a ShellError naming the input, with ADDR_HELP", () => {
    for (const bad of ["-1", "s:x", "1k", "", "c:", "0x", "12 34"]) {
      const e = thrown(() => parseAddr(bad, geo));
      expect(e).toBeInstanceOf(ShellError);
      expect(e.message).toBe(`bad address '${bad}'`);
      expect(e.help).toBe(ADDR_HELP);
    }
    expect(thrown(() => parseAddr(-3, geo)).message).toBe("bad address '-3'");
    expect(thrown(() => parseAddr(1.5, geo)).message).toBe("bad address '1.5'");
    // c:0 lands before the data region on this geometry: 97*512 - 2*2048 is still >= 0, but a
    // tiny geometry can go negative, and negative is never an address.
    expect(thrown(() => parseAddr("c:0", { bytesPerSector: 512, sectorsPerCluster: 8, firstDataSector: 4 })).message).toBe("bad address 'c:0'");
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
