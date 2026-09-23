import type { FormatOptions } from "../../lib/wasm";
import type { FsAdapter } from "../adapter";
import { Fat16Adapter } from "./adapter";

export type Fat16FormatOptions = FormatOptions;

/** The FAT16 family: `fat16.format` is the one place `Volume.formatFat16` is called, and
 *  `fat16.bind` makes the adapter. It is defined beside the class (see adapter.ts). */
export { fat16, Fat16Adapter } from "./adapter";

/** The FAT adapter behind a generic one, for the few callers that need a FAT-only fact
 *  (FatMap's `fat`, the fundamentals scenario's `fatEntryOffset`). */
export function asFat16(fs: FsAdapter): Fat16Adapter {
  if (fs.id !== "fat16") throw new Error(`not a FAT16 adapter: ${fs.id}`);
  return fs as Fat16Adapter;
}

export { FAT_UNIT, clusterByteRange, clusterOfSector, fat16Space, fatColorForRegion } from "./geometry";
export { buildChain, clusterState, describeFatEntry, type ClusterState } from "./fatchain";
export { findEntrySlots, slotOffset, walkEntries } from "./direntry";
export { findRemnants } from "./remnants";
export { BOOT_SECTOR_LEN, CORRUPT_NOTE, NOTES, touchesBootSector } from "./metadata";
export { CLUSTER_SIZES, DEFAULTS, MKFS, SIZES, checkFormat, clusterCountFor } from "./format";
