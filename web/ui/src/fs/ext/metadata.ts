import type { ExtGeometry } from "../../lib/wasm";
import type { ByteChangeLike } from "../../core/patch";
import type { FsAdapter } from "../adapter";

/**
 * True when any change starts in a block the core may re-parse or that moves what the layout
 * shows: a block below its group's first data block (the boot block, a superblock, the
 * descriptors, the bitmaps, an inode table), one of the journal's blocks (its data and its
 * pointer blocks), or an indirect block of any path. The store then re-reads the layout
 * (`ExtAdapter.touchesMetadata`). A change inside a file's data blocks is not metadata.
 */
export function touchesMetadata(
  geo: ExtGeometry,
  journalBlocks: readonly number[],
  indirectBlocks: readonly number[],
  changes: ByteChangeLike[],
): boolean {
  const journal = new Set(journalBlocks), indirect = new Set(indirectBlocks);
  return changes.some((c) => {
    const block = Math.floor(c.offset / geo.blockSize);
    const group = geo.groups.find((g) => block >= g.firstBlock && block < g.firstBlock + g.blockCount);
    const belowData = group ? block < group.firstData : block < geo.firstDataBlock;
    return belowData || journal.has(block) || indirect.has(block);
  });
}

/** DirTree's note while a raw write has left the superblock or descriptors unparsable. */
export const CORRUPT_NOTE = "Superblock or group descriptors do not parse; the tree is unavailable until a raw write repairs them.";

/**
 * The ext clauses the shell logs when a write had to rewrite a whole file: `write --append`
 * logs `appended ${n} bytes by rewriting the whole file (${NOTES.rewrite})`, and dd's
 * `--seek` overlay logs `NOTES.partialWrite` as a line of its own. The ext core overwrites a
 * file in place: it keeps the blocks the file has and maps new ones after them.
 */
export const NOTES: FsAdapter["notes"] = {
  rewrite: "ext keeps the file's blocks and maps new ones after them",
  partialWrite: "the ext volume has no partial writes; the whole file was rewritten over its blocks",
};
