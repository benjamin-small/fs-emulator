import type { CommandCtx, Value } from "./types";
import { SIZE_HELP, parseSize } from "./addr";
import { fromBytes, toBytes, type BytesBlob } from "./bytes";
import { ShellError, wrapFs } from "./errors";
import type { ShellHost } from "./host";
import { canonicalize, type Vfs } from "./vfs";

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

/** The bytes `dd` will copy: the source window `skip*bs` for `count*bs` (or to the end). */
function readSource(host: ShellHost, vfs: Vfs, opts: DdOpts, input: Value): Uint8Array {
  const slice = (all: Uint8Array): Uint8Array => {
    const { start, len } = planWindow(opts, all.length);
    return all.subarray(Math.min(start, all.length), Math.min(start, all.length) + len);
  };
  if (opts.if === undefined) {
    if (input === null) {
      throw new ShellError("no input", { help: "give --if=<path> or pipe bytes in, e.g. cat --bytes /mnt/a | dd --of=/mnt/b" });
    }
    return slice(toBytes(input));
  }
  if (input !== null) {
    throw new ShellError("both --if and piped input given", { help: "drop --if to copy the piped bytes, or drop the pipe" });
  }
  const src = vfs.resolve(opts.if);
  const display = vfs.display(src);
  switch (src.kind) {
    case "raw": {
      const { start, len } = planWindow(opts, host.vol.sectorCount() * host.vol.sectorSize());
      return len === 0 ? new Uint8Array(0) : host.vol.readRaw(start, len);
    }
    case "zero": {
      if (opts.count === undefined) throw new ShellError("/dev/zero is endless; give --count");
      return new Uint8Array(planWindow(opts, Infinity).len);
    }
    case "null":
      return new Uint8Array(0);
    case "volume": {
      let file: Uint8Array;
      try {
        file = host.vol.readFile(src.path);
      } catch (e) {
        throw wrapFs(display, e);
      }
      return slice(file);
    }
    default:
      throw new ShellError(`${display}: Is a directory`);
  }
}

/** Overlay `data` at `off` on the file at `path` (zero-padded; FAT has no partial writes). */
function writeVolumeFile(host: ShellHost, path: string, display: string, off: number, data: Uint8Array, ctx: CommandCtx): void {
  let existing: Uint8Array | null = null;
  try {
    existing = host.vol.readFile(path);
  } catch (e) {
    if ((e as { code?: string }).code !== "NotFound") throw wrapFs(display, e);
  }
  const out = new Uint8Array(Math.max(existing?.length ?? 0, off + data.length));
  if (existing) out.set(existing, 0);
  out.set(data, off);
  const had = existing !== null;
  try {
    host.run((v) => (had ? v.writeFile(path, out) : v.createFile(path, out)));
  } catch (e) {
    throw wrapFs(display, e);
  }
  if (had || off > 0) ctx.log("FAT has no partial writes; the whole file was rewritten");
  host.select(canonicalize(host.vol, path));
}

/**
 * Run one parsed `dd`. Reads the source window (capped at `DD_MAX_BYTES` before anything is
 * written), copies it to `--of` (`/dev/hda` via `writeRaw`, a volume file via overlay and
 * rewrite, `/dev/null` discards) or returns it as a blob when `--of` is absent, then logs
 * `records in`, `records out` and `bytes copied` like the real tool.
 */
export function runDd(host: ShellHost, vfs: Vfs, opts: DdOpts, input: Value, ctx: CommandCtx): BytesBlob | undefined {
  const data = readSource(host, vfs, opts, input);
  const off = opts.seek * opts.bs;
  let result: BytesBlob | undefined;
  if (opts.of === undefined) {
    result = fromBytes(data);
  } else {
    const dst = vfs.resolve(opts.of);
    const display = vfs.display(dst);
    switch (dst.kind) {
      case "null":
        break;
      case "zero":
        throw new ShellError("/dev/zero: cannot write to /dev/zero");
      case "raw": {
        const disk = host.vol.sectorCount() * host.vol.sectorSize();
        // Checked here as well as in Rust so an offset above 2^32 never reaches the u32 binding.
        if (off + data.length > disk) throw new ShellError("/dev/hda: Range runs past the end of the disk", { code: "OutOfBounds" });
        if (data.length > 0) {
          try {
            host.run((v) => v.writeRaw(off, data));
          } catch (e) {
            throw wrapFs(display, e);
          }
        }
        break;
      }
      case "volume":
        writeVolumeFile(host, dst.path, display, off, data, ctx);
        break;
      default:
        throw new ShellError(`${display}: Is a directory`);
    }
  }
  ctx.log(`${formatRecords(data.length, opts.bs)} in`);
  ctx.log(`${formatRecords(data.length, opts.bs)} out`);
  ctx.log(`${data.length} bytes copied`);
  return result;
}
