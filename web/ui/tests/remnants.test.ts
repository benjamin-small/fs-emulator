import { describe, expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";
import { findRemnants } from "../src/core/remnants";
import { scanZeroSectors } from "../src/core/zeros";
import { clusterByteRange } from "../src/core/attribution";

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
});
