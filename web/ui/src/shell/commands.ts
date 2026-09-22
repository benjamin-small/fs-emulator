import type { CommandArgs, CommandCtx, CommandDef, CommandSpec, FlagSpec, PosArg } from "./types";
import type { DateTime, EntryInfo, FormatOptions, Volume } from "../lib/wasm";
import { clusterByteRange } from "../core/attribution";
import { findEntrySlots } from "../core/direntry";
import { buildChain } from "../core/fatchain";
import { ADDR_HELP, SIZE_HELP, parseAddr, parseSize } from "./addr";
import { decodeText, fromBytes, toBytes } from "./bytes";
import { DD_MAX_BYTES, parseDd, runDd } from "./dd";
import { ShellError, wrapFs } from "./errors";
import { atLatest, type ShellHost } from "./host";
import { canonicalize, Vfs, type Resolved } from "./vfs";
import { formatXxd } from "./xxd";

export type { CommandDef } from "./types";

export const RAW_DEVICE_HELP = "read a range with: dd --if=/dev/hda --bs=512 --skip=0 --count=1 | xxd";
export const CORRUPT_HELP =
  "the boot sector no longer parses; rewind on the timeline, or write the saved sector back with: <blob> | dd --of=/dev/hda";

// Spec builders. Every positional is "str" so a bareword like `true` or `512`
// still arrives as text; size and address flags parse their own strings.
export const P = (name: string, desc: string): PosArg => ({ name, shape: "str", desc });
export const F = (long: string, desc: string, extra: Partial<FlagSpec> = {}): FlagSpec => ({ long, desc, ...extra });

/** The one warning every read command emits while the timeline is rewound. */
export function rewoundWarning(host: ShellHost): string {
  return `showing the latest state, not step ${host.cursor + 1} of ${host.historyLength}; click "Back to now" or run a write command`;
}

export function warnIfRewound(host: ShellHost, ctx: CommandCtx): void {
  if (!atLatest(host)) ctx.err(rewoundWarning(host));
}

/** Path-based commands refuse to run while the boot sector does not parse (the Rust gate says the same). */
export function assertMounted(host: ShellHost, display: string): void {
  const c = host.vol.corruption();
  if (c) throw new ShellError(`${display}: ${c}`, { code: "CorruptImage", help: CORRUPT_HELP });
}

/** Resolve `s` and its display string together — the pair almost every command needs first. */
export function resolved(vfs: Vfs, s: string): { r: Resolved; display: string } {
  const r = vfs.resolve(s);
  return { r, display: vfs.display(r) };
}

/** Run `fn`; a thrown `ShellError` passes through unchanged, anything else becomes `wrapFs(display, e)`. */
export function fsCall<T>(display: string, fn: () => T): T {
  try {
    return fn();
  } catch (e) {
    throw wrapFs(display, e);
  }
}

/** Select the canonical volume path (a root path, "/", becomes `null`), the rule `select` uses inline. */
export function selectPath(host: ShellHost, path: string): void {
  const canon = canonicalize(host.vol, path);
  host.select(canon === "/" ? null : canon);
}

export function posStr(args: CommandArgs, i: number): string | undefined {
  const v = args.positionals[i];
  if (v === undefined || v === null) return undefined;
  return typeof v === "string" ? v : String(v);
}

export function reqStr(args: CommandArgs, i: number, name: string): string {
  const v = posStr(args, i);
  if (v === undefined) throw new ShellError(`missing required argument \`${name}\``);
  return v;
}

/** A switch flag is present as `true` or absent entirely; never `false`. */
export function flagOn(args: CommandArgs, name: string): boolean {
  return args.flags[name] === true;
}

export function fmtDate(d: DateTime | null): string {
  if (!d) return "";
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.year}-${p(d.month)}-${p(d.day)} ${p(d.hour)}:${p(d.minute)}:${p(d.second)}`;
}

export const hexAddr = (n: number): string => `0x${n.toString(16)}`;

/** More than 10% control bytes (other than tab, LF, CR). Bytes >= 0x80 count as text so UTF-8 passes. */
export function looksBinary(bytes: Uint8Array): boolean {
  if (bytes.length === 0) return false;
  let odd = 0;
  for (const b of bytes) {
    if ((b < 0x20 && b !== 0x09 && b !== 0x0a && b !== 0x0d) || b === 0x7f) odd++;
  }
  return odd * 10 > bytes.length;
}

type LsRow = { name: string; type: string; size: number; modified?: string };

function lsRows(host: ShellHost, vfs: Vfs, r: Resolved, long: boolean): LsRow[] {
  const vol = host.vol;
  const disk = vol.sectorCount() * vol.sectorSize();
  const row = (name: string, type: string, size: number, modified = ""): LsRow =>
    long ? { name, type, size, modified } : { name, type, size };
  switch (r.kind) {
    case "root":
      return [row("dev", "dir", 0), row("mnt", "dir", 0)];
    case "dev":
      return [row("hda", "block", disk), row("zero", "char", 0), row("null", "char", 0)];
    case "raw":
      return [row("hda", "block", disk)];
    case "zero":
      return [row("zero", "char", 0)];
    case "null":
      return [row("null", "char", 0)];
    case "volume": {
      const display = vfs.display(r);
      assertMounted(host, display);
      let entries: EntryInfo[];
      try {
        const info = vol.stat(r.path);
        entries = info.isDir ? vol.listDir(r.path) : [info];
      } catch (e) {
        throw wrapFs(display, e);
      }
      // On-disk slot order, like the Files pane; not sorted like coreutils.
      return entries.map((e) => row(e.name, e.isDir ? "dir" : "file", e.size, fmtDate(e.modified)));
    }
  }
}

export function readCommands(host: ShellHost, vfs: Vfs): CommandDef[] {
  const lsSpec: CommandSpec = {
    name: "ls",
    summary: "List a directory (/, /dev, /mnt/...)",
    optional: [P("path", "directory or file; defaults to the working directory")],
    flags: [F("long", "add the modified timestamp", { short: "l" })],
  };
  const ls: CommandDef["fn"] = (args, _input, ctx) => {
    warnIfRewound(host, ctx);
    const r = vfs.resolve(posStr(args, 0) ?? vfs.cwd);
    return lsRows(host, vfs, r, flagOn(args, "long"));
  };

  const cd: CommandDef = {
    spec: { name: "cd", summary: "Change the working directory", optional: [P("path", "directory; defaults to /mnt")] },
    fn: (args) => {
      const target = posStr(args, 0);
      if (target === undefined) {
        vfs.cwd = "/mnt";
        return;
      }
      const r = vfs.resolve(target);
      const display = vfs.display(r);
      switch (r.kind) {
        case "root":
          vfs.cwd = "/";
          return;
        case "dev":
          vfs.cwd = "/dev";
          return;
        case "volume": {
          let info: EntryInfo;
          try {
            info = host.vol.stat(r.path);
          } catch (e) {
            throw wrapFs(display, e);
          }
          if (!info.isDir) throw new ShellError(`${display}: Not a directory`, { code: "NotADirectory" });
          vfs.cwd = vfs.toVirtual(canonicalize(host.vol, r.path));
          return;
        }
        default:
          throw new ShellError(`${display}: Not a directory`, { code: "NotADirectory" });
      }
    },
  };

  const pwd: CommandDef = {
    spec: { name: "pwd", summary: "Print the working directory" },
    fn: () => vfs.cwd,
  };

  const cat: CommandDef = {
    spec: {
      name: "cat",
      summary: "Print a file (--bytes for a blob)",
      required: [P("path", "a file under /mnt, or /dev/null")],
      flags: [F("bytes", "return a { bytes, length } blob instead of text")],
    },
    fn: (args, _input, ctx) => {
      warnIfRewound(host, ctx);
      const r = vfs.resolve(reqStr(args, 0, "path"));
      const display = vfs.display(r);
      const asBlob = flagOn(args, "bytes");
      switch (r.kind) {
        case "root":
        case "dev":
          throw new ShellError(`${display}: Is a directory`, { code: "IsADirectory" });
        case "raw":
        case "zero":
          throw new ShellError(`${display}: is a raw device`, { help: RAW_DEVICE_HELP });
        case "null":
          return asBlob ? fromBytes(new Uint8Array(0)) : "";
        case "volume": {
          let bytes: Uint8Array;
          try {
            bytes = host.vol.readFile(r.path);
          } catch (e) {
            throw wrapFs(display, e);
          }
          if (asBlob) return fromBytes(bytes);
          if (bytes.length > DD_MAX_BYTES) {
            throw new ShellError(`${display}: file is ${bytes.length} bytes; cat prints at most ${DD_MAX_BYTES} bytes`, {
              help: `read part of it with: dd --if=${display} --bs=512 --count=1 | xxd`,
            });
          }
          if (looksBinary(bytes)) ctx.err(`binary file; try cat --bytes ${display} | xxd`);
          return decodeText(bytes);
        }
      }
    },
  };

  const stat: CommandDef = {
    spec: { name: "stat", summary: "Where a file lives on disk", required: [P("path", "a path under /mnt, or a device")] },
    fn: (args, _input, ctx) => {
      warnIfRewound(host, ctx);
      const r = vfs.resolve(reqStr(args, 0, "path"));
      const display = vfs.display(r);
      const vol = host.vol;
      switch (r.kind) {
        case "root":
        case "dev":
          return { path: display, type: "dir", size: 0 };
        case "zero":
        case "null":
          return { path: display, type: "char", size: 0 };
        case "raw": {
          const sectorSize = vol.sectorSize();
          const sectors = vol.sectorCount();
          return {
            path: display,
            type: "block",
            size: sectors * sectorSize,
            sectorSize,
            sectors,
            fsType: vol.fsType(),
            state: vol.corruption() ? "corrupt" : "ok",
          };
        }
        case "volume": {
          assertMounted(host, display);
          let info: EntryInfo;
          try {
            info = vol.stat(r.path);
          } catch (e) {
            throw wrapFs(display, e);
          }
          const canon = canonicalize(vol, r.path);
          const g = vol.geometry();
          const fat = vol.fatEntries(0);
          const owners = vol.clusterOwners();
          const first = owners.find((o) => o.path === canon)?.firstCluster ?? 0;
          const chain = buildChain(fat, first);
          const slots = findEntrySlots(vol, g, fat, owners, canon);
          const bps = g.bytesPerSector;
          return {
            path: vfs.toVirtual(canon),
            name: info.name,
            type: info.isDir ? "dir" : "file",
            size: info.size,
            created: fmtDate(info.created),
            modified: fmtDate(info.modified),
            accessed: fmtDate(info.accessed),
            firstCluster: first,
            chain,
            clusters: chain.length,
            entryOffset: slots ? hexAddr(slots.start) : "",
            entrySlots: slots ? `${hexAddr(slots.start)}-${hexAddr(slots.end)}` : "",
            fatEntryOffset: first >= 2 ? hexAddr(g.reservedSectors * bps + first * 2) : "",
            dataOffset: canon === "/" ? hexAddr(g.firstRootDirSector * bps) : first >= 2 ? hexAddr(clusterByteRange(g, first).start) : "",
          };
        }
      }
    },
  };

  const df: CommandDef = {
    spec: { name: "df", summary: "Cluster usage of the mounted volume" },
    fn: (_args, _input, ctx) => {
      warnIfRewound(host, ctx);
      assertMounted(host, "/mnt");
      const vol = host.vol;
      const g = vol.geometry();
      const fat = vol.fatEntries(0);
      let used = 0;
      for (let c = 2; c < g.clusterCount + 2 && c < fat.length; c++) if (fat[c].kind !== "free") used++;
      const free = g.clusterCount - used;
      const clusterSize = g.bytesPerSector * g.sectorsPerCluster;
      const pct = g.clusterCount > 0 ? Math.round((used / g.clusterCount) * 100) : 0;
      return [
        {
          filesystem: "/dev/hda",
          mounted: "/mnt",
          type: vol.fsType(),
          clusterSize,
          clusters: g.clusterCount,
          used,
          free,
          bytesUsed: used * clusterSize,
          bytesFree: free * clusterSize,
          use: `${pct}%`,
        },
      ];
    },
  };

  const mount: CommandDef = {
    spec: { name: "mount", summary: "Mounted volumes" },
    fn: (_args, _input, ctx) => {
      warnIfRewound(host, ctx);
      const vol = host.vol;
      const sectorSize = vol.sectorSize();
      const sectors = vol.sectorCount();
      return [
        {
          device: "/dev/hda",
          mount: "/mnt",
          type: vol.fsType(),
          sectorSize,
          sectors,
          bytes: sectors * sectorSize,
          state: vol.corruption() ? "corrupt" : "ok",
        },
      ];
    },
  };

  const seek: CommandDef = {
    spec: { name: "seek", summary: "Move the hex dump to an address", required: [P("addr", "0x1f, 512, s:65 (sector), or c:3 (cluster)")] },
    fn: (args) => {
      const vol = host.vol;
      const off = parseAddr(reqStr(args, 0, "addr"), vol.geometry());
      const limit = vol.sectorCount() * vol.sectorSize();
      if (off >= limit) throw new ShellError(`${hexAddr(off)} is past the end of the disk (${limit} bytes)`);
      host.jumpTo(off);
    },
  };

  const select: CommandDef = {
    spec: { name: "select", summary: "Highlight a file in the explorer", optional: [P("path", "a path under /mnt; omit to clear")] },
    fn: (args, _input, ctx) => {
      const target = posStr(args, 0);
      if (target === undefined) {
        host.select(null);
        return;
      }
      warnIfRewound(host, ctx);
      const { r, display } = resolved(vfs, target);
      if (r.kind !== "volume") {
        throw new ShellError(`${display}: only volume paths can be selected`, { help: "select /mnt/<file>, or run select with no argument to clear" });
      }
      fsCall(display, () => host.vol.stat(r.path));
      selectPath(host, r.path);
    },
  };

  const exit: CommandDef = {
    spec: { name: "exit", summary: "Close the terminal drawer" },
    fn: () => {
      host.closeTerminal();
    },
  };

  return [
    { spec: lsSpec, fn: ls },
    { spec: { ...lsSpec, name: "dir", summary: "alias of ls" }, fn: ls },
    cd,
    pwd,
    cat,
    stat,
    df,
    mount,
    seek,
    select,
    exit,
  ];
}

/** `dd`, `xxd` and its alias `hexdump`. */
function ddXxdCommands(host: ShellHost, vfs: Vfs): CommandDef[] {
  const dd: CommandDef = {
    spec: {
      name: "dd",
      summary: "Copy blocks between files, /dev/hda and /dev/zero (at most 1 MiB per run)",
      rest: { name: "operand", shape: "str", desc: "quoted classic operands: 'if=/dev/hda' 'bs=512' 'count=1'" },
      flags: [
        { long: "if", shape: "str", desc: "source path (default: the piped input)" },
        { long: "of", shape: "str", desc: "destination path (default: return the bytes as a blob)" },
        { long: "bs", shape: "str", desc: "block size, default 512 (k and M suffixes)" },
        { long: "count", shape: "str", desc: "blocks to copy (default: to the end of the source)" },
        { long: "skip", shape: "str", desc: "blocks to skip at the start of the source" },
        { long: "seek", shape: "str", desc: "blocks to skip at the start of the destination" },
      ],
    },
    fn: (args, input, ctx) => {
      const opts = parseDd(args.flags, args.positionals);
      if (opts.if !== undefined) {
        const { r } = resolved(vfs, opts.if);
        if (r.kind === "raw" || r.kind === "volume") warnIfRewound(host, ctx);
      }
      return runDd(host, vfs, opts, input, ctx);
    },
  };

  const xxdSpec = (name: string, summary: string): CommandSpec => ({
    name,
    summary,
    optional: [{ name: "path", shape: "str", desc: "/mnt/file, /dev/hda (one sector by default) or /dev/zero --len; omit to dump piped bytes" }],
    flags: [
      { long: "offset", shape: "str", desc: `start address (${ADDR_HELP})` },
      { long: "len", shape: "str", desc: `bytes to show (${SIZE_HELP})` },
      { long: "cols", shape: "int", desc: "bytes per row, default 16" },
    ],
  });

  const xxd: CommandDef["fn"] = (args, input, ctx) => {
    const colsFlag = args.flags.cols;
    const cols = colsFlag === undefined || colsFlag === null ? 16 : Number(colsFlag);
    if (!Number.isInteger(cols) || cols < 1 || cols > 64) throw new ShellError("--cols must be 1..64");
    const offsetFlag = args.flags.offset;
    const offset = offsetFlag === undefined || offsetFlag === null ? undefined : parseAddr(String(offsetFlag), host.vol.geometry());
    const lenFlag = args.flags.len;
    const len = lenFlag === undefined || lenFlag === null ? undefined : parseSize(String(lenFlag));
    const base = offset ?? 0;
    const slice = (all: Uint8Array): Uint8Array => {
      const from = Math.min(base, all.length);
      const to = len === undefined ? all.length : Math.min(all.length, from + len);
      return all.subarray(from, to);
    };
    const tooBig = (n: number) => new ShellError(`refusing to dump ${n} bytes; the limit is ${DD_MAX_BYTES}`, { help: "use a smaller --len" });

    let bytes: Uint8Array;
    if (args.positionals.length === 0) {
      if (input === null) throw new ShellError("nothing to dump", { help: "xxd <path>, or pipe bytes in: dd --if=/dev/hda --count=1 | xxd" });
      bytes = slice(toBytes(input));
    } else {
      const { r: target, display } = resolved(vfs, String(args.positionals[0]));
      switch (target.kind) {
        case "raw": {
          warnIfRewound(host, ctx);
          const disk = host.vol.sectorCount() * host.vol.sectorSize();
          if (base >= disk) throw new ShellError(`0x${base.toString(16)} is past the end of the disk (${disk} bytes)`);
          const n = Math.min(len ?? host.vol.sectorSize(), disk - base);
          if (n > DD_MAX_BYTES) throw tooBig(n);
          bytes = host.vol.readRaw(base, n);
          break;
        }
        case "zero": {
          if (len === undefined) throw new ShellError("/dev/zero: give --len", { help: SIZE_HELP });
          if (len > DD_MAX_BYTES) throw tooBig(len);
          bytes = new Uint8Array(len);
          break;
        }
        case "null":
          bytes = new Uint8Array(0);
          break;
        case "volume": {
          warnIfRewound(host, ctx);
          const file = fsCall(display, () => host.vol.readFile(target.path));
          bytes = slice(file);
          break;
        }
        default:
          throw new ShellError(`${display}: Is a directory`);
      }
    }
    if (bytes.length > DD_MAX_BYTES) throw tooBig(bytes.length);
    return formatXxd(bytes, base, cols);
  };

  return [
    dd,
    { spec: xxdSpec("xxd", "Hex dump a file, /dev/hda, or piped bytes"), fn: xxd },
    { spec: xxdSpec("hexdump", "alias of xxd"), fn: xxd },
  ];
}

/**
 * Every command the terminal registers. `echo` is a browser-terminal builtin
 * and is not registered here. `vfs` holds the one working directory per page.
 */
export function createCommands(host: ShellHost, vfs: Vfs = new Vfs()): CommandDef[] {
  return [...readCommands(host, vfs), ...ddXxdCommands(host, vfs)];
}
