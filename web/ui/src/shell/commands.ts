import type { CommandArgs, CommandCtx, CommandDef, CommandSpec, FlagSpec, PosArg } from "./types";
import type { DateTime, EntryInfo, Volume } from "../lib/wasm";
import { SIZE_HELP, addrHelp, parseAddr, parseSize } from "./addr";
import { decodeText, hasInput, toBytes } from "./bytes";
import { DD_MAX_BYTES, parseDd, runDd } from "./dd";
import { CORRUPT_HELP, ShellError, fsCall, wrapFs } from "./errors";
import { atLatest, corruptionOf, selectPath, type ShellHost } from "./host";
import { canonicalize, Vfs, promptFor, resolved, type Resolved } from "./vfs";
import { formatXxd } from "./xxd";

export type { CommandDef } from "./types";
export { fsCall } from "./errors";
export { resolved } from "./vfs";
export { selectPath } from "./host";

export const RAW_DEVICE_HELP = "read a range with: dd --if=/dev/hda --bs=512 --skip=0 --count=1 | xxd";

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

/** Path-based commands refuse to run while the on-disk metadata does not parse (the Rust gate says the same). */
export function assertMounted(host: ShellHost, display: string): void {
  const c = corruptionOf(host.vol);
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

/** "cluster" -> "Cluster": a summary that opens with the family's unit noun. */
const cap = (s: string): string => s.charAt(0).toUpperCase() + s.slice(1);

/** More than 10% control bytes (other than tab, LF, CR). Bytes >= 0x80 count as text so UTF-8 passes. */
export function looksBinary(bytes: Uint8Array): boolean {
  if (bytes.length === 0) return false;
  let odd = 0;
  for (const b of bytes) {
    if ((b < 0x20 && b !== 0x09 && b !== 0x0a && b !== 0x0d) || b === 0x7f) odd++;
  }
  return odd * 10 > bytes.length;
}

/**
 * The whole of one volume file, refused above `DD_MAX_BYTES` before a byte is read — the
 * size comes from `stat`, so the cap costs nothing when it trips. `cat` (both with and
 * without `--bytes`) and the `<` redirect hook share this, so they agree on the cap, on
 * what a directory says, and on how a wasm failure is phrased.
 */
export function readVolumeFile(host: ShellHost, vfs: Vfs, path: string): Uint8Array {
  const display = vfs.toVirtual(path);
  let info: EntryInfo;
  try {
    info = host.vol.stat(path);
  } catch (e) {
    throw wrapFs(display, e);
  }
  // Checked here rather than left to `readFile`, so the message names the tool to reach for.
  if (info.isDir) throw new ShellError(`${display}: Is a directory`, { code: "IsADirectory", help: `list it with: ls ${display}, then cat a file inside it` });
  if (info.size > DD_MAX_BYTES) {
    throw new ShellError(`${display}: file is ${info.size} bytes; cat prints at most ${DD_MAX_BYTES} bytes`, {
      help: `read part of it with: dd --if=${display} --bs=512 --count=1 | xxd`,
    });
  }
  try {
    return host.vol.readFile(path);
  } catch (e) {
    throw wrapFs(display, e);
  }
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
      // Every arm that moves `cwd` ends in `landed()`, so the prompt and the working
      // directory can never disagree; a `cd` that throws leaves both alone.
      const landed = () => host.setPrompt(promptFor(vfs.cwd));
      const target = posStr(args, 0);
      if (target === undefined) {
        vfs.cwd = "/mnt";
        landed();
        return;
      }
      const r = vfs.resolve(target);
      const display = vfs.display(r);
      switch (r.kind) {
        case "root":
          vfs.cwd = "/";
          landed();
          return;
        case "dev":
          vfs.cwd = "/dev";
          landed();
          return;
        case "volume": {
          let info: EntryInfo;
          try {
            info = host.vol.stat(r.path);
          } catch (e) {
            throw wrapFs(display, e);
          }
          if (!info.isDir) throw new ShellError(`${display}: Not a directory`, { code: "NotADirectory" });
          vfs.cwd = vfs.toVirtual(canonicalize(host.adapter, r.path));
          landed();
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
      summary: "Print a file (--bytes for raw bytes)",
      required: [P("path", "a file under /mnt, or /dev/null")],
      flags: [F("bytes", "return the raw bytes instead of decoding them as text")],
    },
    fn: (args, _input, ctx) => {
      warnIfRewound(host, ctx);
      const r = vfs.resolve(reqStr(args, 0, "path"));
      const display = vfs.display(r);
      const asBytes = flagOn(args, "bytes");
      switch (r.kind) {
        case "root":
        case "dev":
          throw new ShellError(`${display}: Is a directory`, { code: "IsADirectory" });
        case "raw":
        case "zero":
          throw new ShellError(`${display}: is a raw device`, { help: RAW_DEVICE_HELP });
        case "null":
          return asBytes ? new Uint8Array(0) : "";
        case "volume": {
          const bytes = readVolumeFile(host, vfs, r.path);
          if (asBytes) return bytes;
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
            state: corruptionOf(vol) ? "corrupt" : "ok",
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
          const canon = canonicalize(host.adapter, r.path);
          // The generic facts first, then the family's own (FAT: firstCluster, chain,
          // clusters, entryOffset, entrySlots, fatEntryOffset, dataOffset), already formatted.
          return {
            path: vfs.toVirtual(canon),
            name: info.name,
            type: info.isDir ? "dir" : "file",
            size: info.size,
            created: fmtDate(info.created),
            modified: fmtDate(info.modified),
            accessed: fmtDate(info.accessed),
            ...host.adapter.stat(canon),
          };
        }
      }
    },
  };

  const df: CommandDef = {
    spec: { name: "df", summary: `${cap(host.adapter.unit.singular)} usage of the mounted volume` },
    fn: (_args, _input, ctx) => {
      warnIfRewound(host, ctx);
      assertMounted(host, "/mnt");
      const { unitSize, units, used, free } = host.adapter.df();
      // The printed keys come from the noun, so FAT still prints `clusterSize` and `clusters`.
      const { singular, plural } = host.adapter.unit;
      const pct = units > 0 ? Math.round((used / units) * 100) : 0;
      return [
        {
          filesystem: "/dev/hda",
          mounted: "/mnt",
          type: host.vol.fsType(),
          [`${singular}Size`]: unitSize,
          [plural]: units,
          used,
          free,
          bytesUsed: used * unitSize,
          bytesFree: free * unitSize,
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
          state: corruptionOf(vol) ? "corrupt" : "ok",
        },
      ];
    },
  };

  const seek: CommandDef = {
    spec: {
      name: "seek",
      summary: "Move the hex dump to an address",
      required: [P("addr", `0x1f, 512, s:65 (sector), or ${host.adapter.unit.letter}:3 (${host.adapter.unit.singular})`)],
    },
    fn: (args) => {
      const vol = host.vol;
      const off = parseAddr(reqStr(args, 0, "addr"), host.adapter);
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

/** `vol.stat(path)`, or `null` when the path does not exist; other failures become ShellErrors. */
function statIfExists(vol: Volume, path: string, display: string): EntryInfo | null {
  try {
    return vol.stat(path);
  } catch (e) {
    if ((e as { code?: string }).code === "NotFound") return null;
    throw wrapFs(display, e);
  }
}

/** What `writeVolumeFile` did, so a caller with a `ctx` can log it. */
export interface VolumeWrite {
  /** Bytes the file holds afterwards — more than were handed in, on a real append. */
  written: number;
  /** True only when `append` was asked for *and* the file already existed. */
  appended: boolean;
}

/**
 * Put `bytes` in the volume file at `path`, creating it or overwriting it, and select it.
 * `append` reads the file and concatenates first — the core has no append, so the whole file
 * is rewritten either way (`adapter.notes.rewrite` says so in the family's words). The one
 * place a file's contents change: the `write` command, which adds the logging, and the
 * `>`/`>>` redirect hook, which has no `ctx` to log to.
 */
export function writeVolumeFile(host: ShellHost, vfs: Vfs, path: string, bytes: Uint8Array, opts: { append: boolean }): VolumeWrite {
  const display = vfs.toVirtual(path);
  const existing = statIfExists(host.vol, path, display);
  if (existing?.isDir) throw new ShellError(`${display}: Is a directory`, { code: "IsADirectory" });
  const appended = opts.append && existing !== null;
  let out = bytes;
  if (appended) {
    const old = fsCall(display, () => host.vol.readFile(path));
    out = new Uint8Array(old.length + bytes.length);
    out.set(old, 0);
    out.set(bytes, old.length);
  }
  const had = existing !== null;
  fsCall(display, () => host.run((v) => (had ? v.writeFile(path, out) : v.createFile(path, out))));
  selectPath(host, path);
  return { written: out.length, appended };
}

/** `write`, `mkdir`, `rmdir`, `rm`, `touch`, `cp`, `mkfs`: every disk change goes through `host.run`. */
function mutationCommands(host: ShellHost, vfs: Vfs): CommandDef[] {
  const joinPath = (dir: string, name: string): string => (dir === "/" ? `/${name}` : `${dir}/${name}`);
  const baseName = (p: string): string => p.slice(p.lastIndexOf("/") + 1);
  /** The volume path behind `target`, or the ShellError for `/`, `/dev` and the devices. */
  const volumePathOf = (target: Resolved, display: string, deviceMsg: string, dirMsg = "Is a directory"): string => {
    if (target.kind === "volume") return target.path;
    if (target.kind === "root" || target.kind === "dev") throw new ShellError(`${display}: ${dirMsg}`);
    throw new ShellError(`${display}: ${deviceMsg}`);
  };
  const flagGiven = (v: unknown): boolean => v !== undefined && v !== null;

  const write: CommandDef = {
    spec: {
      name: "write",
      summary: "Write piped text or bytes to a file, or to /dev/hda --at <addr>",
      required: [{ name: "path", shape: "str", desc: "/mnt/... or /dev/hda" }],
      flags: [
        { long: "append", desc: "read the file, append the input and rewrite the whole file" },
        { long: "at", shape: "str", desc: `disk address for /dev/hda (${addrHelp(host.adapter)})` },
      ],
    },
    fn: (args, input, ctx) => {
      const { r: target, display } = resolved(vfs, reqStr(args, 0, "path"));
      if (!hasInput(input)) throw new ShellError("nothing to write", { help: "pipe text or bytes in, e.g. echo hi | write /mnt/a.txt" });
      const data = toBytes(input);
      const append = args.flags.append === true;
      const at = args.flags.at;
      if (target.kind === "null") {
        ctx.log(`${data.length} bytes -> /dev/null`);
        return;
      }
      if (target.kind === "zero") throw new ShellError("/dev/zero: cannot write to /dev/zero");
      if (target.kind === "raw") {
        if (append) throw new ShellError("--append is not supported on /dev/hda", { help: "give --at <addr> to place the bytes" });
        if (!flagGiven(at)) throw new ShellError("/dev/hda: give --at <addr>", { help: addrHelp(host.adapter) });
        if (data.length === 0) throw new ShellError("nothing to write", { help: "a raw write needs at least one byte" });
        const off = parseAddr(String(at), host.adapter);
        const disk = host.vol.sectorCount() * host.vol.sectorSize();
        if (off + data.length > disk) throw new ShellError("/dev/hda: Range runs past the end of the disk", { code: "OutOfBounds" });
        fsCall(display, () => host.run((v) => v.writeRaw(off, data)));
        ctx.log(`${data.length} bytes -> /dev/hda at 0x${off.toString(16)}`);
        return;
      }
      if (target.kind !== "volume") throw new ShellError(`${display}: Is a directory`);
      if (flagGiven(at)) throw new ShellError("--at only applies to /dev/hda");
      const { written, appended } = writeVolumeFile(host, vfs, target.path, data, { append });
      if (appended) ctx.log(`appended ${data.length} bytes by rewriting the whole file (${host.adapter.notes.rewrite})`);
      ctx.log(`${written} bytes -> ${display}`);
    },
  };

  const mkdir: CommandDef = {
    spec: {
      name: "mkdir",
      summary: "Create a directory under /mnt",
      required: [{ name: "path", shape: "str", desc: "/mnt/..." }],
    },
    fn: (args) => {
      const { r: target, display } = resolved(vfs, reqStr(args, 0, "path"));
      const path = volumePathOf(target, display, "cannot create a directory on a device", "File exists");
      if (path === "/") throw new ShellError("/mnt: File exists");
      fsCall(display, () => host.run((v) => v.createDir(path)));
      selectPath(host, path);
    },
  };

  const rmdir: CommandDef = {
    spec: {
      name: "rmdir",
      summary: "Remove an empty directory",
      required: [{ name: "path", shape: "str", desc: "/mnt/..." }],
    },
    fn: (args) => {
      const { r: target, display } = resolved(vfs, reqStr(args, 0, "path"));
      const path = volumePathOf(target, display, "Not a directory", "cannot remove a virtual directory");
      if (path === "/") throw new ShellError("/mnt: cannot remove the mount point");
      const info = statIfExists(host.vol, path, display);
      if (info === null) throw new ShellError(`${display}: No such file or directory`, { code: "NotFound" });
      if (!info.isDir) throw new ShellError(`${display}: Not a directory`, { code: "NotADirectory" });
      fsCall(display, () => host.run((v) => v.removeDir(path)));
      host.select(null);
    },
  };

  const rm: CommandDef = {
    spec: {
      name: "rm",
      summary: "Delete a file",
      required: [{ name: "path", shape: "str", desc: "/mnt/..." }],
    },
    fn: (args) => {
      const { r: target, display } = resolved(vfs, reqStr(args, 0, "path"));
      const path = volumePathOf(target, display, "cannot remove a device");
      if (path === "/") throw new ShellError("/mnt: Is a directory", { help: "use rmdir" });
      const info = statIfExists(host.vol, path, display);
      if (info === null) throw new ShellError(`${display}: No such file or directory`, { code: "NotFound" });
      if (info.isDir) throw new ShellError(`${display}: Is a directory`, { help: "use rmdir", code: "IsADirectory" });
      fsCall(display, () => host.run((v) => v.deleteFile(path)));
      host.select(null);
    },
  };

  const touch: CommandDef = {
    spec: {
      name: "touch",
      summary: "Create an empty file (an existing file is left alone)",
      required: [{ name: "path", shape: "str", desc: "/mnt/..." }],
    },
    fn: (args, _input, ctx) => {
      const { r: target, display } = resolved(vfs, reqStr(args, 0, "path"));
      const path = volumePathOf(target, display, "cannot touch a device");
      if (path === "/") throw new ShellError("/mnt: Is a directory");
      const info = statIfExists(host.vol, path, display);
      if (info?.isDir) throw new ShellError(`${display}: Is a directory`);
      if (info) {
        ctx.log(`${display} exists; fs explorer has no timestamp-only update, nothing written`);
      } else {
        fsCall(display, () => host.run((v) => v.createFile(path, new Uint8Array(0))));
        ctx.log(`created empty ${display}`);
      }
      selectPath(host, path);
    },
  };

  const cp: CommandDef = {
    spec: {
      name: "cp",
      summary: "Copy a file (into a directory keeps its name)",
      required: [
        { name: "src", shape: "str", desc: "/mnt/file" },
        { name: "dst", shape: "str", desc: "/mnt/file or /mnt/dir" },
      ],
    },
    fn: (args, _input, ctx) => {
      const { r: src, display: srcDisplay } = resolved(vfs, reqStr(args, 0, "src"));
      const { r: dst, display: dstDisplay0 } = resolved(vfs, reqStr(args, 1, "dst"));
      const srcPath = volumePathOf(src, srcDisplay, "use dd for devices");
      const data = fsCall(srcDisplay, () => host.vol.readFile(srcPath));
      let dstPath = volumePathOf(dst, dstDisplay0, "use dd for devices");
      let dstDisplay = dstDisplay0;
      let info = statIfExists(host.vol, dstPath, dstDisplay);
      if (info?.isDir) {
        dstPath = joinPath(canonicalize(host.adapter, dstPath), baseName(canonicalize(host.adapter, srcPath)));
        dstDisplay = vfs.toVirtual(dstPath);
        info = statIfExists(host.vol, dstPath, dstDisplay);
        if (info?.isDir) throw new ShellError(`${dstDisplay}: Is a directory`);
      }
      const had = info !== null;
      const path = dstPath;
      fsCall(dstDisplay, () => host.run((v) => (had ? v.writeFile(path, data) : v.createFile(path, data))));
      ctx.log(`${data.length} bytes -> ${dstDisplay}`);
      selectPath(host, path);
    },
  };

  // The flags, their option keys, and both messages are the family's (`FsFamily.mkfs`); the
  // spec is built once at registration, for the family mounted then.
  const mkfs: CommandDef = {
    spec: {
      name: "mkfs",
      summary: host.adapter.family.mkfs.summary,
      flags: host.adapter.family.mkfs.flags.map((f) => F(f.long, f.desc, { shape: f.kind })),
    },
    fn: (args, _input, ctx) => {
      const { flags, done } = host.adapter.family.mkfs;
      const options: Record<string, number | string> = {};
      for (const f of flags) {
        const v = args.flags[f.long];
        if (!flagGiven(v)) continue;
        if (f.kind === "int") {
          if (typeof v !== "number" || !Number.isInteger(v) || v < 0) throw new ShellError(`--${f.long} must be a non-negative integer`);
          options[f.option] = v;
        } else {
          options[f.option] = String(v);
        }
      }
      fsCall("/dev/hda", () => host.format(host.adapter.id, options));
      vfs.cwd = "/mnt";
      host.setPrompt(promptFor(vfs.cwd));
      ctx.log(done);
    },
  };

  return [write, mkdir, rmdir, rm, touch, cp, mkfs];
}

/** `dd`, `xxd` and its alias `hexdump`. */
function ddXxdCommands(host: ShellHost, vfs: Vfs): CommandDef[] {
  const dd: CommandDef = {
    spec: {
      name: "dd",
      summary: "Copy blocks between files, /dev/hda and /dev/zero (at most 1 MiB per run)",
      rest: { name: "operand", shape: "str", desc: "classic operands: if=/dev/hda bs=512 count=1" },
      flags: [
        { long: "if", shape: "str", desc: "source path (default: the piped input)" },
        { long: "of", shape: "str", desc: "destination path (default: return the bytes down the pipe)" },
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
      { long: "offset", shape: "str", desc: `start address (${addrHelp(host.adapter)})` },
      { long: "len", shape: "str", desc: `bytes to show (${SIZE_HELP})` },
      { long: "cols", shape: "int", desc: "bytes per row, default 16" },
    ],
  });

  const xxd: CommandDef["fn"] = (args, input, ctx) => {
    const colsFlag = args.flags.cols;
    const cols = colsFlag === undefined || colsFlag === null ? 16 : Number(colsFlag);
    if (!Number.isInteger(cols) || cols < 1 || cols > 64) throw new ShellError("--cols must be 1..64");
    const offsetFlag = args.flags.offset;
    const offset = offsetFlag === undefined || offsetFlag === null ? undefined : parseAddr(String(offsetFlag), host.adapter);
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
      if (!hasInput(input)) throw new ShellError("nothing to dump", { help: "xxd <path>, or pipe bytes in: dd --if=/dev/hda --count=1 | xxd" });
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
  return [...readCommands(host, vfs), ...mutationCommands(host, vfs), ...ddXxdCommands(host, vfs)];
}
