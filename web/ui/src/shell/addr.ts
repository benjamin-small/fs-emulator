import { unitIsSector, type UnitSpace } from "../fs/adapter";
import { ShellError } from "./errors";

export const SIZE_HELP = "sizes: 512, 1k, 4M, 0x200";

/**
 * What an address parser needs from a family: the sector size, the sector and unit nouns and
 * letters, the unit's byte range, and optionally the family's own extra forms (`parseAddr`,
 * which answers `undefined` for anything that is not its own, and `extraAddrHelp`, the help
 * text for them). A `UnitSpace` or an `FsAdapter` satisfies it; tests build one from a fixture.
 */
export type AddrSpace = Pick<UnitSpace, "sectorSize" | "unit" | "sector" | "unitByteRange"> & {
  parseAddr?(v: string): number | undefined;
  extraAddrHelp?: string;
};

/** The address forms in one line, for error help and flag descriptions: the sector form, the
 *  unit form when the unit is not the sector, then the family's extras. For FAT this reads
 *  `addresses: 0x1f (hex), 512 (decimal), s:65 (sector), c:3 (cluster)`; for ext
 *  `addresses: 0x1f (hex), 512 (decimal), b:65 (block), i:11 (inode)`. */
export function addrHelp(space: AddrSpace): string {
  const { sector, unit } = space;
  const unitForm = unitIsSector(space) ? "" : `, ${unit.letter}:3 (${unit.singular})`;
  return `addresses: 0x1f (hex), 512 (decimal), ${sector.letter}:65 (${sector.singular})${unitForm}${space.extraAddrHelp ?? ""}`;
}

/** The address forms for a single positional argument (`seek`'s `addr`): the same forms as
 *  `addrHelp`, minus the `addresses:` prefix and with the last form joined by "or" instead of
 *  a bare comma. For FAT this reads `0x1f, 512, s:65 (sector), or c:3 (cluster)`; for ext
 *  `0x1f, 512, b:65 (block), i:11 (inode)` (the unit and the sector coincide, so there is
 *  nothing to join with "or", and `i:N` comes from `extraAddrHelp`). */
export function posAddrHelp(space: AddrSpace): string {
  const { sector, unit } = space;
  const unitForm = unitIsSector(space) ? "" : `, or ${unit.letter}:3 (${unit.singular})`;
  return `0x1f, 512, ${sector.letter}:65 (${sector.singular})${unitForm}${space.extraAddrHelp ?? ""}`;
}

/** `<letter>:N` with the family's letter, case-insensitively like `s:N`; `undefined` otherwise. */
function letterIndex(v: string, letter: string): number | undefined {
  const prefix = `${letter}:`;
  if (v.length <= prefix.length || v.slice(0, prefix.length).toLowerCase() !== prefix.toLowerCase()) return undefined;
  const digits = v.slice(prefix.length);
  return /^\d+$/.test(digits) ? Number(digits) : undefined;
}

/**
 * The address forms HexView's `g` prompt accepts, in order: `s:N` (a sector, on every family),
 * `0x…`, decimal, the family's sector letter when it is not `s` (ext's `b:N`), the family's
 * unit letter (a data unit: FAT's `c:N`, whose clusters start at 2), then whatever else the
 * family parses (`space.parseAddr`, ext's `i:N`). A number passes through when it is a
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
    const sector = space.sector.letter.toLowerCase() === "s" ? undefined : letterIndex(v, space.sector.letter);
    const unit = sector === undefined ? letterIndex(v, space.unit.letter) : undefined;
    off = sector !== undefined ? sector * space.sectorSize : unit !== undefined ? space.unitByteRange(unit).start : space.parseAddr?.(v);
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
