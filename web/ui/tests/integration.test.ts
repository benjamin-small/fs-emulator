import { describe, expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";
import { applyChanges, changedSectors, touchesBootSector } from "../src/core/patch";
import { attrAtOffset, buildAttribution, clusterByteRange } from "../src/core/attribution";
import { buildTree } from "../src/core/tree";
import { findEntrySlots } from "../src/core/direntry";
import type { OpRecord, Region, Volume as VolumeType } from "../src/lib/wasm";

describe("package integration", () => {
  it("seek round trip: the same loops VolumeStore.seek runs reproduce every step's image", () => {
    const vol = Volume.formatFat16(undefined);
    const fresh = Buffer.from(vol.image()); // the disk before any operation (cursor -1)
    const ops: ((v: VolumeType) => OpRecord)[] = [
      (v) => v.createFile("/A.TXT", new TextEncoder().encode("alpha")),
      (v) => v.createDir("/D"),
      (v) => v.writeRaw(43, new TextEncoder().encode("SHELLDISK  ")), // the volume label, inside sector 0
      (v) => v.createFile("/D/B.BIN", new Uint8Array(3000)),
      (v) => v.deleteFile("/A.TXT"),
    ];
    const history: OpRecord[] = [];
    const snaps: Buffer[] = [];
    for (const op of ops) { history.push(op(vol)); snaps.push(Buffer.from(vol.image())); }
    // The raw write journals one change for exactly the written range and lands in sector 0.
    expect(history[2].changes).toHaveLength(1);
    expect(history[2].changes[0].offset).toBe(43);
    expect(Array.from(history[2].changes[0].after)).toEqual(Array.from(new TextEncoder().encode("SHELLDISK  ")));
    expect(changedSectors(history[2].changes, 512)).toEqual([0]);
    expect(touchesBootSector(history[2].changes)).toBe(true);
    expect(history.filter((_, i) => i !== 2).some((r) => touchesBootSector(r.changes))).toBe(false);

    // The cached image the store patches, starting where the store's cursor is: the last step.
    const image = vol.image();
    let cursor = history.length - 1;
    const imageAt = (step: number) => (step === -1 ? fresh : snaps[step]);

    for (let step = cursor - 1; step >= -1; step--) {
      for (let i = cursor; i > step; i--) applyChanges(image, history[i].changes, "reverse"); // seek(), step < cursor
      cursor = step;
      expect(Buffer.compare(Buffer.from(image), imageAt(step)), `reverse to step ${step}`).toBe(0);
    }
    for (let step = 0; step < history.length; step++) {
      for (let i = cursor + 1; i <= step; i++) applyChanges(image, history[i].changes, "forward"); // seek(), step > cursor
      cursor = step;
      expect(Buffer.compare(Buffer.from(image), imageAt(step)), `forward to step ${step}`).toBe(0);
    }
    expect(Buffer.compare(Buffer.from(image), Buffer.from(vol.image()))).toBe(0);
  });
  it("patching the cached image from OpRecord.changes matches vol.image()", () => {
    const vol = Volume.formatFat16(undefined);
    const image = vol.image();
    const rec = vol.createFile("/Hello world.txt", new TextEncoder().encode("hello from the ui"));
    applyChanges(image, rec.changes, "forward");
    expect(Buffer.compare(Buffer.from(image), Buffer.from(vol.image()))).toBe(0);
    expect(changedSectors(rec.changes, 512)).toContain(97);
    const rec2 = vol.deleteFile("/Hello world.txt");
    applyChanges(image, rec2.changes, "forward");
    expect(Buffer.compare(Buffer.from(image), Buffer.from(vol.image()))).toBe(0);
    applyChanges(image, rec2.changes, "reverse");
    applyChanges(image, rec.changes, "reverse");
    expect(Buffer.compare(Buffer.from(image), Buffer.from(Volume.formatFat16(undefined).image()))).toBe(0);
  });
  it("attribution names the file that owns a cluster and the tree lists it", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createDir("/DOCS");
    vol.createFile("/DOCS/N.TXT", new Uint8Array(3000));
    const owners = vol.clusterOwners();
    const t = buildAttribution(vol.geometry(), vol.layout(), owners);
    const { start } = clusterByteRange(vol.geometry(), 3);
    expect(attrAtOffset(t, start)).toMatchObject({ cluster: 3, ownerPath: "/DOCS/N.TXT", isDir: false });
    const tree = buildTree(vol, owners);
    expect(tree.children[0]).toMatchObject({ name: "DOCS", path: "/DOCS", isDir: true, firstCluster: 2 });
    expect(tree.children[0].children[0]).toMatchObject({ name: "N.TXT", path: "/DOCS/N.TXT", size: 3000, firstCluster: 3 });
  });
  it("finds a file's directory entry slots, including LFN entries, in root and subdirectories", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createFile("/My long name.txt", new Uint8Array(1));
    vol.createDir("/D");
    vol.createFile("/D/inner.txt", new Uint8Array(1));
    const g = vol.geometry(), fat = vol.fatEntries(0), owners = vol.clusterOwners();
    const root = g.firstRootDirSector * g.bytesPerSector;
    expect(findEntrySlots(vol, g, fat, owners, "/My long name.txt")).toEqual({ start: root, end: root + 3 * 32 });   // 2 LFN + short
    expect(findEntrySlots(vol, g, fat, owners, "/D")).toEqual({ start: root + 3 * 32, end: root + 4 * 32 });
    const dStart = clusterByteRange(g, owners.find((o) => o.path === "/D")!.firstCluster).start; // /D's own cluster
    expect(findEntrySlots(vol, g, fat, owners, "/D/inner.txt")).toEqual({ start: dStart + 2 * 32, end: dStart + 3 * 32 });   // after . and ..
    expect(findEntrySlots(vol, g, fat, owners, "/nope")).toBeNull();
    expect(findEntrySlots(vol, g, fat, owners, "/")).toBeNull();
  });
  it("a raw write that changes the root-entry count is adopted: geometry and layout follow", () => {
    // 16 root entries fill exactly one 512-byte sector; 32 fill two. VolumeStore.run re-reads
    // geometry and layout whenever touchesBootSector(rec.changes) is true; this replays the
    // same wasm calls and checks that the volume reports the grown root region.
    const vol = Volume.formatFat16({ rootEntries: 16 });
    const before = vol.geometry();
    const layoutBefore = vol.layout();
    expect(before.rootEntries).toBe(16);
    expect(before.rootDirSectors).toBe(1);

    const rec = vol.writeRaw(17, new Uint8Array([32, 0])); // BPB_RootEntCnt, u16 little-endian
    expect(rec.changes).toHaveLength(1);
    expect(rec.changes[0].offset).toBe(17);
    expect(Array.from(rec.changes[0].before)).toEqual([16, 0]);
    expect(Array.from(rec.changes[0].after)).toEqual([32, 0]);
    expect(touchesBootSector(rec.changes)).toBe(true);

    const after = vol.geometry();
    const layoutAfter = vol.layout();
    expect(after.rootEntries).toBe(32);
    expect(after.rootDirSectors).toBe(2);
    expect(after.firstRootDirSector).toBe(before.firstRootDirSector);
    expect(after.firstDataSector).toBe(before.firstDataSector + 1);
    expect(after.bytesPerSector).toBe(before.bytesPerSector);
    expect(after.totalSectors).toBe(before.totalSectors);

    const root = (l: Region[]) => l.find((r) => r.kind === "directory")!;
    const data = (l: Region[]) => l.find((r) => r.kind === "data")!;
    expect(root(layoutBefore).sectors.start).toBe(before.firstRootDirSector);
    expect(root(layoutBefore).sectors.end).toBe(before.firstDataSector);
    expect(root(layoutAfter).sectors.start).toBe(before.firstRootDirSector);
    expect(root(layoutAfter).sectors.end).toBe(before.firstDataSector + 1); // grew by one sector
    expect(data(layoutAfter).sectors.start).toBe(data(layoutBefore).sectors.start + 1);
    expect(data(layoutAfter).sectors.end).toBe(data(layoutBefore).sectors.end);

    // Path operations keep working on the adopted geometry, and an ordinary op
    // (root entry, FAT, data cluster) never lands in sector 0.
    expect(vol.listDir("/")).toEqual([]);
    const rec2 = vol.createFile("/A.TXT", new TextEncoder().encode("a"));
    expect(touchesBootSector(rec2.changes)).toBe(false);
    expect(vol.listDir("/").map((e) => e.name)).toEqual(["A.TXT"]);
  });
});
