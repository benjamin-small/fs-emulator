import { describe, expect, it } from "vitest";
import { metaLabel } from "../src/core/legend";
import type { Region } from "../src/lib/wasm";

const region = (kind: Region["kind"], name: string): Region => ({ kind, name, sectors: { start: 0, end: 1 } });

describe("metaLabel", () => {
  it("shortens the boot region's name to 'boot' for both families' boot areas", () => {
    expect(metaLabel(region("boot", "reserved (boot sector)"))).toBe("boot");
    expect(metaLabel(region("boot", "boot block"))).toBe("boot");
  });

  it("keeps the ext superblock's own names, which share the boot kind", () => {
    expect(metaLabel(region("boot", "superblock"))).toBe("superblock");
    expect(metaLabel(region("boot", "backup superblock (group 1)"))).toBe("backup superblock (group 1)");
  });

  it("calls the fixed root directory 'root' and leaves every other region its name", () => {
    expect(metaLabel(region("directory", "root directory"))).toBe("root");
    expect(metaLabel(region("allocationTable", "FAT 0"))).toBe("FAT 0");
    expect(metaLabel(region("journal", "journal"))).toBe("journal");
  });
});
