import type { FormatOptions, OpRecord, Volume } from "../lib/wasm";
import { ShellError } from "./errors";
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

/** Select the canonical volume path (a root path, "/", becomes `null`), the rule `select` uses inline. */
export function selectPath(host: ShellHost, path: string): void {
  const canon = canonicalize(host.vol, path);
  host.select(canon === "/" ? null : canon);
}

/** What the store adapter throws when `VolumeStore.run`/`format` left a status instead of a record. */
export function statusToError(s: { text: string; code?: string } | null): Error & { code?: string } {
  if (s === null) return new ShellError("the operation failed without a message");
  return new ShellError(s.text, { code: s.code });
}
