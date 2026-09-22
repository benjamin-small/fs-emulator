import type { CommandArgs, CommandCtx, CommandSpec, FlagSpec, PosArg, Value } from "./types";
import type { DateTime, EntryInfo } from "../lib/wasm";
import { clusterByteRange } from "../core/attribution";
import { findEntrySlots } from "../core/direntry";
import { buildChain } from "../core/fatchain";
import { parseAddr } from "./addr";
import { decodeText, fromBytes } from "./bytes";
import { DD_MAX_BYTES } from "./dd";
import { ShellError, wrapFs } from "./errors";
import { atLatest, type ShellHost } from "./host";
import { canonicalize, Vfs, type Resolved } from "./vfs";

import type { CommandDef } from "./types";
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
    fn: (args) => {
      const target = posStr(args, 0);
      if (target === undefined) {
        host.select(null);
        return;
      }
      const r = vfs.resolve(target);
      const display = vfs.display(r);
      if (r.kind !== "volume") {
        throw new ShellError(`${display}: only volume paths can be selected`, { help: "select /mnt/<file>, or run select with no argument to clear" });
      }
      try {
        host.vol.stat(r.path);
      } catch (e) {
        throw wrapFs(display, e);
      }
      const canon = canonicalize(host.vol, r.path);
      host.select(canon === "/" ? null : canon);
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

/**
 * Every command the terminal registers. `echo` is a browser-terminal builtin
 * and is not registered here. `vfs` holds the one working directory per page.
 * Task 6 spreads its `writeCommands(host, vfs)` into this array.
 */
export function createCommands(host: ShellHost, vfs: Vfs = new Vfs()): CommandDef[] {
  return [...readCommands(host, vfs)];
}
