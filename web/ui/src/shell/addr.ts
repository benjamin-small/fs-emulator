import type { UnitSpace } from "../fs/adapter";
import { ShellError } from "./errors";

export const SIZE_HELP = "sizes: 512, 1k, 4M, 0x200";

/**
 * What an address parser needs from a family: the sector size, the unit noun and letter, the
 * unit's byte range, and optionally the family's own extra forms (`parseAddr`, which answers
 * `undefined` for anything that is not its own). A `UnitSpace` or an `FsAdapter` satisfies
 * it; tests build one from a fixture.
 */
export type AddrSpace = Pick<UnitSpace, "sectorSize" | "unit" | "unitByteRange"> & { parseAddr?(v: string): number | undefined };

/** The address forms in one line, for error help and flag descriptions. For FAT this reads
 *  `addresses: 0x1f (hex), 512 (decimal), s:65 (sector), c:3 (cluster)`. */
export function addrHelp(space: AddrSpace): string {
  return `addresses: 0x1f (hex), 512 (decimal), s:65 (sector), ${space.unit.letter}:3 (${space.unit.singular})`;
}

/** `<letter>:N` with the family's letter, case-insensitively like `s:N`; `undefined` otherwise. */
function unitIndex(v: string, letter: string): number | undefined {
  const prefix = `${letter}:`;
  if (v.length <= prefix.length || v.slice(0, prefix.length).toLowerCase() !== prefix.toLowerCase()) return undefined;
  const digits = v.slice(prefix.length);
  return /^\d+$/.test(digits) ? Number(digits) : undefined;
}

/**
 * The address forms HexView's `g` prompt accepts: `s:N` (sector), `0x…`, decimal, the
 * family's `<letter>:N` (a data unit: FAT's `c:N`, whose clusters start at 2), then whatever
 * else the family parses (`space.parseAddr`). A number passes through when it is a
 * non-negative integer. Bounds are the caller's business (HexView checks the image length,
 * `seek` the disk size).
 */
export function parseAddr(v: string | number, space: AddrSpace): number {
  const bad = () => new ShellError(`bad address '${v}'`, { help: addrHelp(space) });
  let off: number | undefined;
  if (typeof v === "number") off = v;
  else if (/^s:\d+$/i.test(v)) off = Number(v.slice(2)) * space.sectorSize;
  else if (/^0x[0-9a-f]+$/i.test(v)) off = parseInt(v, 16);
  else if (/^\d+$/.test(v)) off = Number(v);
  else {
    const unit = unitIndex(v, space.unit.letter);
    off = unit !== undefined ? space.unitByteRange(unit).start : space.parseAddr?.(v);
  }
  if (off === undefined || !Number.isInteger(off) || off < 0) throw bad();
  return off;
}

/** Byte counts: `512`, `1k`, `4M`, `0x200`, or a non-negative integer. Zero is allowed; callers decide. */
export function parseSize(v: string | number): number {
  const bad = () => new ShellError(`bad size '${v}'`, { help: SIZE_HELP });
  let n: number;
  if (typeof v === "number") n = v;
  else {
    const m = /^(\d+)([kKmM]?)$/.exec(v);
    if (m) n = Number(m[1]) * (m[2] === "" ? 1 : m[2].toLowerCase() === "k" ? 1024 : 1048576);
    else if (/^0x[0-9a-f]+$/i.test(v)) n = parseInt(v, 16);
    else throw bad();
  }
  if (!Number.isInteger(n) || n < 0) throw bad();
  return n;
}
