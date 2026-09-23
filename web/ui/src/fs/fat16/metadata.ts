import { BOOT_SECTOR_LEN, touchesBootSector } from "../../core/patch";
import type { FsAdapter } from "../adapter";

// Still defined in core/patch.ts until Task 3 moves the two here for real; this is
// already their FAT home for every importer.
export { BOOT_SECTOR_LEN, touchesBootSector };

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
