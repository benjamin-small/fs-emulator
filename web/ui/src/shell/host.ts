import type { FormatOptions, OpRecord, Volume } from "../lib/wasm";
import { canonicalize } from "./vfs";

/**
 * The seam between the commands and the explorer. The app implements it over the runes
 * stores (storeHost.svelte.ts); tests implement it over a plain Volume with an in-memory
 * history. Paths given to `select` are volume paths ("/A/B"), never "/mnt/A/B".
 */
export interface ShellHost {
  /** The latest volume; every read goes here even while the timeline is rewound. */
  readonly vol: Volume;
  /** Timeline position: -1 before any op, `historyLength - 1` at the latest step. */
  readonly cursor: number;
  readonly historyLength: number;
  /** Run one journaled mutation at the latest state. Throws `{ message, code? }` on failure. */
  run(fn: (v: Volume) => OpRecord): OpRecord;
  /** Replace the volume; the store discards the history. Throws `{ message, code? }` on failure. */
  format(options: FormatOptions): void;
  select(path: string | null): void;
  jumpTo(offset: number): void;
  closeTerminal(): void;
}

export const atLatest = (h: ShellHost): boolean => h.cursor === h.historyLength - 1;

/**
 * `vol.corruption()`, or `null` when the call itself throws. `corruption()` will throw
 * `NotFat` on a non-FAT volume in the future, and a read command must not die on that;
 * VolumeStore.refreshMeta guards it the same way.
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
  const canon = canonicalize(host.vol, path);
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
