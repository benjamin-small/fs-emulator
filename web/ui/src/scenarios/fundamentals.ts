import { asFat16 } from "../fs/fat16";
import type { Scenario } from "../state/scenarios.svelte";

/**
 * The first lesson: a tour of the on-disk structure of a FAT16 volume and how its pieces
 * refer to one another. Two files are created so there is something to point at, but the
 * creation itself is not the subject (that is "Add a small file"); every other step only
 * moves the dump. The numbers quoted in the copy are the default disk's, and
 * tests/fundamentals.test.ts checks them against a freshly formatted Volume.
 *
 * The FAT entry offsets are the one fact here that no generic adapter method answers, so
 * this file reaches past the seam with `asFat16(fs).fatEntryOffset(cluster, copy)`; the
 * boundary test allows `src/scenarios/` to import from `fs/fat16` for exactly this.
 */

export const HELLO = "/HELLO.TXT";
export const BIGGER = "/BIGGER.TXT";
export const HELLO_TEXT = "Hello, FAT16!";
/** More than two clusters, less than three, so the last cluster is partly slack. */
export const BIGGER_BYTES = 4796;

/** Numbered lines, so the dump's ASCII gutter shows readable text in every cluster. */
export function biggerText(bytes: number): Uint8Array {
  let s = "";
  for (let n = 1; s.length < bytes; n++) s += `line ${String(n).padStart(4, "0")}: the quick brown fox jumps over the lazy dog\n`;
  return new TextEncoder().encode(s.slice(0, bytes));
}

export const scenario: Scenario = {
  id: "fundamentals",
  title: "The fundamentals",
  summary: "Tour the regions of a FAT16 disk and follow how a file's slot, chain, and clusters link together.",
  family: "fat16",
  steps: [
    {
      title: "One disk, five regions",
      text: "This volume is 32,768 sectors of 512 bytes: 16 MB of numbered bytes and nothing else. FAT16 divides them into five regions in a fixed order: the boot sector, two copies of the file allocation table, the root directory, and the data area. The ribbon above the dump draws them in that order. HELLO.TXT is already on the disk so there is something to point at.",
      action: (v) => v.createFile(HELLO, new TextEncoder().encode(HELLO_TEXT)),
      focus: { sector: 0 },
    },
    {
      title: "Sector 0 describes the rest",
      text: "The BIOS parameter block at the start of sector 0 is a handful of small numbers: 512 bytes per sector (offset 11), 4 sectors per cluster (13), 1 reserved sector (14), 2 FATs (16), 512 root entries (17), 32,768 sectors in total (19), and 32 sectors per FAT (22). Every other address on the disk is arithmetic on these seven numbers. Point at any of them and the Inspector decodes it.",
      focus: { offset: 11 },
    },
    {
      title: "Where each region starts",
      text: "One reserved sector, so FAT 0 begins at sector 1. Each FAT is 32 sectors, so FAT 1 begins at sector 33 and the root directory at 65. 512 entries × 32 bytes is 16,384 bytes, 32 sectors, so the data area begins at sector 97. Nothing on the disk stores those four numbers; the driver computes them from sector 0, and so does the explorer.",
      focus: (fs) => ({ sector: fs.regionStart("directory") }),
    },
    {
      title: "The FAT: one entry per cluster",
      text: "The allocation table is an array of 16-bit entries, one per data cluster, indexed by cluster number. Entries 0 and 1 are reserved (F8 FF FF FF: the media descriptor and an end marker), so entry 2 is the first real one. A value of 0000 means the cluster is free, FFF8 through FFFF means end of chain, and anything else is the number of the next cluster in the same file. Cluster 2 is HELLO.TXT, and its entry reads end of chain: nothing follows.",
      focus: (fs) => ({ offset: asFat16(fs).fatEntryOffset(2, 0) }),
    },
    {
      title: "FAT 1 is a mirror",
      text: "The second copy starts at sector 33 and is byte for byte the same as the first. Every allocation is written to both, so a damaged first table can be recovered from the second. The entry for cluster 2 here matches the one you just saw.",
      focus: (fs) => ({ offset: asFat16(fs).fatEntryOffset(2, 1) }),
    },
    {
      title: "The root directory names the file",
      text: "The root directory is 512 fixed 32-byte slots starting at sector 65. HELLO.TXT's slot holds its 8.3 name padded with spaces, attribute flags, timestamps, its first cluster at byte 26 of the slot, and its size at byte 28. That first-cluster field is the link from the name to the data.",
      focus: (fs) => ({ path: HELLO, offset: fs.entrySlots(HELLO)?.start }),
    },
    {
      title: "Following the links",
      text: "Reading a file is three hops. The directory slot gives the first cluster: 2. The FAT entry for cluster 2 says whether more follow: it does not. And cluster 2's bytes live at sector 97 + (2 − 2) × 4 = sector 97, because cluster numbering starts at 2 and each cluster is 4 sectors. Only 13 of the cluster's 2,048 bytes are the file; the rest is untouched zeros.",
      focus: { path: HELLO, unit: 2 },
    },
    {
      title: "A larger file chains clusters",
      text: "BIGGER.TXT is 4,796 bytes, more than two clusters' worth, so it takes three: 3, 4, and 5. The FAT map on the left draws the chain as a line and the ribbon shows the three clusters side by side.",
      action: (v) => v.createFile(BIGGER, biggerText(BIGGER_BYTES)),
      focus: { path: BIGGER },
    },
    {
      title: "The chain in the table",
      text: "Now the FAT reads like a linked list: entry 3 says 4, entry 4 says 5, entry 5 says end of chain. The links are sequential here because the disk was empty; on a busy disk they can point anywhere, and a chain that jumps around is what fragmentation means.",
      focus: (fs) => ({ path: BIGGER, offset: asFat16(fs).fatEntryOffset(3, 0) }),
    },
    {
      title: "Size versus space",
      text: "The directory slot records 4,796 bytes; the chain reserves 3 × 2,048 = 6,144. The 1,348 bytes at the end of cluster 5 belong to the file's allocation but not to its contents: slack. The Files panel shows the size; the ribbon shows the space.",
      focus: { path: BIGGER, unit: 5 },
    },
    {
      title: "Free means zero in the table",
      text: "8,167 clusters exist and 4 are in use, so 8,163 entries read 0000. Nothing marks the data area itself as free: when a file is deleted its bytes stay until another file overwrites them, which the \"Delete and see what remains\" scenario shows.",
      focus: (fs) => ({ path: null, offset: asFat16(fs).fatEntryOffset(6, 0) }),
    },
    {
      title: "Putting it together",
      text: "Every path through this disk starts at sector 0: its numbers locate the tables and the root directory, a directory slot names a first cluster, the table chains the rest, and cluster numbers turn into sector addresses by arithmetic. The other scenarios change the disk one operation at a time; watch the ribbon and the What changed panel to see the same three places, slot, table, and data, move each time.",
      focus: { path: null, sector: 0 },
    },
  ],
};
