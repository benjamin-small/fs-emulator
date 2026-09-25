import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { adapterFor } from "../../src/fs";
import { asFat16, fat16, findEntrySlots } from "../../src/fs/fat16";
import { unitIsSector, type FsAdapter } from "../../src/fs/adapter";
import { buildTree } from "../../src/core/tree";
import { scanZeroSectors } from "../../src/core/zeros";

const enc = (s: string) => new TextEncoder().encode(s);
const ROOT = 65 * 512;      // the root directory's first byte on the default disk
const DATA = 97 * 512;      // cluster 2's first byte
const CLUSTER = 2048;

/** The read-commands fixture: a directory (cluster 2), a two-cluster file in it (3, 4), and a
 *  long-named root file (5) whose entry takes two LFN slots and a short one. */
function fixture() {
  const vol = Volume.formatFat16(undefined);
  vol.createDir("/DOCS");
  vol.createFile("/DOCS/N.TXT", new Uint8Array(3000));
  vol.createFile("/Hello world.txt", enc("hello from the shell\n"));
  return { vol, fs: asFat16(adapterFor(vol)) };
}

describe("Fat16Adapter: identity and unit space", () => {
  it("is the fat16 family bound to its volume", () => {
    const { vol, fs } = fixture();
    expect(fs.id).toBe("fat16");
    expect(fs.name).toBe("FAT16");
    expect(fs.family).toBe(fat16);
    expect(fs.vol).toBe(vol);
    const alien = new Proxy(fs, { get: (t, k, r) => (k === "id" ? "ext3" : Reflect.get(t, k, r)) });
    expect(() => asFat16(alien)).toThrow("not a FAT16 adapter: ext3");
  });

  it("names the unit and reports the default geometry", () => {
    const { fs } = fixture();
    expect(fs.unit).toEqual({ singular: "cluster", plural: "clusters", letter: "c", first: 2, fileParts: "its entry, chain, and clusters" });
    expect(fs.unitCount).toBe(8167);
    expect(fs.unitSize).toBe(CLUSTER);
    expect(fs.sectorSize).toBe(512);
    expect(fs.totalSectors).toBe(32768);
  });

  it("names its sector apart from its cluster, and has no journal to recover", () => {
    const { fs } = fixture();
    expect(fs.sector).toEqual({ singular: "sector", plural: "sectors", letter: "s" });
    expect(unitIsSector(fs)).toBe(false);
    expect(fs.needsRecovery).toBe(false);
    const generic: FsAdapter = fs; // the optional members, as the chrome and the shell see them
    expect(generic.journal).toBeUndefined();
    expect(generic.extraAddrHelp).toBeUndefined();
    expect(fat16.fsTypes).toEqual(["FAT16"]);
  });

  it("does cluster arithmetic like the core", () => {
    const { fs } = fixture();
    expect(fs.unitOfSector(96)).toBeUndefined();
    expect(fs.unitOfSector(97)).toBe(2);
    expect(fs.unitOfSector(101)).toBe(3);
    expect(fs.unitByteRange(2)).toEqual({ start: DATA, end: DATA + CLUSTER });
    expect(fs.unitOfOffset(DATA + 7)).toBe(2);
    expect(fs.unitOfOffset(0)).toBeUndefined();
    expect(fs.unitStartsAt(97)).toBe(true);
    expect(fs.unitStartsAt(98)).toBe(false);
    expect(fs.unitStartsAt(101)).toBe(true);
    expect(fs.unitStartsAt(65)).toBe(false); // not a data sector
  });

  it("colours FAT 1 apart from FAT 0", () => {
    const { fs } = fixture();
    const [boot, fat0, fat1, root, data] = fs.vol.layout();
    expect(fs.colorForRegion(fat0)).toBe(2);
    expect(fs.colorForRegion(fat1)).toBe(10);
    expect(fs.colorForRegion(root)).toBe(3);
    expect(fs.colorForRegion(data)).toBe(0);
    expect(fs.colorForRegion(boot)).toBe(1);
  });
});

describe("Fat16Adapter: owners, chains, and entries", () => {
  it("mirrors the wasm owners in the generic shape, sorted by unit", () => {
    const { fs } = fixture();
    expect(fs.owners).toEqual([
      { unit: 2, path: "/DOCS", isDir: true, firstUnit: 2 },
      { unit: 3, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3 },
      { unit: 4, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3 },
      { unit: 5, path: "/Hello world.txt", isDir: false, firstUnit: 5 },
    ]);
    expect(fs.ownerOf("/DOCS/N.TXT")).toEqual({ unit: 3, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3 });
    expect(fs.ownerOf("/nope")).toBeUndefined();
  });

  it("follows a file's chain and returns [] where there is none", () => {
    const { fs } = fixture();
    expect(fs.chain("/DOCS/N.TXT")).toEqual([3, 4]);
    expect(fs.chain("/DOCS")).toEqual([2]);
    expect(fs.chain("/")).toEqual([]);
    expect(fs.chain("/nope")).toEqual([]);
  });

  it("finds the entry slots of a root entry, a subdirectory entry, an LFN name, and none for /", () => {
    const { vol, fs } = fixture();
    expect(fs.entrySlots("/DOCS")).toEqual({ start: ROOT, end: ROOT + 32 });
    expect(fs.entrySlots("/Hello world.txt")).toEqual({ start: ROOT + 32, end: ROOT + 4 * 32 }); // 2 LFN + short
    expect(fs.entrySlots("/DOCS/N.TXT")).toEqual({ start: DATA + 2 * 32, end: DATA + 3 * 32 });   // after . and ..
    expect(fs.entrySlots("/")).toBeNull();
    expect(fs.entrySlots("/nope")).toBeNull();
    expect(fs.entrySlots("/DOCS/N.TXT")).toEqual(findEntrySlots(vol, vol.geometry(), vol.fatEntries(0), vol.clusterOwners(), "/DOCS/N.TXT"));
  });

  it("reports remnants after a delete: the freed slots and the dirty free cluster", () => {
    const { vol, fs } = fixture();
    expect(fs.remnants(scanZeroSectors(vol.image(), 512))).toEqual([]);
    vol.deleteFile("/Hello world.txt");
    fs.refresh();
    const r = fs.remnants(scanZeroSectors(vol.image(), 512));
    expect(r).toContainEqual({ start: ROOT + 32, end: ROOT + 4 * 32 });
    expect(r).toContainEqual(fs.unitByteRange(5));
    expect(fs.ownerOf("/Hello world.txt")).toBeUndefined();
  });

  it("locates the data of the root, a file, and nothing for an empty file", () => {
    const { vol, fs } = fixture();
    expect(fs.dataStart("/")).toBe(ROOT);
    expect(fs.dataStart("/DOCS/N.TXT")).toBe(DATA + CLUSTER);
    expect(fs.dataStart("/nope")).toBeNull();
    vol.createFile("/EMPTY", new Uint8Array(0));
    fs.refresh();
    expect(fs.dataStart("/EMPTY")).toBeNull();
  });

  it("reads region starts from the layout", () => {
    const { fs } = fixture();
    expect(fs.regionStart("boot")).toBe(0);
    expect(fs.regionStart("allocationTable")).toBe(1); // FAT 0, the first of that kind
    expect(fs.regionStart("directory")).toBe(65);
    expect(fs.regionStart("data")).toBe(97);
    expect(fs.regionStart("metadata")).toBeUndefined();
  });
});

describe("Fat16Adapter: what the shell and the Inspector print", () => {
  it("stat returns exactly the seven FAT keys, formatted as the shell prints them", () => {
    const { vol, fs } = fixture();
    const n = fs.stat("/DOCS/N.TXT");
    expect(Object.keys(n)).toEqual(["firstCluster", "chain", "clusters", "entryOffset", "entrySlots", "fatEntryOffset", "dataOffset"]);
    expect(n).toEqual({
      firstCluster: 3, chain: [3, 4], clusters: 2,
      entryOffset: "0xc240", entrySlots: "0xc240-0xc260", fatEntryOffset: "0x206", dataOffset: "0xca00",
    });
    expect(fs.stat("/")).toEqual({ firstCluster: 0, chain: [], clusters: 0, entryOffset: "", entrySlots: "", fatEntryOffset: "", dataOffset: "0x8200" });
    vol.createFile("/EMPTY", new Uint8Array(0));
    fs.refresh();
    const empty = fs.stat("/EMPTY");
    expect(empty).toMatchObject({ firstCluster: 0, chain: [], clusters: 0, fatEntryOffset: "", dataOffset: "" });
    expect(empty.entryOffset).toBe("0x8280"); // root slot 4, after DOCS and the three Hello world.txt slots
    expect(empty.entrySlots).toBe("0x8280-0x82a0");
  });

  it("df counts used clusters from FAT 0", () => {
    const { fs } = fixture();
    expect(fs.df()).toEqual({ unitSize: CLUSTER, units: 8167, used: 4, free: 8163 });
  });

  it("trace gives the Inspector its three rows, or the muted no-data row", () => {
    const { vol, fs } = fixture();
    expect(fs.trace("/DOCS/N.TXT")).toEqual([
      { label: "Directory entry · offset 0xc240", offset: 0xc240 },
      { label: "FAT chain · 2 clusters starting at 3 · FAT entry at 0x206", offset: 0x206 },
      { label: "Data · cluster 3 at 0xca00", offset: 0xca00 },
    ]);
    vol.createFile("/EMPTY", new Uint8Array(0));
    fs.refresh();
    expect(fs.trace("/EMPTY")).toEqual([
      { label: "Directory entry · offset 0x8280", offset: 0x8280 },
      { label: "Data · no data clusters", offset: null },
    ]);
    expect(fs.trace("/nope")).toEqual([{ label: "Data · no data clusters", offset: null }]);
  });

  it("describes a FAT entry the way the Inspector's card does", () => {
    const { fs } = fixture();
    expect(fs.describeUnit(2)).toBe("FAT: end of chain");
    expect(fs.describeUnit(3)).toBe("FAT: next → 4");
    expect(fs.describeUnit(6)).toBe("FAT: free");
    expect(fs.describeUnit(0)).toBe("FAT: reserved");
    expect(fs.describeUnit(100000)).toBeNull();
  });

  it("annotates a sector with the cached owners", () => {
    const { vol, fs } = fixture();
    expect(fs.annotateSector(0)).toEqual(vol.annotateSectorWith(0, vol.clusterOwners()));
    expect(fs.annotateSector(65)).toEqual(vol.annotateSectorWith(65, vol.clusterOwners()));
    expect(fs.annotateSector(65).length).toBeGreaterThan(0);
  });

  it("matches names case-insensitively and flags boot-sector writes as metadata changes", () => {
    const { fs } = fixture();
    expect(fs.namesMatch("hello world.txt", "HELLO WORLD.TXT")).toBe(true);
    expect(fs.namesMatch("a.txt", "b.txt")).toBe(false);
    const c = (offset: number) => ({ offset, before: new Uint8Array(1), after: new Uint8Array(1) });
    expect(fs.touchesMetadata([c(511)])).toBe(true);
    expect(fs.touchesMetadata([c(1000), c(17)])).toBe(true);
    expect(fs.touchesMetadata([c(512), c(4096)])).toBe(false);
    expect(fs.touchesMetadata([])).toBe(false);
  });

  it("locates a cluster's FAT entry in either copy", () => {
    const { fs } = fixture();
    expect(fs.fatEntryOffset(3)).toBe(1 * 512 + 3 * 2);
    expect(fs.fatEntryOffset(2, 1)).toBe((1 + 32) * 512 + 2 * 2);
    expect(fs.corruptNote).toBe("Boot sector does not parse; the tree is unavailable until a raw write repairs it.");
    expect(fs.notes).toEqual({
      rewrite: "FAT has no append; the old chain is freed and reallocated",
      partialWrite: "FAT has no partial writes; the whole file was rewritten",
    });
  });
});

describe("Fat16Adapter: caches", () => {
  it("answers from the last refresh() until the next one", () => {
    const { vol, fs } = fixture();
    vol.createFile("/NEW.TXT", new Uint8Array(1));
    expect(fs.ownerOf("/NEW.TXT")).toBeUndefined();
    expect(fs.chain("/NEW.TXT")).toEqual([]);
    expect(fs.fat[6]).toEqual({ kind: "free" });
    expect(fs.df().used).toBe(4);
    fs.refresh();
    expect(fs.ownerOf("/NEW.TXT")).toEqual({ unit: 6, path: "/NEW.TXT", isDir: false, firstUnit: 6 });
    expect(fs.chain("/NEW.TXT")).toEqual([6]);
    expect(fs.fat[6]).toEqual({ kind: "endOfChain" });
    expect(fs.df().used).toBe(5);
  });

  it("re-reads the geometry: a boot-sector write that grows the root directory moves the data area", () => {
    const vol = Volume.formatFat16({ rootEntries: 16 });
    const fs = asFat16(adapterFor(vol));
    expect(fs.dataStart("/")).toBe(65 * 512);
    expect(fs.unitByteRange(2).start).toBe(66 * 512);
    const rec = vol.writeRaw(17, new Uint8Array([32, 0])); // BPB_RootEntCnt, u16 little-endian
    expect(fs.touchesMetadata(rec.changes)).toBe(true);
    fs.refresh();
    expect(fs.unitByteRange(2).start).toBe(67 * 512);
    expect(fs.regionStart("data")).toBe(67);
  });

  it("survives a corrupt volume as the store did, and sees it again once repaired", () => {
    const { vol, fs } = fixture();
    const slots = fs.entrySlots("/DOCS/N.TXT");
    const saved = vol.readRaw(0, 512);
    expect(vol.corruption()).toBeNull();
    vol.writeRaw(0, new Uint8Array(512));
    expect(vol.corruption()).toContain("boot sector no longer parses");
    // The same three unguarded wasm calls refreshMeta made: the FAT crate answers from its
    // last good geometry, so the owners are still there; only the path-based walks are gated.
    expect(() => fs.refresh()).not.toThrow();
    expect(fs.owners).toHaveLength(4);
    expect(fs.chain("/DOCS/N.TXT")).toEqual([3, 4]);
    expect(fs.entrySlots("/DOCS/N.TXT")).toBeNull();
    expect(fs.remnants(scanZeroSectors(vol.image(), 512))).toEqual([]);
    // The owner list is irrelevant to an empty root: listDir throws CorruptImage and buildTree absorbs it.
    expect(buildTree(vol, [])).toMatchObject({ name: "/", path: "/", isDir: true, children: [] });
    vol.writeRaw(0, saved);
    expect(vol.corruption()).toBeNull();
    fs.refresh();
    expect(fs.entrySlots("/DOCS/N.TXT")).toEqual(slots);
    expect(fs.owners).toHaveLength(4);
    expect(buildTree(vol, []).children.map((c) => c.name)).toEqual(["DOCS", "Hello world.txt"]);
  });
});
