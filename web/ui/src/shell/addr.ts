import type { Geometry } from "../lib/wasm";
import { ShellError } from "./errors";

export const ADDR_HELP = "addresses: 0x1f (hex), 512 (decimal), s:65 (sector), c:3 (cluster)";
export const SIZE_HELP = "sizes: 512, 1k, 4M, 0x200";

export type AddrGeometry = Pick<Geometry, "bytesPerSector" | "sectorsPerCluster" | "firstDataSector">;

/**
 * The address forms HexView's `g` prompt accepts: `s:N` (sector), `c:N` (cluster, data
 * clusters start at 2), `0x…`, or decimal. A number passes through when it is a
 * non-negative integer. Bounds are the caller's business (HexView checks the image length,
 * `seek` the disk size).
 */
export function parseAddr(v: string | number, g: AddrGeometry): number {
  const bad = () => new ShellError(`bad address '${v}'`, { help: ADDR_HELP });
  let off: number;
  if (typeof v === "number") off = v;
  else if (/^s:\d+$/i.test(v)) off = Number(v.slice(2)) * g.bytesPerSector;
  else if (/^c:\d+$/i.test(v)) off = g.firstDataSector * g.bytesPerSector + (Number(v.slice(2)) - 2) * g.bytesPerSector * g.sectorsPerCluster;
  else if (/^0x[0-9a-f]+$/i.test(v)) off = parseInt(v, 16);
  else if (/^\d+$/.test(v)) off = Number(v);
  else throw bad();
  if (!Number.isInteger(off) || off < 0) throw bad();
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
