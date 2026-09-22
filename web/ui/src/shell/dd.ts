import type { Value } from "./types";
import { ShellError } from "./errors";
import { SIZE_HELP, parseSize } from "./addr";

/**
 * Per-invocation cap on the bytes `dd` reads (and therefore writes). Every journaled byte is
 * stored twice in Rust (before/after) and again in two histories, and a blob is 2x as hex,
 * so the shell caps here rather than in the core. `cat` without `--bytes` uses the same limit.
 */
export const DD_MAX_BYTES = 1 << 20;

export const DD_KEYS = ["if", "of", "bs", "count", "skip", "seek"] as const;
export type DdKey = (typeof DD_KEYS)[number];

export interface DdOpts {
  if?: string;
  of?: string;
  bs: number;
  count?: number;
  skip: number;
  seek: number;
}

export const OPERAND_HELP =
  "operands: if= of= bs= count= skip= seek= (quote them, e.g. 'if=/dev/hda', because = is reserved by the shell; or write --if=/dev/hda)";

const isDdKey = (k: string): k is DdKey => (DD_KEYS as readonly string[]).includes(k);

/**
 * Merge `--if/--of/--bs/--count/--skip/--seek` flags (the documented form is `--if=/dev/hda`)
 * with quoted classic `'if=/dev/hda'` operands. The same key from both sources, or twice
 * among the operands, is an error; so is any operand that is not `key=value` with a known key.
 * `bs` defaults to 512 and must be positive; `count`, `skip`, `seek` take sizes too (so
 * `--count=1k` works) and must be non-negative.
 */
export function parseDd(flags: Record<string, Value>, operands: Value[]): DdOpts {
  const given = new Map<DdKey, Value>();
  const put = (key: DdKey, value: Value) => {
    if (given.has(key)) throw new ShellError(`'${key}' given twice`, { help: "give each of if/of/bs/count/skip/seek once, as a flag or a quoted operand" });
    given.set(key, value);
  };
  for (const key of DD_KEYS) if (key in flags) put(key, flags[key]);
  for (const op of operands) {
    const eq = typeof op === "string" ? op.indexOf("=") : -1;
    const key = typeof op === "string" && eq > 0 ? op.slice(0, eq) : "";
    if (typeof op !== "string" || !isDdKey(key)) throw new ShellError(`unrecognized operand '${String(op)}'`, { help: OPERAND_HELP });
    put(key, op.slice(eq + 1));
  }

  const path = (key: "if" | "of"): string | undefined => {
    if (!given.has(key)) return undefined;
    const v = given.get(key);
    if (typeof v !== "string" || v === "") throw new ShellError(`'${key}' needs a path`, { help: OPERAND_HELP });
    return v;
  };
  const size = (key: "bs" | "count" | "skip" | "seek", what: string): number | undefined => {
    if (!given.has(key)) return undefined;
    const v = given.get(key);
    const bad = new ShellError(`invalid ${what} '${String(v)}'`, { help: SIZE_HELP });
    if (typeof v !== "string" && typeof v !== "number") throw bad;
    try { return parseSize(v); } catch { throw bad; }
  };

  const bs = size("bs", "block size") ?? 512;
  if (bs === 0) throw new ShellError("invalid block size '0'", { help: SIZE_HELP });
  const opts: DdOpts = { bs, skip: size("skip", "skip") ?? 0, seek: size("seek", "seek") ?? 0 };
  const src = path("if"), dst = path("of"), count = size("count", "count");
  if (src !== undefined) opts.if = src;
  if (dst !== undefined) opts.of = dst;
  if (count !== undefined) opts.count = count;
  return opts;
}

export interface DdWindow {
  start: number;
  len: number;
}

/**
 * The read window: `skip*bs` for `count*bs` bytes, or to the end when `count` is absent,
 * clipped to `available` (the source length; `Infinity` for /dev/zero, whose callers require
 * `count`). Throws when the window exceeds DD_MAX_BYTES, before any write happens.
 */
export function planWindow(opts: Pick<DdOpts, "bs" | "count" | "skip">, available: number): DdWindow {
  const start = opts.skip * opts.bs;
  const avail = Math.max(0, available - start);
  const len = opts.count === undefined ? avail : Math.min(opts.count * opts.bs, avail);
  if (len > DD_MAX_BYTES) {
    throw new ShellError(`refusing to copy ${len} bytes in one dd; the limit is ${DD_MAX_BYTES} (1 MiB)`, {
      help: "lower --count or --bs, or copy in several runs with --skip and --seek",
    });
  }
  return { start, len };
}

/** dd's `N+P records` count: N full blocks and P (0 or 1) partial block. */
export function formatRecords(len: number, bs: number): string {
  return `${Math.floor(len / bs)}+${len % bs === 0 ? 0 : 1} records`;
}
