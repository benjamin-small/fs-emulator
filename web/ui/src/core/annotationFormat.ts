/**
 * How the Inspector prints a sector annotation. Offsets are hex, like the dump's address
 * column, so a learner can match a field against the bytes on screen; a value the emulator
 * rendered as a plain decimal integer is shown in hex too. The decimal stays one hover away
 * as a tooltip.
 */
export interface ByteRange {
  start: number;
  end: number;
}

/** Hex digits an offset within a sector needs: 3 for 512-byte sectors (0x000..0x1ff). */
export function offsetDigits(sectorSize: number): number {
  return Math.max(2, (Math.max(1, sectorSize) - 1).toString(16).length);
}

/** `0x00b..0x00d` for bytes 11 and 12 of a 512-byte sector. The end stays exclusive, as
 *  the Rust side and the dump's ranges have it. */
export function formatRange(r: ByteRange, sectorSize: number): string {
  const w = offsetDigits(sectorSize);
  return `0x${r.start.toString(16).padStart(w, "0")}..0x${r.end.toString(16).padStart(w, "0")}`;
}

/** The tooltip behind a range: the same bounds in decimal, in the family's sector noun (the
 *  Inspector passes `sector.singular`: "sector" on FAT, "block" on ext). */
export function describeRange(r: ByteRange, sector = "sector"): string {
  return `bytes ${r.start}..${r.end} of the ${sector} (decimal)`;
}

const DECIMAL = /^\d+$/;

/**
 * A value the emulator rendered as a plain decimal integer (`512`, `2`, `4096`) becomes hex
 * with the decimal in the tooltip. Anything else (`FAT16   `, `EB 3C 90`, `0xF8`, `unused`,
 * a date, `next → 5`) is shown as it came, with no tooltip. BigInt, so a 64-bit field never
 * loses digits on the way through.
 */
export function formatValue(value: string): { text: string; title: string | null } {
  if (!DECIMAL.test(value)) return { text: value, title: null };
  return { text: `0x${BigInt(value).toString(16)}`, title: `${value} (decimal)` };
}
