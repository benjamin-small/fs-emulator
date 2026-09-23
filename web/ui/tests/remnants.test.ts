import { describe, expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";
import { findRemnants } from "../src/fs/fat16/remnants";
import { findEntrySlots } from "../src/fs/fat16/direntry";
import { scanZeroSectors } from "../src/core/zeros";
import { clusterByteRange } from "../src/fs/fat16/geometry";

describe("remnants", () => {
  it("marks deleted directory slots and non-zero free clusters", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createFile("/GONE.TXT", new TextEncoder().encode("still here"));
    vol.deleteFile("/GONE.TXT");
    const g = vol.geometry();
    const r = findRemnants(vol, g, vol.fatEntries(0), vol.clusterOwners(), scanZeroSectors(vol.image(), g.bytesPerSector));
    const root = g.firstRootDirSector * g.bytesPerSector;
    expect(r).toContainEqual({ start: root, end: root + 32 });
    expect(r).toContainEqual(clusterByteRange(g, 2));
    const fresh = Volume.formatFat16(undefined);
    expect(findRemnants(fresh, g, fresh.fatEntries(0), [], scanZeroSectors(fresh.image(), 512))).toEqual([]);
  });

  it("finds deleted slots inside a long-named subdirectory", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createDir("/My Folder");
    vol.createFile("/My Folder/GONE.TXT", new TextEncoder().encode("bye"));
    const g = vol.geometry();
    const slot = findEntrySlots(vol, g, vol.fatEntries(0), vol.clusterOwners(), "/My Folder/GONE.TXT")!;
    expect(slot).not.toBeNull();
    const clusterRange = clusterByteRange(g, vol.clusterOwners().find((o) => o.path === "/My Folder/GONE.TXT")!.firstCluster);
    vol.deleteFile("/My Folder/GONE.TXT");
    const r = findRemnants(vol, g, vol.fatEntries(0), vol.clusterOwners(), scanZeroSectors(vol.image(), g.bytesPerSector));
    expect(r).toContainEqual(slot);
    expect(r).toContainEqual(clusterRange);
  });

  it("terminates when a directory contains a cycle back to an ancestor", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createDir("/D");
    const g = vol.geometry();
    const owners = vol.clusterOwners();
    const dOwner = owners.find((o) => o.path === "/D")!;
    const rootSlot = findEntrySlots(vol, g, vol.fatEntries(0), owners, "/D")!;
    const image = new Uint8Array(vol.image());
    const rootEntryBytes = image.slice(rootSlot.start, rootSlot.end); // /D's own 32-byte directory entry (short, isDir, firstCluster = /D's cluster)
    const dClusterStart = clusterByteRange(g, dOwner.firstCluster).start;
    image.set(rootEntryBytes, dClusterStart + 2 * 32); // slot 2 inside /D's own cluster, after "." and ".."
    const corrupted = Volume.fromImage(image);
    const fat = corrupted.fatEntries(0);
    const corruptedOwners = corrupted.clusterOwners();
    const zeros = scanZeroSectors(corrupted.image(), g.bytesPerSector);
    expect(() => findRemnants(corrupted, g, fat, corruptedOwners, zeros)).not.toThrow();
  });
});
