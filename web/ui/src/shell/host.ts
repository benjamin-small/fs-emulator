import type { OpRecord, Volume } from "../lib/wasm";
import type { CrashPhase, FsAdapter, FsFamilyId } from "../fs/adapter";
import type { CommandCtx } from "./types";
import { canonicalize } from "./vfs";

/**
 * The seam between the commands and the explorer. The app implements it over the runes
 * stores (storeHost.svelte.ts); tests implement it over a plain Volume with an in-memory
 * history. Paths given to `select` are volume paths ("/A/B"), never "/mnt/A/B".
 */
export interface ShellHost {
  /** The latest volume; every read goes here even while the timeline is rewound. */
  readonly vol: Volume;
  /** The family adapter bound to `vol`: refreshed by the host after every `run`, re-bound by
   *  `format`. Commands take unit arithmetic, `stat`/`df` facts, address help, `mkfs` flags,
   *  and the rewrite notes from it, never from a family module directly. */
  readonly adapter: FsAdapter;
  /** Timeline position: -1 before any op, `historyLength - 1` at the latest step. */
  readonly cursor: number;
  readonly historyLength: number;
  /** Run one journaled mutation at the latest state. Throws `{ message, code? }` on failure. */
  run(fn: (v: Volume) => OpRecord): OpRecord;
  /** Replace the volume with a fresh one of `family`, formatted with that family's options;
   *  the store discards the history. Throws `{ message, code? }` on failure. */
  format(family: FsFamilyId, options?: unknown): void;
  /** Arm a crash in the mounted journal (`null` disarms). Not an op: no timeline step. The app
   *  arms through the store (`volume.setArmedPhase`), so the Journal panel shows the phase at
   *  once. Throws `{ message, code? }` on failure. */
  setArmedPhase(phase: CrashPhase | null): void;
  select(path: string | null): void;
  jumpTo(offset: number): void;
  /**
   * Set the prefix the terminal renders before its `❯`, so the prompt shows the working
   * directory. The app hands this straight to browser-terminal's `setPrompt` (0.3.0+),
   * which applies it to every pane. Commands call it whenever `vfs.cwd` changes.
   */
  setPrompt(prefix: string): void;
  closeTerminal(): void;
}

export const atLatest = (h: ShellHost): boolean => h.cursor === h.historyLength - 1;

/** The one warning every read command emits while the timeline is rewound. */
export function rewoundWarning(host: ShellHost): string {
  return `showing the latest state, not step ${host.cursor + 1} of ${host.historyLength}; click "Back to now" or run a write command`;
}

export function warnIfRewound(host: ShellHost, ctx: CommandCtx): void {
  if (!atLatest(host)) ctx.err(rewoundWarning(host));
}

/**
 * `vol.corruption()`, or `null` when the call itself throws. `corruption()` is on the
 * `FileSystem` trait, so no family makes it throw `NotFat`; the guard stays so a read
 * command never dies on a wasm boundary failure (errors.test.ts feeds it a throwing
 * double).
 */
export function corruptionOf(vol: Volume): string | null {
  try {
    return vol.corruption();
  } catch {
    return null;
  }
}

/** Select the canonical volume path (a root path, "/", becomes `null`), the rule `select` uses inline. */
export function selectPath(host: ShellHost, path: string): void {
  const canon = canonicalize(host.adapter, path);
  host.select(canon === "/" ? null : canon);
}

/**
 * What the store adapter throws when `VolumeStore.run`/`format` left a status instead of a
 * record. A plain `Error`, deliberately not a `ShellError`: the store's text is the raw wasm
 * message, and every `host.run`/`host.format` call sits inside an `fsCall`, which passes a
 * ShellError through untouched but wraps anything else as `${display}: ${phrase}`. So the
 * app prints "/mnt/A.TXT: File exists" like the tests do, instead of the bare "already exists".
 */
export function statusToError(s: { text: string; code?: string } | null): Error & { code?: string } {
  const e: Error & { code?: string } = new Error(s === null ? "the operation failed without a message" : s.text);
  if (s?.code !== undefined) e.code = s.code;
  return e;
}
