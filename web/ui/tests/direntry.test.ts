import { describe, expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";
import { clusterByteRange } from "../src/fs/fat16/geometry";
import { findEntrySlots, slotOffset } from "../src/fs/fat16/direntry";

const ENTRY = 32;

describe("directory slot offsets", () => {
  const vol = Volume.formatFat16(undefined);
  vol.createDir("/D");
  vol.createFile("/D/inner.txt", new Uint8Array(1));
  const g = vol.geometry(), fat = vol.fatEntries(0), owners = vol.clusterOwners();
  const perCluster = (g.bytesPerSector * g.sectorsPerCluster) / ENTRY;
  const dirStart = clusterByteRange(g, owners.find((o) => o.path === "/D")!.firstCluster).start;

  it("resolves slots inside the directory's cluster chain", () => {
    expect(slotOffset(g, fat, owners, "/D", 0)).toBe(dirStart);
    expect(slotOffset(g, fat, owners, "/D", perCluster - 1)).toBe(dirStart + (perCluster - 1) * ENTRY);
  });

  it("returns -1 for a slot past the end of a one-cluster chain", () => {
    expect(slotOffset(g, fat, owners, "/D", perCluster)).toBe(-1);
    expect(slotOffset(g, fat, owners, "/D", perCluster * 4 + 7)).toBe(-1);
    expect(slotOffset(g, fat, owners, "/nosuchdir", 0)).toBe(-1); // no owner, so no chain at all
  });

  it("the root directory is a flat run of slots, so any index resolves", () => {
    expect(slotOffset(g, fat, owners, "/", 5)).toBe(g.firstRootDirSector * g.bytesPerSector + 5 * ENTRY);
  });

  it("still finds a real entry's slots", () => {
    expect(findEntrySlots(vol, g, fat, owners, "/D/inner.txt")).toEqual({ start: dirStart + 2 * ENTRY, end: dirStart + 3 * ENTRY });
  });
});
