import type { FsAdapter } from "../fs/adapter";

/** The ribbon's free-space label, from the family's free count (`freeUnits`): on FAT the units
 *  no owner row claims, the count the ribbon always made, so FAT's label is the one it always
 *  was; on ext the superblock's count, because counting the blocks without an owner row would
 *  call the inode tables, the bitmaps, and the journal free (`16.0 MB free` on a fresh ext3 disk
 *  where 15,205 blocks are free). */
export function freeSpaceLabel(fs: Pick<FsAdapter, "freeUnits" | "unitSize">): string {
  return `${((fs.freeUnits() * fs.unitSize) / (1024 * 1024)).toFixed(1)} MB free`;
}
