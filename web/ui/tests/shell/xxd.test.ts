import { describe, expect, it } from "vitest";
import { ShellError } from "../../src/shell/errors";
import { formatXxd } from "../../src/shell/xxd";

const text = (s: string) => new TextEncoder().encode(s);

describe("formatXxd", () => {
  it("prints the classic xxd layout: 8-digit address, byte pairs, padded hex area, ASCII", () => {
    const bytes = new Uint8Array([...text("Hello, FAT16!"), 0, 0, 0, 0, 0]); // 18 bytes: two rows
    expect(formatXxd(bytes, 0)).toBe(
      "00000000: 4865 6c6c 6f2c 2046 4154 3136 2100 0000  Hello, FAT16!...\n" +
      "00000010: 0000                                     ..",
    );
  });
  it("pads a single short row so the ASCII column stays aligned", () => {
    expect(formatXxd(text("Hello, FAT16!"), 0)).toBe("00000000: 4865 6c6c 6f2c 2046 4154 3136 21         Hello, FAT16!");
  });
  it("uses the base as the absolute address and honours cols", () => {
    const bytes = Uint8Array.from({ length: 20 }, (_, i) => i);
    expect(formatXxd(bytes, 0x8200, 8)).toBe(
      "00008200: 0001 0203 0405 0607  ........\n" +
      "00008208: 0809 0a0b 0c0d 0e0f  ........\n" +
      "00008210: 1011 1213            ....",
    );
    expect(formatXxd(text("abc"), 0, 3)).toBe("00000000: 6162 63  abc");
  });
  it("shows non-printables as . and empty input as an empty string", () => {
    expect(formatXxd(new Uint8Array([0x1b, 0x7f, 0x20, 0x7e, 0xff]), 0)).toBe("00000000: 1b7f 207e ff                             .. ~.");
    expect(formatXxd(new Uint8Array(0), 0)).toBe("");
  });
  it("widens the address past 8 digits when needed and rejects bad cols", () => {
    expect(formatXxd(new Uint8Array([1]), 0x1_0000_0000)).toBe("100000000: 01                                       .");
    for (const cols of [0, 65, 1.5, -1]) {
      let caught: unknown;
      try { formatXxd(new Uint8Array([1]), 0, cols); } catch (e) { caught = e; }
      expect(caught).toBeInstanceOf(ShellError);
      expect((caught as ShellError).message).toBe("--cols must be 1..64");
    }
  });
});
