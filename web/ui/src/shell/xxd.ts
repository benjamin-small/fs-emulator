import { ShellError } from "./errors";

/**
 * Classic `xxd` layout, lines joined by "\n" with no trailing newline:
 *
 *   00000000: 4865 6c6c 6f2c 2046 4154 3136 2100 0000  Hello, FAT16!...
 *   00000010: 0000                                     ..
 *
 * Address = `base + row offset` as at least 8 lowercase hex digits; bytes in pairs with one
 * space between pairs; the hex area is padded to a full row so the ASCII column aligns; two
 * spaces; ASCII with `.` for anything outside 0x20..0x7e (the renderer would strip control
 * bytes anyway). `cols` is the bytes per row, 1..64.
 */
export function formatXxd(bytes: Uint8Array, base: number, cols = 16): string {
  if (!Number.isInteger(cols) || cols < 1 || cols > 64) throw new ShellError("--cols must be 1..64");
  const width = cols * 2 + Math.ceil(cols / 2) - 1;
  const lines: string[] = [];
  for (let i = 0; i < bytes.length; i += cols) {
    const row = bytes.subarray(i, i + cols);
    const pairs: string[] = [];
    for (let j = 0; j < row.length; j += 2) {
      let pair = row[j].toString(16).padStart(2, "0");
      if (j + 1 < row.length) pair += row[j + 1].toString(16).padStart(2, "0");
      pairs.push(pair);
    }
    let ascii = "";
    for (let j = 0; j < row.length; j++) ascii += row[j] >= 0x20 && row[j] <= 0x7e ? String.fromCharCode(row[j]) : ".";
    lines.push(`${(base + i).toString(16).padStart(8, "0")}: ${pairs.join(" ").padEnd(width)}  ${ascii}`);
  }
  return lines.join("\n");
}
