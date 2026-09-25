import type { ExtGeometry, Region } from "../../src/lib/wasm";
import { extSpace } from "../../src/fs/ext/geometry";

// The default `Volume.formatExt3(undefined)` disk: 16,384 blocks of 1 KiB in two groups of
// 512 inodes; group 0's metadata is blocks 1..68, the root directory is block 69, lost+found
// 70..81, the journal's data 82..1105 (its five pointer blocks 1106..1110 lie in the data
// region after it); group 1 starts with its backup superblock at 8193. The free counts are
// the freshly formatted ones. tests/fs/ext.test.ts checks both against a live volume.
export const geo: ExtGeometry = {
  blockSize: 1024, totalBlocks: 16384, firstDataBlock: 1, blocksPerGroup: 8192, inodesPerGroup: 512,
  inodesCount: 1024, inodeSize: 128, inodeTableBlocks: 64, descriptorBlocks: 1,
  groups: [
    { index: 0, firstBlock: 1, blockCount: 8192, superblockBlock: 1, descriptorsBlock: 2, blockBitmap: 3, inodeBitmap: 4, inodeTable: 5, firstData: 69, freeBlocks: 7082, freeInodes: 501, usedDirs: 2 },
    { index: 1, firstBlock: 8193, blockCount: 8191, superblockBlock: 8193, descriptorsBlock: 8194, blockBitmap: 8195, inodeBitmap: 8196, inodeTable: 8197, firstData: 8261, freeBlocks: 8123, freeInodes: 512, usedDirs: 0 },
  ],
};
export const layout: Region[] = [
  { name: "boot block", sectors: { start: 0, end: 1 }, kind: "boot" },
  { name: "superblock", sectors: { start: 1, end: 2 }, kind: "boot" },
  { name: "group descriptors (group 0)", sectors: { start: 2, end: 3 }, kind: "metadata" },
  { name: "block bitmap (group 0)", sectors: { start: 3, end: 4 }, kind: "allocationTable" },
  { name: "inode bitmap (group 0)", sectors: { start: 4, end: 5 }, kind: "allocationTable" },
  { name: "inode table (group 0)", sectors: { start: 5, end: 69 }, kind: "metadata" },
  { name: "data (group 0)", sectors: { start: 69, end: 82 }, kind: "data" },
  { name: "journal", sectors: { start: 82, end: 1106 }, kind: "journal" },
  { name: "data (group 0)", sectors: { start: 1106, end: 8193 }, kind: "data" },
  { name: "backup superblock (group 1)", sectors: { start: 8193, end: 8194 }, kind: "boot" },
  { name: "group descriptors (group 1)", sectors: { start: 8194, end: 8195 }, kind: "metadata" },
  { name: "block bitmap (group 1)", sectors: { start: 8195, end: 8196 }, kind: "allocationTable" },
  { name: "inode bitmap (group 1)", sectors: { start: 8196, end: 8197 }, kind: "allocationTable" },
  { name: "inode table (group 1)", sectors: { start: 8197, end: 8261 }, kind: "metadata" },
  { name: "data (group 1)", sectors: { start: 8261, end: 16384 }, kind: "data" },
];

/** The same disk as a `UnitSpace`, for tests of the generic core that need no Volume. */
export const space = extSpace(geo);
