import { describe, expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";
import { buildTree } from "../src/core/tree";
import { findEntrySlots } from "../src/core/direntry";
import { findRemnants } from "../src/core/remnants";
import { scanZeroSectors } from "../src/core/zeros";

function corruptFixture() {
  const vol = Volume.formatFat16(undefined);
  vol.createFile("/A.TXT", new Uint8Array(5));
  vol.createDir("/D");
  vol.createFile("/D/INNER.TXT", new Uint8Array(3));

  const owners = vol.clusterOwners();
  const fat = vol.fatEntries(0);
  const geo = vol.geometry();
  const zeros = scanZeroSectors(vol.image(), geo.bytesPerSector);
  const saved = vol.readRaw(0, 512);

  expect(vol.corruption()).toBeNull();
  vol.writeRaw(0, new Uint8Array(512));
  expect(vol.corruption()).toContain("boot sector no longer parses");

  return { vol, owners, fat, geo, zeros, saved };
}

describe("a corrupt volume", () => {
  it("buildTree returns an empty root instead of throwing", () => {
    const { vol, owners } = corruptFixture();
    const tree = buildTree(vol, owners);
    expect(tree).toMatchObject({ name: "/", path: "/", isDir: true, children: [] });
  });

  it("findEntrySlots returns null (its not-found shape) instead of throwing", () => {
    const { vol, geo, fat, owners } = corruptFixture();
    expect(findEntrySlots(vol, geo, fat, owners, "/A.TXT")).toBeNull();
  });

  it("findRemnants returns an empty array instead of throwing", () => {
    const { vol, geo, fat, owners, zeros } = corruptFixture();
    expect(findRemnants(vol, geo, fat, owners, zeros)).toEqual([]);
  });

  it("writing the saved boot sector back clears corruption() and the derivations see the volume again", () => {
    const { vol, saved } = corruptFixture();
    vol.writeRaw(0, saved);
    expect(vol.corruption()).toBeNull();
    const tree = buildTree(vol, vol.clusterOwners());
    expect(tree.children.map((c) => c.name).sort()).toEqual(["A.TXT", "D"]);
  });

  it("a non-corrupt error still propagates: findEntrySlots is unaffected when the parent doesn't exist", () => {
    const vol = Volume.formatFat16(undefined);
    const g = vol.geometry(), fat = vol.fatEntries(0), owners = vol.clusterOwners();
    // No corruption here; the parent directory simply doesn't exist. This must behave
    // exactly as it did before this feature: return null (the helper's own pre-existing
    // "not found" handling), not the new CorruptImage absorption.
    expect(findEntrySlots(vol, g, fat, owners, "/NOSUCHDIR/A.TXT")).toBeNull();
  });

  it("isCorrupt narrowly matches only a CorruptImage code, so any other error still propagates", async () => {
    const { isCorrupt } = await import("../src/core/corrupt");
    expect(isCorrupt({ code: "CorruptImage", message: "boot sector no longer parses" })).toBe(true);
    expect(isCorrupt({ code: "NotFound", message: "nope" })).toBe(false);
    expect(isCorrupt(new Error("plain error"))).toBe(false);
    expect(isCorrupt("CorruptImage")).toBe(false);
    expect(isCorrupt(null)).toBe(false);
    expect(isCorrupt(undefined)).toBe(false);
  });
});
