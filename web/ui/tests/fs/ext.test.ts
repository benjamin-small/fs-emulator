import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { isPseudoOwner, unitIsSector } from "../../src/fs/adapter";
import { defaultColorForRegion } from "../../src/core/attribution";
import { COLOR_BOOT, COLOR_FREE, COLOR_JOURNAL, COLOR_TABLE } from "../../src/core/palette";
import { EXT_SECTOR, EXT_UNIT } from "../../src/fs/ext/geometry";
import { ExtAdapter, JOURNAL_OWNER, MKFS, asExt, ext } from "../../src/fs/ext";
import { fat16 } from "../../src/fs/fat16";
import { buildTree } from "../../src/core/tree";
import { freeSpaceLabel } from "../../src/core/freeSpace";
import { ExtJournal } from "../../src/fs/ext/journal";
import { CORRUPT_NOTE, NOTES, touchesMetadata } from "../../src/fs/ext/metadata";
import { geo, layout, space } from "../fixtures/extGeometry";

const BLOCK = 1024;
const enc = (s: string) => new TextEncoder().encode(s);
/** A one-byte change at `offset`, as a raw write records it. */
const at = (offset: number) => ({ offset, before: new Uint8Array(1), after: new Uint8Array(1) });

describe("the ext unit space", () => {
  it("the fixture is the default ext3 disk", () => {
    const vol = Volume.formatExt3(undefined);
    expect(vol.extGeometry()).toEqual(geo);
    expect(vol.layout()).toEqual(layout);
  });

  it("names the block once: the unit is the sector", () => {
    expect(EXT_UNIT).toEqual({ singular: "block", plural: "blocks", letter: "b", first: 1, fileParts: "its inode, block map, and blocks" });
    expect(EXT_SECTOR).toEqual({ singular: "block", plural: "blocks", letter: "b" });
    expect(space.unit).toBe(EXT_UNIT);
    expect(space.sector).toBe(EXT_SECTOR);
    expect(unitIsSector(space)).toBe(true);
  });

  it("does block arithmetic: every block from firstDataBlock is a unit, block 0 is not", () => {
    expect(space.unitCount).toBe(16383);
    expect(space.unit.first + space.unitCount).toBe(16384); // the attribution tables' size
    expect(space.unitSize).toBe(BLOCK);
    expect(space.sectorSize).toBe(BLOCK);
    expect(space.totalSectors).toBe(16384);
    expect(space.unitOfSector(0)).toBeUndefined();
    expect(space.unitOfSector(1)).toBe(1);
    expect(space.unitOfSector(1111)).toBe(1111);
    expect(space.unitOfSector(16383)).toBe(16383);
    expect(space.unitOfSector(16384)).toBeUndefined();
    expect(space.unitByteRange(1111)).toEqual({ start: 1111 * BLOCK, end: 1112 * BLOCK });
    expect(space.unitOfOffset(1111 * BLOCK + 7)).toBe(1111);
    expect(space.unitOfOffset(100)).toBeUndefined();
    expect(space.unitStartsAt(69)).toBe(true);
    expect(space.unitStartsAt(0)).toBe(false);
  });

  it("colours journal regions COLOR_JOURNAL and every other region by kind", () => {
    expect(layout.map((r) => space.colorForRegion(r))).toEqual([
      COLOR_BOOT, COLOR_BOOT, COLOR_BOOT, COLOR_TABLE, COLOR_TABLE, COLOR_BOOT, COLOR_FREE, COLOR_JOURNAL, COLOR_FREE,
      COLOR_BOOT, COLOR_BOOT, COLOR_TABLE, COLOR_TABLE, COLOR_BOOT, COLOR_FREE,
    ]);
    expect(space.colorForRegion).toBe(defaultColorForRegion); // it already gives journal regions COLOR_JOURNAL
  });
});

describe("ext metadata", () => {
  const journal = Array.from({ length: 1110 - 82 + 1 }, (_, i) => 82 + i); // 82..1105 and the pointer blocks 1106..1110

  it("counts a change as metadata below a group's first data block, in the journal, or in an indirect block", () => {
    expect(touchesMetadata(geo, journal, [1126], [at(0)])).toBe(true);                   // the boot block
    expect(touchesMetadata(geo, journal, [1126], [at(1 * BLOCK + 0x38)])).toBe(true);    // the superblock's magic
    expect(touchesMetadata(geo, journal, [1126], [at(6 * BLOCK + 0x200)])).toBe(true);   // an inode in the table
    expect(touchesMetadata(geo, journal, [1126], [at(68 * BLOCK + 1023)])).toBe(true);   // the last inode-table byte
    expect(touchesMetadata(geo, journal, [1126], [at(90 * BLOCK)])).toBe(true);          // a journal data block
    expect(touchesMetadata(geo, journal, [1126], [at(1107 * BLOCK)])).toBe(true);        // a journal pointer block
    expect(touchesMetadata(geo, journal, [1126], [at(1126 * BLOCK + 4)])).toBe(true);    // a file's single-indirect block
    expect(touchesMetadata(geo, journal, [1126], [at(8195 * BLOCK)])).toBe(true);        // group 1's block bitmap
  });

  it("does not count a change inside directory or file data", () => {
    expect(touchesMetadata(geo, journal, [1126], [at(69 * BLOCK)])).toBe(false);         // the root directory
    expect(touchesMetadata(geo, journal, [1126], [at(1112 * BLOCK), at(1125 * BLOCK)])).toBe(false);
    expect(touchesMetadata(geo, journal, [1126], [at(8261 * BLOCK)])).toBe(false);       // group 1's first data block
    expect(touchesMetadata(geo, [], [], [at(90 * BLOCK)])).toBe(false);                   // no journal (ext2): block 90 is data
    expect(touchesMetadata(geo, journal, [1126], [])).toBe(false);
  });

  it("words the corrupt note and the rewrite clauses for ext", () => {
    expect(CORRUPT_NOTE).toBe("Superblock or group descriptors do not parse; the tree is unavailable until a raw write repairs them.");
    expect(NOTES).toEqual({
      rewrite: "ext keeps the file's blocks and maps new ones after them",
      partialWrite: "the ext volume has no partial writes; the whole file was rewritten over its blocks",
    });
  });
});

describe("ExtJournal", () => {
  function fresh() {
    const vol = Volume.formatExt3(undefined);
    return { vol, journal: new ExtJournal(vol, () => vol.journalInfo()!) };
  }

  it("state() is the journal header without the inode number", () => {
    const { journal } = fresh();
    expect(journal.state()).toEqual({ mode: "ordered", sequence: 1, head: 1, start: 0, maxlen: 1024, firstBlock: 82, maxTransaction: 256, needsRecovery: false });
  });

  it("blocks() reads the ring: a create writes a descriptor, seven copies, and a commit, stale once checkpointed", () => {
    const { vol, journal } = fresh();
    expect(journal.blocks().filter((b) => b.kind !== "unused").map((b) => b.kind)).toEqual(["superblock"]);
    vol.createFile("/hello.txt", enc("Hello, ext3!"));
    const ring = journal.blocks();
    expect(ring).toHaveLength(1024);
    expect(ring[0]).toEqual({ index: 0, block: 82, kind: "superblock", tid: null, home: null, escaped: null, stale: false });
    const used = ring.filter((b) => b.kind !== "unused");
    expect(used.map((b) => b.kind)).toEqual(["superblock", "descriptor", "copy", "copy", "copy", "copy", "copy", "copy", "copy", "commit"]);
    expect(used.filter((b) => b.kind === "copy").map((b) => b.home)).toEqual([1, 2, 3, 4, 5, 6, 69]);
    expect(used.slice(1).every((b) => b.tid === 1 && b.stale)).toBe(true);
    expect(journal.state()).toMatchObject({ sequence: 2, head: 10, start: 0 });
  });

  it("arms, reports, and disarms a crash phase", () => {
    const { journal } = fresh();
    expect(journal.phase()).toBeNull();
    journal.arm("during_checkpoint");
    expect(journal.phase()).toBe("during_checkpoint");
    journal.disarm();
    expect(journal.phase()).toBeNull();
    journal.disarm(); // a no-op when nothing is armed
    expect(journal.phase()).toBeNull();
  });

  it("a crash leaves a live transaction; recover() replays it and clears the flag", () => {
    const { vol, journal } = fresh();
    journal.arm("after_commit");
    const rec = vol.createFile("/crash.txt", new Uint8Array(2 * BLOCK));
    expect(rec.events.at(-1)?.text).toBe("crashed after commit");
    expect(journal.phase()).toBeNull(); // the crash used the armed phase up
    expect(journal.state()).toMatchObject({ needsRecovery: true, start: 1 });
    expect(journal.blocks().filter((b) => b.kind !== "unused" && b.kind !== "superblock").every((b) => !b.stale)).toBe(true);
    const r = journal.recover();
    expect(r.op).toBe("recover");
    expect(r.events[0].text).toBe("scanned the journal from block 1, sequence 1: 1 committed transaction, 7 tagged blocks");
    expect(journal.state()).toMatchObject({ needsRecovery: false, start: 0 });
    expect(vol.listDir("/").map((e) => e.name)).toContain("crash.txt");
  });

  it("recover() on a clean journal is a record with no changes and one event", () => {
    const { journal } = fresh();
    const r = journal.recover();
    expect(r).toEqual({ op: "recover", changes: [], events: [{ kind: "recovery_scanned", text: "journal is clean; nothing to replay", region: { start: 0, end: 0 } }] });
  });
});

/** The default ext3 disk with a directory (inode 12, block 1111), a one-block file (inode 13,
 *  block 1112), and a 13-block file (inode 14, blocks 1113..1125, its single-indirect block
 *  1126). The root's entries sit at block 69 + 0, 12, 24 (lost+found), 44, 56, 76. */
function fixture() {
  const vol = ext.format();
  vol.createDir("/docs");
  vol.createFile("/hello.txt", enc("Hello, ext3!"));
  vol.createFile("/bigger.txt", new Uint8Array(13 * BLOCK));
  return { vol, fs: asExt(ext.bind(vol)) };
}
const range = (from: number, to: number) => Array.from({ length: to - from + 1 }, (_, i) => from + i);

describe("ExtAdapter: identity and unit space", () => {
  it("is the ext family bound to its volume, named by the volume's type", () => {
    const { vol, fs } = fixture();
    expect(fs).toBeInstanceOf(ExtAdapter);
    expect(fs.id).toBe("ext");
    expect(fs.name).toBe("ext3");
    expect(fs.family).toBe(ext);
    expect(fs.vol).toBe(vol);
    expect(ext.bind(ext.format({ variant: "ext2" })).name).toBe("ext2");
    expect(() => asExt(fat16.bind(fat16.format()))).toThrow("not an ext adapter: fat16");
  });

  it("the family binds ext2 and ext3 and formats ext3 by default", () => {
    expect(ext.id).toBe("ext");
    expect(ext.name).toBe("ext");
    expect(ext.fsTypes).toEqual(["ext2", "ext3"]);
    expect(ext.mkfs).toBe(MKFS);
    const vol = ext.format();
    expect(vol.fsType()).toBe("ext3");
    expect(vol.extGeometry()).toEqual(geo);
    expect(vol.journalInfo()?.mode).toBe("ordered");
    expect(ext.format({ variant: "ext2", totalBlocks: 4096 }).fsType()).toBe("ext2");
    expect(ext.format({ totalBlocks: 4096, journalMode: "data" }).journalInfo()?.mode).toBe("data");
    expect(() => ext.format({ variant: "ext2", journalMode: "data" })).toThrow("journalBlocks and journalMode need variant ext3");
  });

  it("does block arithmetic over the volume's geometry", () => {
    const { fs } = fixture();
    expect(fs.unit).toBe(EXT_UNIT);
    expect(fs.sector).toBe(EXT_SECTOR);
    expect(unitIsSector(fs)).toBe(true);
    expect([fs.unitCount, fs.unitSize, fs.sectorSize, fs.totalSectors]).toEqual([16383, BLOCK, BLOCK, 16384]);
    expect(fs.unitOfSector(0)).toBeUndefined();
    expect(fs.unitOfSector(1112)).toBe(1112);
    expect(fs.unitByteRange(1112)).toEqual({ start: 1112 * BLOCK, end: 1113 * BLOCK });
    expect(fs.unitOfOffset(1112 * BLOCK + 3)).toBe(1112);
    expect(fs.unitStartsAt(1112)).toBe(true);
    expect(fs.colorForRegion(layout[7])).toBe(COLOR_JOURNAL);
    expect(fs.geo.groups.map((g) => g.firstData)).toEqual([69, 8261]);
  });
});

describe("ExtAdapter: owners, chains, and slots", () => {
  it("lists every path's blocks with their roles and first block, and none of the journal's data blocks", () => {
    const { fs } = fixture();
    const bigger = (unit: number, role: "data" | "indirect") => ({ unit, path: "/bigger.txt", isDir: false, firstUnit: 1113, role });
    expect(fs.owners.filter((o) => o.path !== "/lost+found" && o.path !== "<journal>")).toEqual([
      { unit: 69, path: "/", isDir: true, firstUnit: 69, role: "directory" },
      { unit: 1111, path: "/docs", isDir: true, firstUnit: 1111, role: "directory" },
      { unit: 1112, path: "/hello.txt", isDir: false, firstUnit: 1112, role: "data" },
      ...range(1113, 1125).map((b) => bigger(b, "data")),
      bigger(1126, "indirect"),
    ]);
    expect(fs.owners.filter((o) => o.path === "/lost+found")).toEqual(range(70, 81).map((unit) => ({ unit, path: "/lost+found", isDir: true, firstUnit: 70, role: "directory" })));
    expect(fs.owners.map((o) => o.unit)).toEqual([...fs.owners.map((o) => o.unit)].sort((a, b) => a - b));
    expect(fs.journalBlocks).toEqual(range(82, 1110)); // the journal's data 82..1105 and its pointer blocks 1106..1110
    expect(fs.ownerOf("/hello.txt")).toEqual({ unit: 1112, path: "/hello.txt", isDir: false, firstUnit: 1112, role: "data" });
    expect(fs.ownerOf("/nope")).toBeUndefined();
  });

  it("keeps the journal's pointer blocks as <journal> rows in the journal colour, and never its data blocks", () => {
    const { fs } = fixture();
    // The pointer blocks 1106..1110 lie in the data region after the journal region (82..1105):
    // units, so they are owned rather than free. The journal's data blocks are not units at all.
    expect(fs.owners.filter((o) => o.path === "<journal>")).toEqual(range(1106, 1110).map((unit) => ({ unit, path: "<journal>", isDir: false, role: "indirect", color: COLOR_JOURNAL, firstUnit: unit })));
    const owned = new Set(fs.owners.map((o) => o.unit));
    expect(range(82, 1105).filter((b) => owned.has(b))).toEqual([]);
    expect(fs.owners.filter((o) => o.color !== undefined).map((o) => o.path)).toEqual(Array(5).fill("<journal>")); // no other row carries a colour
    expect(JOURNAL_OWNER).toBe("<journal>");
    expect(fs.owners.filter((o) => isPseudoOwner(o.path)).map((o) => o.unit)).toEqual(range(1106, 1110)); // no volume path starts with "<"
    for (const b of range(1106, 1110)) expect(fs.describeUnit(b), `block ${b}`).toBe("pointer block of the journal");
  });

  it("an ext2 volume has no journal blocks", () => {
    const fs = asExt(ext.bind(ext.format({ variant: "ext2" })));
    expect(fs.journalBlocks).toEqual([]);
    expect(fs.owners.map((o) => o.unit)).toEqual(range(69, 81)); // the root and lost+found
  });

  it("chains a file's data blocks in logical order, a directory's blocks, and [] where there are none", () => {
    const { vol, fs } = fixture();
    expect(fs.chain("/bigger.txt")).toEqual(range(1113, 1125)); // the pointer block 1126 is not data
    expect(fs.chain("/hello.txt")).toEqual([1112]);
    expect(fs.chain("/docs")).toEqual([1111]);
    expect(fs.chain("/")).toEqual([69]);
    expect(fs.chain("/nope")).toEqual([]);
    vol.createFile("/empty", new Uint8Array(0));
    fs.refresh();
    expect(fs.chain("/empty")).toEqual([]);
  });

  it("takes a path's first block from its chain, not from its lowest block", () => {
    // /a takes block 1111 and /b 1112; with /a deleted, /b grows into the freed 1111, which
    // becomes its second logical block.
    const vol = ext.format();
    vol.createFile("/a", enc("a"));
    vol.createFile("/b", enc("b"));
    vol.deleteFile("/a");
    vol.writeFile("/b", new Uint8Array(2 * BLOCK));
    const fs = asExt(ext.bind(vol));
    expect(fs.chain("/b")).toEqual([1112, 1111]);
    expect(fs.ownerOf("/b")?.firstUnit).toBe(1112);
    expect(fs.owners.filter((o) => o.path === "/b").map((o) => [o.unit, o.firstUnit])).toEqual([[1111, 1112], [1112, 1112]]);
    expect(fs.dataStart("/b")).toBe(1112 * BLOCK);
    expect(buildTree(vol, fs.owners).children.find((c) => c.path === "/b")?.firstUnit).toBe(1112); // the tree's "first block"
  });

  it("gives the tree's root the root directory's block as its first block", () => {
    // ext's root is a directory with a block and an owner row of its own; FAT's root is a fixed
    // region with no row, and its tree root stays null (tests/integration.test.ts).
    const { vol, fs } = fixture();
    expect(fs.ownerOf("/")?.firstUnit).toBe(69);
    expect(buildTree(vol, fs.owners).firstUnit).toBe(69);
  });

  it("gives a path's inode slot as its entry slots, and the inode and entry offsets", () => {
    const { fs } = fixture();
    expect(fs.entrySlots("/hello.txt")).toEqual({ start: 6 * BLOCK + 0x200, end: 6 * BLOCK + 0x280 });
    expect(fs.entrySlots("/docs")).toEqual({ start: 6 * BLOCK + 0x180, end: 6 * BLOCK + 0x200 });
    expect(fs.entrySlots("/")).toEqual({ start: 5 * BLOCK + 0x80, end: 5 * BLOCK + 0x100 });
    expect(fs.entrySlots("/nope")).toBeNull();
    expect(fs.inodeOffset(2)).toBe(5 * BLOCK + 0x80);
    expect(fs.inodeOffset(8)).toBe(5 * BLOCK + 0x380);
    expect(fs.inodeOffset(11)).toBe(6 * BLOCK + 0x100);
    expect(() => fs.inodeOffset(0)).toThrow();
    expect(fs.dirEntryOffset("/docs")).toBe(69 * BLOCK + 44);
    expect(fs.dirEntryOffset("/hello.txt")).toBe(69 * BLOCK + 56);
    expect(fs.dirEntryOffset("/bigger.txt")).toBe(69 * BLOCK + 76);
    expect(fs.dirEntryOffset("/")).toBeNull();
    expect(fs.dirEntryOffset("/nope")).toBeNull();
  });

  it("locates the data of the root, a directory, a file, and nothing for an empty file", () => {
    const { vol, fs } = fixture();
    expect(fs.dataStart("/")).toBe(69 * BLOCK);
    expect(fs.dataStart("/docs")).toBe(1111 * BLOCK);
    expect(fs.dataStart("/bigger.txt")).toBe(1113 * BLOCK);
    expect(fs.dataStart("/nope")).toBeNull();
    vol.createFile("/empty", new Uint8Array(0));
    fs.refresh();
    expect(fs.dataStart("/empty")).toBeNull();
  });

  it("reads region starts from the layout", () => {
    const { fs } = fixture();
    expect(fs.regionStart("boot")).toBe(0);
    expect(fs.regionStart("metadata")).toBe(2);
    expect(fs.regionStart("allocationTable")).toBe(3);
    expect(fs.regionStart("data")).toBe(69);
    expect(fs.regionStart("journal")).toBe(82);
    expect(fs.regionStart("directory")).toBeUndefined();
  });
});

describe("ExtAdapter: what the shell and the Inspector print", () => {
  it("stat returns the ext facts, formatted as the shell prints them", () => {
    const { fs } = fixture();
    const hello = fs.stat("/hello.txt");
    expect(Object.keys(hello)).toEqual(["inode", "inodeOffset", "mode", "links", "blocks512", "dataBlocks", "indirectBlocks", "dirEntryOffset"]);
    expect(hello).toEqual({ inode: 13, inodeOffset: "0x1a00", mode: "0100644", links: 1, blocks512: 2, dataBlocks: [1112], indirectBlocks: [], dirEntryOffset: "0x11438" });
    expect(fs.stat("/bigger.txt")).toEqual({ inode: 14, inodeOffset: "0x1a80", mode: "0100644", links: 1, blocks512: 28, dataBlocks: range(1113, 1125), indirectBlocks: [1126], dirEntryOffset: "0x1144c" });
    expect(fs.stat("/")).toEqual({ inode: 2, inodeOffset: "0x1480", mode: "040755", links: 4, blocks512: 2, dataBlocks: [69], indirectBlocks: [], dirEntryOffset: "" });
  });

  it("df counts blocks from the superblock", () => {
    const { fs } = fixture();
    expect(fs.df()).toEqual({ unitSize: BLOCK, units: 16384, used: 1195, free: 15189 });
    expect(asExt(ext.bind(ext.format())).df()).toEqual({ unitSize: BLOCK, units: 16384, used: 1179, free: 15205 });
  });

  it("labels the ribbon's free space from df, not from the blocks no file owns", () => {
    // 15,205 free blocks of 1 KiB; the blocks no file owns (16.0 MB) include the inode tables,
    // the bitmaps, and the journal.
    const fs = asExt(ext.bind(ext.format()));
    expect(fs.freeUnits()).toBe(15205);
    expect(freeSpaceLabel(fs)).toBe("14.8 MB free");
  });

  it("trace gives the entry, the inode, the pointer blocks, and the first data block", () => {
    const { vol, fs } = fixture();
    expect(fs.trace("/hello.txt")).toEqual([
      { label: "directory entry in / (block 69 + 0x38)", offset: 69 * BLOCK + 0x38 },
      { label: "inode 13 (block 6 + 0x200)", offset: 6 * BLOCK + 0x200 },
      { label: "data block 1112 (first of 1)", offset: 1112 * BLOCK },
    ]);
    expect(fs.trace("/bigger.txt")).toEqual([
      { label: "directory entry in / (block 69 + 0x4c)", offset: 69 * BLOCK + 0x4c },
      { label: "inode 14 (block 6 + 0x280)", offset: 6 * BLOCK + 0x280 },
      { label: "single-indirect block 1126", offset: 1126 * BLOCK },
      { label: "data block 1113 (first of 13)", offset: 1113 * BLOCK },
    ]);
    expect(fs.trace("/")).toEqual([
      { label: "inode 2 (block 5 + 0x80)", offset: 5 * BLOCK + 0x80 },
      { label: "data block 69 (first of 1)", offset: 69 * BLOCK },
    ]);
    vol.createFile("/docs/n.txt", new Uint8Array(0));
    fs.refresh();
    expect(fs.trace("/docs/n.txt")).toEqual([
      { label: "directory entry in /docs (block 1111 + 0x18)", offset: 1111 * BLOCK + 0x18 },
      { label: "inode 15 (block 6 + 0x300)", offset: 6 * BLOCK + 0x300 },
      { label: "no data blocks", offset: null },
    ]);
    expect(fs.trace("/nope")).toEqual([{ label: "no data blocks", offset: null }]);
  });

  it("names a double-indirect block and the level-1 blocks under it", () => {
    const vol = ext.format();
    vol.createFile("/huge.bin", new Uint8Array(269 * BLOCK)); // 12 direct + 256 single + 1 through the double
    const fs = asExt(ext.bind(vol));
    expect(fs.trace("/huge.bin").map((r) => r.label)).toEqual([
      "directory entry in / (block 69 + 0x2c)",
      "inode 12 (block 6 + 0x180)",
      "single-indirect block 1380",
      "double-indirect block 1381",
      "indirect block 1382 (level 1, under the double)",
      "data block 1111 (first of 269)",
    ]);
    expect(fs.describeUnit(1380)).toBe("single-indirect block of /huge.bin");
    expect(fs.describeUnit(1381)).toBe("double-indirect block of /huge.bin");
    expect(fs.describeUnit(1382)).toBe("indirect block of /huge.bin (level 1)");
    expect(fs.describeUnit(1379)).toBe("data block 268 of /huge.bin");
  });

  it("describes what a block holds for its owner, free, or nothing outside the data area", () => {
    const { fs } = fixture();
    expect(fs.describeUnit(1112)).toBe("data block 0 of /hello.txt");
    expect(fs.describeUnit(1125)).toBe("data block 12 of /bigger.txt");
    expect(fs.describeUnit(1126)).toBe("single-indirect block of /bigger.txt");
    expect(fs.describeUnit(69)).toBe("directory block 0 of /");
    expect(fs.describeUnit(81)).toBe("directory block 11 of /lost+found");
    expect(fs.describeUnit(1111)).toBe("directory block 0 of /docs");
    expect(fs.describeUnit(1127)).toBe("free");
    expect(fs.describeUnit(16383)).toBe("free");
    for (const b of [0, 1, 5, 68, 90, 1105, 8193, 8260, 16384, -1]) expect(fs.describeUnit(b), `block ${b}`).toBeNull();
  });

  it("annotates a block through the volume: block 1 is the superblock's fields", () => {
    const { fs } = fixture();
    const fields = fs.annotateSector(1);
    expect(fields[0]).toEqual({ range: { start: 0, end: 4 }, label: "inodes count", value: "1024" });
    expect(fields.find((a) => a.label === "magic")).toEqual({ range: { start: 0x38, end: 0x3a }, label: "magic", value: "0xEF53" });
  });

  it("parses i:N as inode N's slot and leaves every other form to the shell", () => {
    const { fs } = fixture();
    expect(fs.parseAddr("i:2")).toBe(5 * BLOCK + 0x80);
    expect(fs.parseAddr("I:13")).toBe(6 * BLOCK + 0x200);
    for (const v of ["i:0", "i:1025", "i:", "i:x", "b:3", "12"]) expect(fs.parseAddr(v), v).toBeUndefined();
    expect(fs.extraAddrHelp).toBe(", i:11 (inode)");
  });

  it("traces, stats, counts, describes, and parses ext2 the way it does ext3", () => {
    // The fixture's three paths on each variant. ext2 has no journal, so its first free block is
    // 82, right after lost+found; block 90 is bigger's seventh data block there and a journal
    // block on ext3.
    const cases = [
      { variant: "ext2", docs: 82, hello: 83, bigger: 84, used: 166, free: 16218, at90: "data block 6 of /bigger.txt" },
      { variant: "ext3", docs: 1111, hello: 1112, bigger: 1113, used: 1195, free: 15189, at90: null },
    ] as const;
    for (const c of cases) {
      const vol = ext.format({ variant: c.variant });
      vol.createDir("/docs");
      vol.createFile("/hello.txt", enc("Hello, ext3!"));
      vol.createFile("/bigger.txt", new Uint8Array(13 * BLOCK));
      const fs = asExt(ext.bind(vol));
      const pointer = c.bigger + 13;
      expect(fs.trace("/bigger.txt"), c.variant).toEqual([
        { label: "directory entry in / (block 69 + 0x4c)", offset: 69 * BLOCK + 0x4c },
        { label: "inode 14 (block 6 + 0x280)", offset: 6 * BLOCK + 0x280 },
        { label: `single-indirect block ${pointer}`, offset: pointer * BLOCK },
        { label: `data block ${c.bigger} (first of 13)`, offset: c.bigger * BLOCK },
      ]);
      expect(fs.stat("/hello.txt"), c.variant).toEqual({ inode: 13, inodeOffset: "0x1a00", mode: "0100644", links: 1, blocks512: 2, dataBlocks: [c.hello], indirectBlocks: [], dirEntryOffset: "0x11438" });
      expect(fs.stat("/bigger.txt").indirectBlocks, c.variant).toEqual([pointer]);
      expect(fs.df(), c.variant).toEqual({ unitSize: BLOCK, units: 16384, used: c.used, free: c.free });
      expect(fs.describeUnit(c.docs), c.variant).toBe("directory block 0 of /docs");
      expect(fs.describeUnit(c.hello), c.variant).toBe("data block 0 of /hello.txt");
      expect(fs.describeUnit(pointer), c.variant).toBe("single-indirect block of /bigger.txt");
      expect(fs.describeUnit(pointer + 1), c.variant).toBe("free");
      expect(fs.describeUnit(90), c.variant).toBe(c.at90);
      expect(fs.describeUnit(68), c.variant).toBeNull();
      expect(fs.parseAddr("i:13"), c.variant).toBe(6 * BLOCK + 0x200);
      expect(fs.parseAddr("i:1025"), c.variant).toBeUndefined();
    }
  });

  it("matches names exactly and flags inode writes as metadata, not data writes", () => {
    const { vol, fs } = fixture();
    expect(fs.namesMatch("hello.txt", "hello.txt")).toBe(true);
    expect(fs.namesMatch("hello.txt", "HELLO.TXT")).toBe(false);
    expect(fs.touchesMetadata(vol.writeRaw(1112 * BLOCK, enc("J")).changes)).toBe(false);                    // hello's data
    expect(fs.touchesMetadata(vol.writeRaw(6 * BLOCK + 0x200 + 0x1a, new Uint8Array([1])).changes)).toBe(true); // hello's inode (i_links_count)
    expect(fs.touchesMetadata([at(1126 * BLOCK)])).toBe(true);                                                // bigger's indirect block
    expect(fs.touchesMetadata([at(1107 * BLOCK)])).toBe(true);                                                // a journal pointer block
    expect(fs.touchesMetadata(vol.createFile("/new.txt", enc("x")).changes)).toBe(true);
    expect(fs.touchesMetadata([])).toBe(false);
  });

  it("words the corrupt note and the rewrite clauses for ext", () => {
    const { fs } = fixture();
    expect(fs.corruptNote).toBe(CORRUPT_NOTE);
    expect(fs.notes).toBe(NOTES);
  });
});

describe("ExtAdapter: caches and the journal capability", () => {
  it("answers from the last refresh() until the next one", () => {
    const { vol, fs } = fixture();
    vol.createFile("/new.txt", enc("x"));
    expect(fs.ownerOf("/new.txt")).toBeUndefined();
    expect(fs.df().free).toBe(15189);
    expect(fs.journal!.state().sequence).toBe(4);
    fs.refresh();
    expect(fs.ownerOf("/new.txt")).toEqual({ unit: 1127, path: "/new.txt", isDir: false, firstUnit: 1127, role: "data" });
    expect(fs.df().free).toBe(15188);
    expect(fs.journal!.state().sequence).toBe(5);
  });

  it("has a journal on ext3 only, and nothing to recover after a clean format", () => {
    const { fs } = fixture();
    expect(fs.journal).toBeInstanceOf(ExtJournal);
    expect(fs.needsRecovery).toBe(false);
    expect(fs.journal!.state()).toEqual({ mode: "ordered", sequence: 4, head: 30, start: 0, maxlen: 1024, firstBlock: 82, maxTransaction: 256, needsRecovery: false });
    const journal = fs.journal;
    fs.refresh();
    expect(fs.journal).toBe(journal); // the same capability across refreshes
    const ext2 = asExt(ext.bind(ext.format({ variant: "ext2" })));
    expect(ext2.journal).toBeUndefined();
    expect(ext2.needsRecovery).toBe(false);
  });

  it("the journal's blocks after a create: the ring the Journal panel draws", () => {
    const vol = ext.format();
    const fs = asExt(ext.bind(vol));
    vol.createFile("/hello.txt", enc("Hello, ext3!"));
    fs.refresh();
    const used = fs.journal!.blocks().filter((b) => b.kind !== "unused");
    expect(used.map((b) => [b.index, b.block, b.kind])).toEqual([
      [0, 82, "superblock"], [1, 83, "descriptor"],
      [2, 84, "copy"], [3, 85, "copy"], [4, 86, "copy"], [5, 87, "copy"], [6, 88, "copy"], [7, 89, "copy"], [8, 90, "copy"],
      [9, 91, "commit"],
    ]);
  });

  it("a crash makes the volume need recovery; recover() returns the record that clears it", () => {
    const { vol, fs } = fixture();
    const journal = fs.journal!;
    journal.arm("after_commit");
    expect(journal.phase()).toBe("after_commit");
    vol.createFile("/crash.txt", new Uint8Array(2 * BLOCK));
    expect(fs.needsRecovery).toBe(false); // the cache, until refresh()
    fs.refresh();
    expect(fs.needsRecovery).toBe(true);
    expect(journal.state().needsRecovery).toBe(true);
    expect(() => vol.createFile("/more.txt", enc("x"))).toThrow();
    const rec = journal.recover();
    expect(rec.op).toBe("recover");
    expect(rec.changes.length).toBeGreaterThan(0);
    fs.refresh();
    expect(fs.needsRecovery).toBe(false);
    expect(journal.state().needsRecovery).toBe(false);
    expect(fs.ownerOf("/crash.txt")?.firstUnit).toBe(1127);
  });
});
