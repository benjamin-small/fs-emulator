import type { ShellHost } from "./host";
import { corruptionOf } from "./host";
import { MOUNT, type Vfs } from "./vfs";

/**
 * Tab completion for paths. browser-terminal completes command names and flags itself, and
 * asks a host provider for anything else; with its own filesystem turned off
 * (`filesystem: false`) nothing completes `/mnt` or `/dev` unless the explorer does.
 */

/** The part of browser-terminal's `ArgumentCompletionContext` this reads. */
export interface PathCompletionContext {
  command: string;
  prefix: string;
  argumentIndex: number;
  flag?: string;
  kind: "argument" | "redirect";
}

/** browser-terminal's `CompletionItem`: the whole replacement for the typed word. */
export interface PathCompletion {
  value: string;
  directory: boolean;
}

/**
 * The commands whose positionals are all paths, and how many there are; `dirs` offers only
 * directories (`cd`, `mkdir`, `rmdir`). complete.test.ts checks this against every family's
 * command specs, so a new path command fails the test until it is listed here.
 */
export const PATH_COMMANDS: Readonly<Record<string, { count: number; dirs?: boolean }>> = {
  ls: { count: 1 },
  dir: { count: 1 },
  cd: { count: 1, dirs: true },
  cat: { count: 1 },
  stat: { count: 1 },
  select: { count: 1 },
  write: { count: 1 },
  mkdir: { count: 1, dirs: true },
  rmdir: { count: 1, dirs: true },
  rm: { count: 1 },
  touch: { count: 1 },
  cp: { count: 2 },
  xxd: { count: 1 },
  hexdump: { count: 1 },
};

/** `dd`'s operands that name a file: `if=/dev/hda`, `of=/mnt/A.TXT`. */
const DD_PATH_OPERAND = /^(if|of)=/;

/**
 * The completions for the word being typed, or `[]` when it is not a path. Each value is the
 * typed directory part plus an entry name (directories end in `/`), so a relative prefix stays
 * relative. FAT names match case-insensitively (`adapter.namesMatch`), and a match continues
 * in lower case when the user typed in lower case, which FAT resolves the same.
 */
export function completePath(host: ShellHost, vfs: Vfs, ctx: PathCompletionContext): PathCompletion[] {
  let word = ctx.prefix;
  let key = "";
  let dirs = false;
  if (ctx.kind !== "redirect") {
    if (ctx.flag) return [];
    if (ctx.command === "dd") {
      const m = DD_PATH_OPERAND.exec(word);
      if (!m) return [];
      key = m[0];
      word = word.slice(key.length);
    } else {
      const spec = PATH_COMMANDS[ctx.command];
      if (!spec || ctx.argumentIndex >= spec.count) return [];
      dirs = !!spec.dirs;
    }
  }

  const slash = word.lastIndexOf("/");
  const parent = word.slice(0, slash + 1);
  const leaf = word.slice(slash + 1);
  const entries = listVirtual(host, vfs.normalize(parent === "" ? "." : parent));
  const folds = host.adapter.namesMatch("a", "A");
  const lowerTyped = folds && leaf !== "" && leaf === leaf.toLowerCase();

  const out: PathCompletion[] = [];
  for (const e of entries) {
    if (dirs && !e.directory) continue;
    const starts = folds ? e.name.toLowerCase().startsWith(leaf.toLowerCase()) : e.name.startsWith(leaf);
    if (!starts) continue;
    const name = folds ? leaf + (lowerTyped ? e.name.slice(leaf.length).toLowerCase() : e.name.slice(leaf.length)) : e.name;
    out.push({ value: key + parent + name + (e.directory ? "/" : ""), directory: e.directory });
  }
  return out;
}

/** The entries of virtual directory `dir` (absolute, normalized), or `[]` when it is not one. */
function listVirtual(host: ShellHost, dir: string): { name: string; directory: boolean }[] {
  if (dir === "/") return [{ name: "dev", directory: true }, { name: "mnt", directory: true }];
  if (dir === "/dev") return ["hda", "null", "zero"].map((name) => ({ name, directory: false }));
  if (dir !== MOUNT && !dir.startsWith(`${MOUNT}/`)) return [];
  if (corruptionOf(host.vol)) return [];
  try {
    return host.vol
      .listDir(dir === MOUNT ? "/" : dir.slice(MOUNT.length))
      .filter((e) => e.name !== "." && e.name !== "..")
      .map((e) => ({ name: e.name, directory: e.isDir }));
  } catch {
    return []; // a missing directory, or a file where a directory was typed
  }
}
