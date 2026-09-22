import { describe, expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";
import { applyChanges, changedSectors } from "../src/core/patch";
import { attrAtOffset, buildAttribution, clusterByteRange } from "../src/core/attribution";
import { buildTree } from "../src/core/tree";

describe("package integration", () => {
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
});
