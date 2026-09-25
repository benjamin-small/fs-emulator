import type { FsAdapter } from "../adapter";
import type { ExtAdapter } from "./adapter";

/** The ext family: `ext.format` is the one place `Volume.formatExt2`/`formatExt3` are called,
 *  and `ext.bind` makes the adapter. It is defined beside the class (see adapter.ts). */
export { ext, ExtAdapter, JOURNAL_OWNER } from "./adapter";

/** The ext adapter behind a generic one, for the few callers that need an ext-only fact (the
 *  block-group map's `geo` and `journalBlocks`, the journal panel, the lessons' `inodeOffset`). */
export function asExt(fs: FsAdapter): ExtAdapter {
  if (fs.id !== "ext") throw new Error(`not an ext adapter: ${fs.id}`);
  return fs as ExtAdapter;
}

export * from "./geometry";
export * from "./format";
export { ExtJournal } from "./journal";
export { CORRUPT_NOTE, NOTES, touchesMetadata } from "./metadata";
