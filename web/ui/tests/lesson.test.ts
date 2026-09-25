import { describe, expect, it } from "vitest";
import { describeFocus } from "../src/core/lesson";
import { layout, space } from "./fixtures/geometry";
import { layout as extLayout, space as extSpace } from "./fixtures/extGeometry";
import type { UnitSpace } from "../src/fs/adapter";

/** The panel's `regionNameAt`, which reads the attribution table; the layout fixture is enough here. */
const regionNameAt = (sector: number): string =>
  layout.find((r) => sector >= r.sectors.start && sector < r.sectors.end)?.name ?? "unknown";

const describe_ = (focus: Parameters<typeof describeFocus>[0]) => describeFocus(focus, space, regionNameAt);

describe("describeFocus", () => {
  it("names the selected file", () => {
    expect(describe_({ path: "/DOCS/N.TXT" })).toBe("Files: /DOCS/N.TXT, its entry, chain, and clusters");
  });

  it("names a cluster with the byte offset it starts at", () => {
    // Cluster 2 is the first data cluster: sector 97 x 512 bytes. The noun comes from the space.
    expect(describe_({ unit: 2 })).toBe("Cluster 2 in the data region (offset 0xc200)");
  });

  it("names a sector with its region", () => {
    expect(describe_({ sector: 65 })).toBe("Sector 65, root directory");
  });

  it("names an offset with the region of the sector it lands in", () => {
    expect(describe_({ offset: 0x8200 })).toBe("Offset 0x8200 in root directory");
  });

  it("names one place only, the one the dump actually goes to", () => {
    // `applyFocus` jumps to offset, else sector, else unit; the card must say the same.
    expect(describe_({ offset: 0x200, sector: 5, unit: 9 })).toBe("Offset 0x200 in FAT 0");
    expect(describe_({ sector: 5, unit: 9 })).toBe("Sector 5, FAT 0");
  });

  it("mentions the remnant hatching and the strings overlay", () => {
    expect(describe_({ showRemnants: true })).toBe("Deleted entries are shown hatched");
    expect(describe_({ strings: true })).toBe("Printable strings are highlighted");
  });

  it("joins the file, the one place, and the overlays, in a fixed order, capitalising once", () => {
    expect(describe_({ strings: true, sector: 1, path: "/A.TXT", showRemnants: true })).toBe(
      "Files: /A.TXT, its entry, chain, and clusters; sector 1, FAT 0; deleted entries are shown hatched; printable strings are highlighted",
    );
    expect(describe_({ path: "/A.TXT", unit: 2, strings: true })).toBe(
      "Files: /A.TXT, its entry, chain, and clusters; cluster 2 in the data region (offset 0xc200); printable strings are highlighted",
    );
  });

  it("takes the file clause and the sector noun from the space (a family whose unit is its block)", () => {
    const blocks: UnitSpace = {
      ...space,
      unit: { singular: "block", plural: "blocks", letter: "b", first: 1, fileParts: "its inode, block map, and blocks" },
      sector: { singular: "block", plural: "blocks", letter: "b" },
    };
    const ext = (focus: Parameters<typeof describeFocus>[0]) => describeFocus(focus, blocks, regionNameAt);
    expect(ext({ path: "/a" })).toBe("Files: /a, its inode, block map, and blocks");
    expect(ext({ sector: 65 })).toBe("Block 65, root directory");
    expect(ext({ path: "/a", sector: 1 })).toBe("Files: /a, its inode, block map, and blocks; block 1, FAT 0");
  });

  it("speaks ext's vocabulary on the default ext3 disk: blocks, the inode clause, ext's region names", () => {
    const extRegionAt = (sector: number): string =>
      extLayout.find((r) => sector >= r.sectors.start && sector < r.sectors.end)?.name ?? "unknown";
    const ext = (focus: Parameters<typeof describeFocus>[0]) => describeFocus(focus, extSpace, extRegionAt);
    expect(ext({ path: "/hello.txt" })).toBe("Files: /hello.txt, its inode, block map, and blocks");
    expect(ext({ sector: 69 })).toBe("Block 69, data (group 0)");
    expect(ext({ sector: 82 })).toBe("Block 82, journal");
    expect(ext({ unit: 1112 })).toBe("Block 1112 in the data region (offset 0x116000)");
    expect(ext({ offset: 0x1a00 })).toBe("Offset 0x1a00 in inode table (group 0)");
    expect(ext({ path: "/hello.txt", offset: 0x1a00 })).toBe("Files: /hello.txt, its inode, block map, and blocks; offset 0x1a00 in inode table (group 0)");
  });

  it("is null when there is nothing to point at", () => {
    expect(describe_(null)).toBeNull();
    expect(describe_(undefined)).toBeNull();
    expect(describe_({})).toBeNull();
    expect(describe_({ path: null })).toBeNull(); // clears the selection; nothing to look at
    expect(describe_({ showRemnants: false, strings: false })).toBeNull();
  });
});
