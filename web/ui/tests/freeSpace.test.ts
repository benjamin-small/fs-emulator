import { describe, expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";
import { buildAttribution } from "../src/core/attribution";
import { freeSpaceLabel } from "../src/core/freeSpace";
import { adapterFor } from "../src/fs";

/** The label the ribbon printed before it asked the family: every unit no file owns, counted free. */
function ownerlessLabel(vol: Volume): string {
  const fs = adapterFor(vol);
  const { ownerByUnit } = buildAttribution(fs, vol.layout(), fs.owners);
  let free = 0;
  for (let u = fs.unit.first; u < fs.unit.first + fs.unitCount; u++) if (ownerByUnit[u] < 0) free++;
  return `${((free * fs.unitSize) / (1024 * 1024)).toFixed(1)} MB free`;
}

describe("freeSpaceLabel", () => {
  it("gives the FAT default disk the label the owner count gave it", () => {
    const vol = Volume.formatFat16(undefined);
    expect(freeSpaceLabel(adapterFor(vol))).toBe(ownerlessLabel(vol));
    expect(freeSpaceLabel(adapterFor(vol))).toBe("16.0 MB free");
  });

  it("still agrees with the owner count on FAT once a directory and a file hold clusters", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createDir("/DOCS");
    vol.createFile("/DOCS/N.TXT", new Uint8Array(300_000));
    expect(freeSpaceLabel(adapterFor(vol))).toBe(ownerlessLabel(vol));
  });

  it("counts FAT's free clusters as the ones no owner row claims", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createDir("/DOCS");
    vol.createFile("/DOCS/N.TXT", new Uint8Array(300_000));
    const fs = adapterFor(vol);
    expect(fs.owners.length).toBeGreaterThan(1);
    expect(fs.freeUnits()).toBe(fs.unitCount - fs.owners.length);
  });

  it("prints the free units in MiB to one decimal", () => {
    expect(freeSpaceLabel({ freeUnits: () => 15205, unitSize: 1024 })).toBe("14.8 MB free");
  });
});
