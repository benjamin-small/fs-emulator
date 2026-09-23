import { describe, expect, it } from "vitest";
import type { Volume } from "../src/lib/wasm";
import { adapterFor, FAMILIES } from "../src/fs";
import { all } from "../src/scenarios";
import { BIGGER, BIGGER_BYTES, biggerText, HELLO, HELLO_TEXT, scenario } from "../src/scenarios/fundamentals";

// "The fundamentals" quotes the default disk's numbers in its copy (sector 97, 8,167
// clusters, 4,796 bytes, ...). These tests pin every quoted number to the volume the
// scenario actually runs on, so a change to the default format cannot leave the lesson
// teaching stale arithmetic.

const n = (v: number) => v.toLocaleString("en-US");
const stepText = (title: string) => {
  const s = scenario.steps.find((st) => st.title === title);
  if (!s) throw new Error(`no step titled "${title}"`);
  return s.text;
};

/** Run the scenario the way the runner does: a format of its family, then each action in
 *  order with the adapter refreshed after it, stopping after the step with the given title.
 *  Returns the volume in that state. */
function runThrough(title: string): Volume {
  const vol = FAMILIES[scenario.family].format();
  const fs = adapterFor(vol);
  for (const step of scenario.steps) {
    if (step.action) {
      step.action(vol, fs);
      fs.refresh();
    }
    if (step.title === title) break;
  }
  return vol;
}

/** A step's focus as the runner resolves it: the function form gets an adapter bound to the
 *  volume in its current state. */
const focusOf = (title: string, vol: Volume) => {
  const s = scenario.steps.find((st) => st.title === title)!;
  return typeof s.focus === "function" ? s.focus(adapterFor(vol)) : s.focus;
};

describe("the fundamentals scenario", () => {
  it("is the first scenario, so the picker defaults to it", () => {
    expect(all[0].id).toBe("fundamentals");
  });

  it("quotes the default geometry correctly", () => {
    const g = FAMILIES[scenario.family].format().geometry();
    expect(g).toMatchObject({
      bytesPerSector: 512, sectorsPerCluster: 4, reservedSectors: 1, fatCount: 2, sectorsPerFat: 32,
      rootEntries: 512, rootDirSectors: 32, firstRootDirSector: 65, firstDataSector: 97, totalSectors: 32768, clusterCount: 8167,
    });
    const bpb = stepText("Sector 0 describes the rest");
    for (const s of [`${g.bytesPerSector} bytes per sector`, `${g.sectorsPerCluster} sectors per cluster`, `${g.reservedSectors} reserved sector`, `${g.fatCount} FATs`, `${g.rootEntries} root entries`, `${n(g.totalSectors)} sectors in total`, `${g.sectorsPerFat} sectors per FAT`]) {
      expect(bpb).toContain(s);
    }
    const regions = stepText("Where each region starts");
    const fat1 = g.reservedSectors + g.sectorsPerFat;
    for (const s of [`FAT 0 begins at sector ${g.reservedSectors}`, `FAT 1 begins at sector ${fat1}`, `root directory at ${g.firstRootDirSector}`, `${n(g.rootEntries * 32)} bytes, ${g.rootDirSectors} sectors`, `data area begins at sector ${g.firstDataSector}`]) {
      expect(regions).toContain(s);
    }
    expect(stepText("One disk, five regions")).toContain(`${n(g.totalSectors)} sectors of ${g.bytesPerSector} bytes`);
    expect(stepText("Free means zero in the table")).toContain(`${n(g.clusterCount)} clusters exist`);
  });

  it("puts HELLO.TXT in cluster 2 with an end-of-chain entry", () => {
    const vol = runThrough("One disk, five regions");
    expect(vol.stat(HELLO).size).toBe(HELLO_TEXT.length);
    expect(vol.clusterOwners().filter((o) => o.path === HELLO).map((o) => o.cluster)).toEqual([2]);
    expect(vol.fatEntries(0)[2]).toEqual({ kind: "endOfChain" });
    expect(vol.fatEntries(1)[2]).toEqual({ kind: "endOfChain" });
    const g = vol.geometry();
    expect(stepText("Following the links")).toContain(`sector ${g.firstDataSector} + (2 − 2) × ${g.sectorsPerCluster} = sector ${g.firstDataSector}`);
    expect(stepText("Following the links")).toContain(`Only ${HELLO_TEXT.length} of the cluster's ${n(g.bytesPerSector * g.sectorsPerCluster)} bytes`);
  });

  it("chains BIGGER.TXT through clusters 3, 4, 5 with the quoted slack", () => {
    const vol = runThrough("A larger file chains clusters");
    expect(biggerText(BIGGER_BYTES).length).toBe(BIGGER_BYTES);
    expect(vol.stat(BIGGER).size).toBe(BIGGER_BYTES);
    expect(vol.clusterOwners().filter((o) => o.path === BIGGER).map((o) => o.cluster).sort((a, b) => a - b)).toEqual([3, 4, 5]);
    const fat = vol.fatEntries(0);
    expect(fat[3]).toEqual({ kind: "next", cluster: 4 });
    expect(fat[4]).toEqual({ kind: "next", cluster: 5 });
    expect(fat[5]).toEqual({ kind: "endOfChain" });
    expect(fat[6]).toEqual({ kind: "free" });
    const g = vol.geometry();
    const perCluster = g.bytesPerSector * g.sectorsPerCluster;
    const clusters = Math.ceil(BIGGER_BYTES / perCluster);
    expect(clusters).toBe(3);
    expect(stepText("A larger file chains clusters")).toContain(`${n(BIGGER_BYTES)} bytes`);
    expect(stepText("Size versus space")).toContain(`${clusters} × ${n(perCluster)} = ${n(clusters * perCluster)}`);
    expect(stepText("Size versus space")).toContain(`The ${n(clusters * perCluster - BIGGER_BYTES)} bytes at the end of cluster 5`);
    const used = fat.filter((e) => e.kind !== "free").length - 2; // entries 0 and 1 are reserved
    expect(used).toBe(4);
    expect(stepText("Free means zero in the table")).toContain(`${used} are in use, so ${n(g.clusterCount - used)} entries read 0000`);
  });

  it("points each table step at the FAT entry it talks about", () => {
    const vol = runThrough("A larger file chains clusters");
    const g = vol.geometry();
    const fat0 = g.reservedSectors * g.bytesPerSector;
    const fat1 = (g.reservedSectors + g.sectorsPerFat) * g.bytesPerSector;
    expect(focusOf("The FAT: one entry per cluster", vol)).toEqual({ offset: fat0 + 2 * 2 });
    expect(focusOf("FAT 1 is a mirror", vol)).toEqual({ offset: fat1 + 2 * 2 });
    expect(focusOf("The chain in the table", vol)).toEqual({ path: BIGGER, offset: fat0 + 3 * 2 });
    expect(focusOf("Free means zero in the table", vol)).toEqual({ path: null, offset: fat0 + 6 * 2 });
    // HELLO.TXT took the first root slot, so its entry starts where the root directory does.
    expect(focusOf("The root directory names the file", vol)).toEqual({ path: HELLO, offset: g.firstRootDirSector * g.bytesPerSector });
    expect(focusOf("Sector 0 describes the rest", vol)).toEqual({ offset: 11 });
  });

  it("points the region-start step at the root directory's sector on the default disk", () => {
    const vol = runThrough("One disk, five regions");
    expect(focusOf("Where each region starts", vol)).toEqual({ sector: 65 });
  });
});
