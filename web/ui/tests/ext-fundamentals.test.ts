import { describe, expect, it } from "vitest";
import { applyChanges } from "../src/core/patch";
import { FAMILIES } from "../src/fs";
import { BIGGER, BIGGER_BYTES, biggerText, HELLO, HELLO_TEXT, scenario } from "../src/scenarios/extFundamentals";
import { lessonHelpers } from "./fixtures/lesson";

// The ext tab's "The fundamentals" quotes the default ext3 disk's numbers in its copy (block 69,
// 15,190 free blocks, inode 12 at block 6 + 0x180, ...). These tests pin every quoted number
// to the volume the lesson runs on, and every step's resolved focus, so a change to the default
// format cannot leave the lesson teaching stale arithmetic.

const n = (v: number) => v.toLocaleString("en-US");
const u32 = (b: Uint8Array, at: number) => new DataView(b.buffer, b.byteOffset, b.byteLength).getUint32(at, true);
const { stepText, runThrough, focusOf } = lessonHelpers(scenario);
/** The first offset where two images differ, or -1: a 16 MB `toEqual` is too slow to diff. */
const firstDiff = (a: Uint8Array, b: Uint8Array) => {
  if (a.length !== b.length) return Math.min(a.length, b.length);
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return i;
  return -1;
};
const setBits = (bytes: Uint8Array) => bytes.reduce((sum, b) => sum + b.toString(2).split("").filter((c) => c === "1").length, 0);

describe("the ext fundamentals lesson", () => {
  it("runs on the default ext3 disk", () => {
    expect(scenario.id).toBe("ext-fundamentals");
    expect(scenario.title).toBe("The fundamentals");
    expect(scenario.family).toBe("ext");
    expect(FAMILIES[scenario.family].format().fsType()).toBe("ext3");
  });

  it("creates both files in one timeline step that rewinds cleanly", () => {
    const fresh = FAMILIES.ext.format().image();
    const { vol, records } = runThrough("One disk, two block groups");
    expect(records).toHaveLength(1);
    const [rec] = records;
    expect(rec.op).toBe(`create_file ${HELLO}; create_file ${BIGGER}`);
    // The runner patches its cached image with the record's changes: forward must reach the
    // volume's bytes, and reverse must get back to the freshly formatted disk.
    const image = fresh.slice();
    applyChanges(image, rec.changes, "forward");
    expect(firstDiff(image, vol.image())).toBe(-1);
    applyChanges(image, rec.changes, "reverse");
    expect(firstDiff(image, fresh)).toBe(-1);
    expect(new TextDecoder().decode(vol.readFile(HELLO))).toBe(HELLO_TEXT);
    expect(vol.readFile(BIGGER)).toEqual(biggerText(BIGGER_BYTES));
  });

  it("quotes the disk's size, its two groups, and the zero boot block", () => {
    const { vol } = runThrough("One disk, two block groups");
    const g = vol.extGeometry();
    expect(g).toMatchObject({ blockSize: 1024, totalBlocks: 16384, blocksPerGroup: 8192, firstDataBlock: 1 });
    expect(g.groups.map((x) => [x.firstBlock, x.blockCount])).toEqual([[1, 8192], [8193, 8191]]);
    expect(vol.sector(0).every((b) => b === 0)).toBe(true);
    const text = stepText("One disk, two block groups");
    for (const s of [`${n(g.totalBlocks)} blocks of ${n(g.blockSize)} bytes: 16 MB`, `block groups of ${n(g.blocksPerGroup)}`, "this disk has two", `group 0 starts at block ${g.groups[0].firstBlock}`, `one block short, ${n(g.groups[1].blockCount)}`]) {
      expect(text).toContain(s);
    }
    expect(g.totalBlocks * g.blockSize).toBe(16 * 1024 * 1024);
  });

  it("quotes the superblock's magic, sizes, and free counts after the two creates", () => {
    const { vol } = runThrough("One disk, two block groups");
    const sb = vol.extSuperblock();
    expect(sb).toMatchObject({ magic: 0xef53, inodesCount: 1024, blocksCount: 16384, freeBlocks: 15190, freeInodes: 1011 });
    const block1 = vol.sector(1);
    expect([block1[0x38], block1[0x39]]).toEqual([0x53, 0xef]);
    const at = (offset: number) => vol.annotateSector(1).find((a) => a.range.start === offset)?.label;
    expect([at(0x00), at(0x04), at(0x0c), at(0x10), at(0x38)]).toEqual(["inodes count", "blocks count", "free blocks count", "free inodes count", "magic"]);
    // 15 blocks (1 for hello.txt, 13 data and 1 indirect for bigger.txt) and 2 inodes left the fresh counts.
    expect(FAMILIES.ext.format().extSuperblock()).toMatchObject({ freeBlocks: sb.freeBlocks + 15, freeInodes: sb.freeInodes + 2 });
    const text = stepText("Block 1 is the superblock");
    for (const s of ["At +0x38 is the magic number, 53 EF, which is 0xEF53", `${n(sb.inodesCount)} inodes (+0x00)`, `${n(sb.blocksCount)} blocks (+0x04)`, "+0x0C and +0x10 hold the free counts", `${n(sb.freeBlocks)} blocks and ${n(sb.freeInodes)} inodes`]) {
      expect(text).toContain(s);
    }
  });

  it("quotes each group's descriptor", () => {
    const { vol } = runThrough("One disk, two block groups");
    const [g0, g1] = vol.extGeometry().groups;
    expect(g0).toMatchObject({ descriptorsBlock: 2, blockBitmap: 3, inodeBitmap: 4, inodeTable: 5, freeBlocks: 7067, freeInodes: 499 });
    expect(g1).toMatchObject({ blockBitmap: 8195, inodeBitmap: 8196, inodeTable: 8197 });
    const block2 = vol.sector(2);
    expect([u32(block2, 0x00), u32(block2, 0x04), u32(block2, 0x08)]).toEqual([3, 4, 5]);
    expect([u32(block2, 0x20), u32(block2, 0x24), u32(block2, 0x28)]).toEqual([8195, 8196, 8197]);
    const text = stepText("The group descriptors");
    for (const s of ["one 32-byte descriptor per group", `block bitmap (block ${g0.blockBitmap})`, `inode bitmap (block ${g0.inodeBitmap})`, `inode table (block ${g0.inodeTable})`, `${n(g0.freeBlocks)} blocks and ${n(g0.freeInodes)} inodes`, `at +0x20, names blocks ${g1.blockBitmap}, ${g1.inodeBitmap}, and ${g1.inodeTable}`]) {
      expect(text).toContain(s);
    }
  });

  it("quotes the block bitmap: metadata and journal to 1110, the files to 1125", () => {
    const fresh = FAMILIES.ext.format();
    // Bit i is block i + 1: the first free block of a fresh disk is 1111.
    expect(setBits(fresh.sector(3))).toBe(1110);
    expect(fresh.sector(3)[138]).toBe(0x3f);
    const layout = fresh.layout();
    expect(layout.find((r) => r.kind === "journal")?.sectors).toEqual({ start: 82, end: 1106 });
    expect(fresh.blockOwners().filter((o) => o.path === "<journal>" && o.role === "indirect").map((o) => o.block)).toEqual([1106, 1107, 1108, 1109, 1110]);
    expect(fresh.fileBlocks("/lost+found").data).toEqual([70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81]);
    expect(fresh.fileBlocks("/").data).toEqual([69]);

    const { vol } = runThrough("One disk, two block groups");
    const bitmap = vol.sector(3);
    expect(setBits(bitmap)).toBe(1125);
    expect(bitmap.slice(0, 140).every((b) => b === 0xff)).toBe(true);
    expect(bitmap[0x8c]).toBe(0x1f);
    expect(bitmap.slice(0x8d).every((b) => b === 0)).toBe(true);
    const files = [...vol.fileBlocks(HELLO).data, ...vol.fileBlocks(BIGGER).data, ...vol.fileBlocks(BIGGER).indirect.map((i) => i.block)].sort((a, b) => a - b);
    expect(files).toEqual(Array.from({ length: 15 }, (_, i) => 1111 + i));
    const text = stepText("The block bitmap");
    for (const s of ["starting with block 1", "Blocks 1 to 1110", "(69 to 81)", "(82 to 1110)", "the next 15, 1111 to 1125", "140 bytes of FF", "at +0x8C, 1F", `${n(1125)} bits set`]) {
      expect(text).toContain(s);
    }
  });

  it("quotes the inode bitmap", () => {
    const { vol } = runThrough("One disk, two block groups");
    const sb = vol.extSuperblock();
    expect(sb).toMatchObject({ firstIno: 11, journalInum: 8, inodesPerGroup: 512 });
    expect([vol.inodeNumber("/"), vol.inodeNumber("/lost+found"), vol.inodeNumber(HELLO), vol.inodeNumber(BIGGER)]).toEqual([2, 11, 12, 13]);
    const bitmap = vol.sector(4);
    expect([bitmap[0], bitmap[1]]).toEqual([0xff, 0x1f]);
    // 512 inodes need 64 bytes; the rest of the block is padding, all ones.
    expect(setBits(bitmap.slice(0, 512 / 8))).toBe(13);
    expect(bitmap.slice(0x40).every((b) => b === 0xff)).toBe(true);
    const text = stepText("The inode bitmap");
    for (const s of ["group 0's 512 inodes", "starting with inode 1", "Inodes 1 to 10 are reserved (2 is the root directory, 8 the journal), 11 is lost+found", "took 12 and 13", "FF 1F: 13 bits set", "only 64 bytes", "+0x40 onward reads FF"]) {
      expect(text).toContain(s);
    }
  });

  it("quotes the inode table's size and the slots of inodes 2 and 11", () => {
    const { vol } = runThrough("One disk, two block groups");
    const g = vol.extGeometry();
    expect(g).toMatchObject({ inodesPerGroup: 512, inodeSize: 128, inodeTableBlocks: 64 });
    expect(g.groups[0].inodeTable + g.inodeTableBlocks - 1).toBe(68);
    expect(g.blockSize / g.inodeSize).toBe(8);
    expect(vol.extInode(2).slot).toEqual({ block: 5, offset: 5 * 1024 + 0x80 });
    expect(vol.extInode(11).slot).toEqual({ block: 6, offset: 6 * 1024 + 0x100 });
    expect(vol.extInode(2).mode.toString(8)).toBe("40755");
    expect(vol.extInode(2).block).toHaveLength(15);
    const text = stepText("The inode table");
    for (const s of ["Blocks 5 to 68", "512 inodes of 128 bytes, eight to a block", "(N − 1) × 128", "inode 2, the root directory, is at block 5 + 0x80", "inode 11, lost+found, at block 6 + 0x100", "15 block pointers"]) {
      expect(text).toContain(s);
    }
  });

  it("quotes the root directory's entries and rec_lens", () => {
    const { vol } = runThrough("One disk, two block groups");
    expect(vol.extInode(2).block[0]).toBe(69);
    const entries = vol.dirEntries("/");
    expect(entries.map((e) => [e.name, e.inode, e.recLen])).toEqual([[".", 2, 12], ["..", 2, 12], ["lost+found", 11, 20], ["hello.txt", 12, 20], ["bigger.txt", 13, 960]]);
    expect(entries.reduce((sum, e) => sum + e.recLen, 0)).toBe(1024);
    const text = stepText("The root directory");
    expect(text).toContain("Inode 2's first block pointer reads 69");
    expect(text).toContain(". and .. (both inode 2) take 12 bytes each, lost+found 20, hello.txt 20, and bigger.txt the remaining 960");
  });

  it("quotes hello.txt's inode and its one block", () => {
    const { vol } = runThrough("One disk, two block groups");
    const ino = vol.extInode(12);
    expect(ino.slot).toEqual({ block: 6, offset: 6 * 1024 + 0x180 });
    const slot = vol.sector(6).slice(0x180, 0x200);
    expect(u32(slot, 0x04)).toBe(HELLO_TEXT.length);
    expect(u32(slot, 0x28)).toBe(1111);
    expect(ino.block.slice(1)).toEqual(new Array(14).fill(0));
    // 1111 is the first block a fresh format leaves free.
    expect(FAMILIES.ext.format().sector(3)[138]).toBe(0x3f);
    const text = stepText("hello.txt's inode");
    for (const s of ["names inode 12, at block 6 + 0x180", `size field (+0x04) reads ${HELLO_TEXT.length}`, "first block pointer (+0x28) reads 1111", "the other 14 pointers are zero"]) {
      expect(text).toContain(s);
    }
    const data = vol.sector(1111);
    expect(new TextDecoder().decode(data.slice(0, HELLO_TEXT.length))).toBe(HELLO_TEXT);
    expect(data.slice(HELLO_TEXT.length).every((b) => b === 0)).toBe(true);
    const follow = stepText("Following the pointer");
    expect(follow).toContain(`Block 1111 holds the file's ${HELLO_TEXT.length} bytes, ${HELLO_TEXT}, and ${n(1024 - HELLO_TEXT.length)} zeros`);
    expect(follow).toContain("times 1,024");
  });

  it("quotes bigger.txt's thirteen blocks and its single-indirect block", () => {
    const { vol } = runThrough("One disk, two block groups");
    expect(BIGGER_BYTES).toBe(13 * 1024);
    const blocks = vol.fileBlocks(BIGGER);
    expect(blocks.data).toEqual(Array.from({ length: 13 }, (_, i) => 1112 + i));
    expect(blocks.indirect).toEqual([{ block: 1125, level: 1 }]);
    const ino = vol.extInode(13);
    expect(ino.block.slice(0, 12)).toEqual(blocks.data.slice(0, 12));
    expect(ino.block[12]).toBe(1125);
    expect(u32(vol.sector(6).slice(0x200, 0x280), 0x58)).toBe(1125);
    const pointers = vol.sector(1125);
    expect(u32(pointers, 0)).toBe(1124);
    expect(pointers.slice(4).every((b) => b === 0)).toBe(true);
    const text = stepText("A 13th block needs an indirect block");
    for (const s of [`${n(BIGGER_BYTES)} bytes, 13 blocks: 1112 to 1124`, "only 12 direct pointers", "inode 13's 13th pointer (+0x58)", "single-indirect block, 1125", `${1024 / 4} four-byte block numbers`, "1124, the file's 13th block"]) {
      expect(text).toContain(s);
    }
  });

  it("quotes the journal's blocks, inode, and superblock magic", () => {
    const { vol } = runThrough("One disk, two block groups");
    const info = vol.journalInfo()!;
    expect(info).toMatchObject({ inode: 8, firstBlock: 82, maxlen: 1024 });
    expect(vol.layout().find((r) => r.kind === "journal")?.sectors).toEqual({ start: 82, end: 1106 });
    expect(vol.blockOwners().filter((o) => o.path === "<journal>" && o.role === "indirect").map((o) => o.block)).toEqual([1106, 1107, 1108, 1109, 1110]);
    expect(Array.from(vol.sector(82).slice(0, 4))).toEqual([0xc0, 0x3b, 0x39, 0x98]);
    const text = stepText("The journal");
    for (const s of ["Blocks 82 to 1105", `${n(info.maxlen)} blocks of a hidden file, inode ${info.inode}`, "pointer blocks are 1106 to 1110", "Block 82 is the journal's superblock (magic C0 3B 39 98)"]) {
      expect(text).toContain(s);
    }
  });

  it("quotes group 1's backup superblock, tables, and free blocks", () => {
    const { vol } = runThrough("One disk, two block groups");
    const g = vol.extGeometry();
    const g1 = g.groups[1];
    expect(g1).toMatchObject({ firstBlock: 8193, superblockBlock: 8193, descriptorsBlock: 8194, blockBitmap: 8195, inodeBitmap: 8196, inodeTable: 8197, firstData: 8261, freeBlocks: 8123 });
    expect(g1.inodeTable + g.inodeTableBlocks - 1).toBe(8260);
    expect(g.totalBlocks - g1.firstData).toBe(g1.freeBlocks);
    const backup = vol.sector(8193);
    expect([backup[0x38], backup[0x39]]).toEqual([0x53, 0xef]);
    expect(vol.layout().filter((r) => r.sectors.start >= 8193).map((r) => r.name)).toEqual(["backup superblock (group 1)", "group descriptors (group 1)", "block bitmap (group 1)", "inode bitmap (group 1)", "inode table (group 1)", "data (group 1)"]);
    const text = stepText("Group 1 keeps a backup");
    for (const s of ["begins at block 8193", "the same 53 EF at +0x38", "in block 8194, of the descriptors", "blocks 8195 and 8196", "inode table 8197 to 8260", `8261 onward, are all free: ${n(g1.freeBlocks)} of them`]) {
      expect(text).toContain(s);
    }
  });

  it("resolves every step's focus against the live volume", () => {
    const { fs } = runThrough("One disk, two block groups");
    const focuses = scenario.steps.map((s) => [s.title, focusOf(s.title, fs)]);
    expect(focuses).toEqual([
      ["One disk, two block groups", { sector: 0 }],
      ["Block 1 is the superblock", { offset: 1024 + 0x38 }],
      ["The group descriptors", { sector: 2 }],
      ["The block bitmap", { offset: 3 * 1024 + 0x8c }],
      ["The inode bitmap", { sector: 4 }],
      ["The inode table", { offset: 5 * 1024 + 0x80 }],
      ["The root directory", { path: "/", offset: 69 * 1024 }],
      ["hello.txt's inode", { path: HELLO, offset: 6 * 1024 + 0x180 }],
      ["Following the pointer", { path: HELLO, sector: 1111 }],
      ["A 13th block needs an indirect block", { path: BIGGER, sector: 1125 }],
      ["The journal", { path: null, sector: 82 }],
      ["Group 1 keeps a backup", { sector: 8193 }],
      ["Putting it together", { path: null, sector: 1 }],
    ]);
  });
});
