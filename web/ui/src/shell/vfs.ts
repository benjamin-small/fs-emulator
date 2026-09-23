import type { FsAdapter } from "../fs/adapter";
import { ShellError } from "./errors";

/** Where a virtual path lands. `volume.path` is the path fs-core sees ("/" for /mnt itself). */
export type Resolved =
  | { kind: "root" }
  | { kind: "dev" }
  | { kind: "raw" }
  | { kind: "zero" }
  | { kind: "null" }
  | { kind: "volume"; path: string };

export const MOUNT = "/mnt";
const DEV = "/dev";
export const PATH_HELP = "the volume is mounted at /mnt; the raw disk is /dev/hda (with /dev/zero and /dev/null beside it)";
export const DEVICE_HELP = "devices: /dev/hda (the whole disk), /dev/zero, /dev/null";

/**
 * The virtual tree the shell shows: `/` holds `dev` and `mnt`. Normalization is client-side
 * because fs-core rejects `.` and `..`. One cwd per page: commands cannot learn their session
 * from browser-terminal's ctx, and there is one terminal instance anyway.
 */
export class Vfs {
  cwd = MOUNT;

  /** Absolute virtual path: `\` as `/`, separators collapsed, joined to cwd, `.` dropped, `..` popped (root stays root), no trailing `/`. */
  normalize(input: string): string {
    const slashed = input.replace(/\\/g, "/");
    const joined = slashed.startsWith("/") ? slashed : `${this.cwd}/${slashed}`;
    const parts: string[] = [];
    for (const part of joined.split("/")) {
      if (part === "" || part === ".") continue;
      if (part === "..") { parts.pop(); continue; }
      parts.push(part);
    }
    return "/" + parts.join("/");
  }

  resolve(input: string): Resolved {
    const p = this.normalize(input);
    if (p === "/") return { kind: "root" };
    if (p === DEV) return { kind: "dev" };
    if (p === `${DEV}/hda`) return { kind: "raw" };
    if (p === `${DEV}/zero`) return { kind: "zero" };
    if (p === `${DEV}/null`) return { kind: "null" };
    if (p === MOUNT) return { kind: "volume", path: "/" };
    if (p.startsWith(`${MOUNT}/`)) return { kind: "volume", path: p.slice(MOUNT.length) };
    if (p.startsWith(`${DEV}/`)) throw new ShellError(`no such device: ${p}`, { help: DEVICE_HELP });
    throw new ShellError(`no such file or directory: ${p}`, { help: PATH_HELP });
  }

  display(r: Resolved): string {
    switch (r.kind) {
      case "root": return "/";
      case "dev": return DEV;
      case "raw": return `${DEV}/hda`;
      case "zero": return `${DEV}/zero`;
      case "null": return `${DEV}/null`;
      case "volume": return this.toVirtual(r.path);
    }
  }

  toVirtual(volumePath: string): string {
    return volumePath === "/" ? MOUNT : `${MOUNT}${volumePath}`;
  }
}

/** Resolve `s` and its display string together — the pair almost every command needs first. */
export function resolved(vfs: Vfs, s: string): { r: Resolved; display: string } {
  const r = vfs.resolve(s);
  return { r, display: vfs.display(r) };
}

/**
 * Re-spell each component of a volume path the way the directory stores it, matching names
 * the family's way (`fs.namesMatch`: FAT lookups are case-insensitive, so `/docs/n.txt` is
 * `/DOCS/N.TXT` on disk). A component that is missing, or whose parent cannot be listed (a
 * file in the middle, a corrupt volume), keeps the typed spelling; callers validate
 * existence with `stat` separately.
 */
export function canonicalize(fs: Pick<FsAdapter, "vol" | "namesMatch">, volumePath: string): string {
  const out: string[] = [];
  let dir = "/";
  for (const part of volumePath.split("/")) {
    if (part === "") continue;
    let name = part;
    try {
      const hit = fs.vol.listDir(dir).find((e) => fs.namesMatch(e.name, part));
      if (hit) name = hit.name;
    } catch {
      // unreadable parent: keep the typed spelling for this and the remaining components
    }
    out.push(name);
    dir = joinVolume(dir, name);
  }
  return "/" + out.join("/");
}

/** The prompt prefix for a working directory: the path, then one space before the `❯`. */
export function promptFor(cwd: string): string {
  return `${cwd} `;
}

export function basename(p: string): string {
  const i = p.lastIndexOf("/");
  return i < 0 ? p : p.slice(i + 1);
}

export function joinVolume(dir: string, name: string): string {
  return dir === "/" ? `/${name}` : `${dir}/${name}`;
}
