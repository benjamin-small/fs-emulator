import type { ByteChangeLike } from "../../core/patch";
import type { FsAdapter } from "../adapter";

/** Bytes the FAT core re-parses as the boot sector after a raw write. */
export const BOOT_SECTOR_LEN = 512;

/** True when any change starts inside the boot sector. The core may then have adopted
 *  a new geometry, so the store must re-read the layout (`Fat16Adapter.touchesMetadata`). */
export function touchesBootSector(changes: ByteChangeLike[]): boolean {
  return changes.some((c) => c.offset < BOOT_SECTOR_LEN);
}

/** DirTree's note while a raw write has left the boot sector unparsable. */
export const CORRUPT_NOTE = "Boot sector does not parse; the tree is unavailable until a raw write repairs it.";

/**
 * The FAT clauses the shell logs when a write had to rewrite a whole file, strings unchanged:
 * `write --append` logs `appended ${n} bytes by rewriting the whole file (${NOTES.rewrite})`,
 * and dd's `--seek` overlay logs `NOTES.partialWrite` as a line of its own.
 */
export const NOTES: FsAdapter["notes"] = {
  rewrite: "FAT has no append; the old chain is freed and reallocated",
  partialWrite: "FAT has no partial writes; the whole file was rewritten",
};
