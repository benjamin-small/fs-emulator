import type { OpRecord } from "../lib/wasm";
import { asExt } from "../fs/ext";
import type { Scenario } from "../state/scenarios.svelte";
import { biggerText } from "./fundamentals";

/**
 * The ext tour: the on-disk structure of the default ext3 volume (16,384 blocks of 1 KiB, two
 * block groups) and how a name reaches its bytes through an inode. The first step creates the
 * two files the tour points at; every other step only moves the dump. The numbers quoted in the
 * copy are the default disk's, and tests/ext-fundamentals.test.ts checks every one of them
 * against a freshly formatted Volume.
 *
 * Inode slots have no generic adapter method, so this file reaches past the seam with
 * `asExt(fs).inodeOffset(ino)`; the boundary test allows `src/scenarios/` to import `fs/ext`.
 */

export const HELLO = "/hello.txt";
export const HELLO_TEXT = "Hello, ext3!";
export const BIGGER = "/bigger.txt";
/** Thirteen blocks: one more than an inode's twelve direct pointers, so the file needs a
 *  single-indirect block. */
export const BIGGER_BYTES = 13312;
export { biggerText };

/** Two creates as one timeline step: the changes and events of `a` then `b`, in the order the
 *  core made them, so rewinding the step undoes both. */
function both(a: OpRecord, b: OpRecord): OpRecord {
  return { op: `${a.op}; ${b.op}`, changes: [...a.changes, ...b.changes], events: [...a.events, ...b.events] };
}

export const scenario: Scenario = {
  id: "ext-fundamentals",
  title: "The fundamentals (ext)",
  summary: "Tour the block groups of an ext3 disk and follow a name through its inode to its blocks.",
  family: "ext",
  steps: [
    {
      title: "One disk, two block groups",
      text: "This volume is 16,384 blocks of 1,024 bytes: 16 MB. ext cuts the blocks into block groups of 8,192, each with its own bitmaps and inode table beside the blocks they describe; this disk has two, and the block-group map on the left draws them as two bands. Block 0, the boot block, belongs to neither: it is left all zero for a boot loader, which is why group 0 starts at block 1 and group 1 is one block short, 8,191. hello.txt and bigger.txt are already on the disk so there is something to point at.",
      action: (v) => both(v.createFile(HELLO, new TextEncoder().encode(HELLO_TEXT)), v.createFile(BIGGER, biggerText(BIGGER_BYTES))),
      focus: { sector: 0 },
    },
    {
      title: "Block 1 is the superblock",
      text: "The superblock counts the whole disk. At +0x38 is the magic number, 53 EF, which is 0xEF53 read little-endian: that is how a tool knows the disk is ext. The first fields are 1,024 inodes (+0x00) and 16,384 blocks (+0x04); +0x0C and +0x10 hold the free counts, 15,190 blocks and 1,011 inodes now that the two files exist. Point at any field and the Inspector decodes it.",
      focus: { offset: 1024 + 0x38 },
    },
    {
      title: "The group descriptors",
      text: "Block 2 holds one 32-byte descriptor per group. Group 0's, at +0x00, names its block bitmap (block 3), its inode bitmap (block 4), and the first block of its inode table (block 5), then its free counts: 7,067 blocks and 499 inodes. Group 1's, at +0x20, names blocks 8195, 8196, and 8197. A driver reads this block to find any group's tables without scanning the disk.",
      focus: { sector: 2 },
    },
    {
      title: "The block bitmap",
      text: "Block 3 is group 0's block bitmap: one bit per block, starting with block 1, set when the block is in use. Blocks 1 to 1110 are taken before any file exists: the group's metadata, the root directory and lost+found (69 to 81), and the journal (82 to 1110). The two files took the next 15, 1111 to 1125, so the bitmap reads 140 bytes of FF and then, at +0x8C, 1F: 1,125 bits set.",
      focus: { offset: 3 * 1024 + 0x8c },
    },
    {
      title: "The inode bitmap",
      text: "Block 4 does the same for group 0's 512 inodes, starting with inode 1. Inodes 1 to 10 are reserved (2 is the root directory, 8 the journal), 11 is lost+found, and the two files took 12 and 13, so the bitmap starts FF 1F: 13 bits set. 512 inodes need only 64 bytes; the rest of the block is padded with ones (+0x40 onward reads FF), so no inode past 512 can ever be handed out.",
      focus: { sector: 4 },
    },
    {
      title: "The inode table",
      text: "Blocks 5 to 68 are group 0's inode table: 512 inodes of 128 bytes, eight to a block. Inode N sits (N − 1) × 128 bytes into the table, so inode 2, the root directory, is at block 5 + 0x80, and inode 11, lost+found, at block 6 + 0x100. An inode holds a file's type and permissions, its size, its timestamps, and 15 block pointers. It does not hold the name.",
      focus: (fs) => ({ offset: asExt(fs).inodeOffset(2) }),
    },
    {
      title: "The root directory",
      text: "Inode 2's first block pointer reads 69, and block 69 is the root directory: a run of entries, each an inode number, a record length (rec_len), a name length, a type, and the name. . and .. (both inode 2) take 12 bytes each, lost+found 20, hello.txt 20, and bigger.txt the remaining 960: the last entry's rec_len runs to the end of the block, which is how a directory records its free space.",
      focus: (fs) => ({ path: "/", offset: fs.dataStart("/") ?? undefined }),
    },
    {
      title: "hello.txt's inode",
      text: "The entry for hello.txt names inode 12, at block 6 + 0x180. Its size field (+0x04) reads 12, and its first block pointer (+0x28) reads 1111, the first block the format left free; the other 14 pointers are zero.",
      focus: (fs) => ({ path: HELLO, offset: asExt(fs).inodeOffset(12) }),
    },
    {
      title: "Following the pointer",
      text: "Block 1111 holds the file's 12 bytes, Hello, ext3!, and 1,012 zeros. Reading a file is three hops: the directory entry gives the inode number, the inode gives the block numbers, and a block number times 1,024 is the byte offset on the disk.",
      focus: (fs) => ({ path: HELLO, sector: fs.chain(HELLO)[0] }),
    },
    {
      title: "A 13th block needs an indirect block",
      text: "bigger.txt is 13,312 bytes, 13 blocks: 1112 to 1124. An inode has only 12 direct pointers, so inode 13's 13th pointer (+0x58) names a single-indirect block, 1125, allocated after the data. That block is an array of 256 four-byte block numbers, and only the first is used: 1124, the file's 13th block. The block-group map marks the indirect block with a dot.",
      focus: (fs) => ({ path: BIGGER, sector: fs.owners.find((o) => o.path === BIGGER && o.role === "indirect")?.unit }),
    },
    {
      title: "The journal",
      text: "Blocks 82 to 1105 are the journal: 1,024 blocks of a hidden file, inode 8, whose own pointer blocks are 1106 to 1110. Block 82 is the journal's superblock (magic C0 3B 39 98); the rest is a ring the driver writes every metadata change to before it touches the change's home block. The Journal panel under the map draws the ring, and \"A journaled write\" follows one change through it.",
      focus: (fs) => ({ path: null, sector: asExt(fs).journal!.state().firstBlock }),
    },
    {
      title: "Group 1 keeps a backup",
      text: "Group 1 begins at block 8193 with a backup of the superblock (the same 53 EF at +0x38) and, in block 8194, of the descriptors, so a damaged block 1 or 2 can be rebuilt. Its own bitmaps are blocks 8195 and 8196 and its inode table 8197 to 8260. Its data blocks, 8261 onward, are all free: 8,123 of them.",
      focus: { sector: 8193 },
    },
    {
      title: "Putting it together",
      text: "Every path through this disk starts at block 1: the superblock sizes the groups, the descriptors in block 2 locate each group's bitmaps and inode table, inode 2 names the root directory's block, a directory entry maps a name to an inode, and the inode's pointers, direct or through an indirect block, name the data. The journal sits beside all of it, so a change to any of those blocks can be finished or thrown away after a crash.",
      focus: { path: null, sector: 1 },
    },
  ],
};
