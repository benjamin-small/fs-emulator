import { describe, expect, it } from "vitest";
import { buildAttribution } from "../../src/core/attribution";
import { COLOR_BOOT, COLOR_DIR, COLOR_JOURNAL, COLOR_TABLE, colorIndexForPath } from "../../src/core/palette";
import { asExt, ext } from "../../src/fs/ext";
import {
  BAND_GAP, CELL, FILL_FREE, HEADER, MAX_HEIGHT, PITCH,
  bandHeader, blockAt, blockCaption, blockFills, blocksTouched, cellOf, clampScroll, clickPath, indirectBlocks, layoutBands, mapHeading, visibleRows, wrapHeader,
} from "../../src/fs/ext/blockMap";

const BLOCK = 1024;
const n = (x: number) => x.toLocaleString();

/** The default ext3 disk with /hello.txt (block 1111) and /bigger.txt (13 data blocks
 *  1112..1124 and its single-indirect block 1125), and the attribution table the store builds. */
function fixture() {
  const vol = ext.format();
  vol.createFile("/hello.txt", new TextEncoder().encode("Hello, ext3!"));
  vol.createFile("/bigger.txt", new Uint8Array(13 * BLOCK));
  const fs = asExt(ext.bind(vol));
  return { vol, fs, t: buildAttribution(fs, vol.layout(), fs.owners) };
}

describe("the block-group map's text", () => {
  it("heads the panel with the group and block counts", () => {
    const { fs } = fixture();
    expect(mapHeading(fs.geo)).toBe(`Block groups · 2 groups · ${n(16384)} blocks`);
    expect(mapHeading(asExt(ext.bind(ext.format({ totalBlocks: 4096 }))).geo)).toBe(`Block groups · 1 group · ${n(4096)} blocks`);
  });

  it("heads each band with its group, its block range, and its cached free count", () => {
    const fresh = asExt(ext.bind(ext.format()));
    expect(fresh.geo.groups.map(bandHeader)).toEqual([`group 0 · blocks 1–8192 · ${n(7082)} free`, `group 1 · blocks 8193–16383 · ${n(8123)} free`]);
    const { fs } = fixture();
    expect(bandHeader(fs.geo.groups[0])).toBe(`group 0 · blocks 1–8192 · ${n(7082 - 15)} free`); // 1 + 13 data blocks and 1 pointer block
  });

  it("wraps a band header at its separators only when it does not fit the width", () => {
    const header = `group 0 · blocks 1–8192 · 7,082 free`;
    const measure = (s: string) => s.length * 6; // a 6 px monospace advance
    expect(wrapHeader(header, 400, measure)).toEqual([header]);
    expect(wrapHeader(header, 150, measure)).toEqual(["group 0 · blocks 1–8192", "7,082 free"]);
    expect(wrapHeader(header, 50, measure)).toEqual(["group 0", "blocks 1–8192", "7,082 free"]); // a part wider than the width still gets a line
  });

  it("captions a block with its region, then its owner or free", () => {
    const { t } = fixture();
    expect(blockCaption(t, 0)).toBe("block 0 · boot block");
    expect(blockCaption(t, 3)).toBe("block 3 · block bitmap (group 0)");
    expect(blockCaption(t, 90)).toBe("block 90 · journal");
    expect(blockCaption(t, 69)).toBe("block 69 · data (group 0) · /");
    expect(blockCaption(t, 1107)).toBe("block 1107 · data (group 0) · <journal>");
    expect(blockCaption(t, 1111)).toBe("block 1111 · data (group 0) · /hello.txt");
    expect(blockCaption(t, 1125)).toBe("block 1125 · data (group 0) · /bigger.txt");
    expect(blockCaption(t, 1126)).toBe("block 1126 · data (group 0) · free");
    expect(blockCaption(t, 8195)).toBe("block 8195 · block bitmap (group 1)");
  });

  it("selects a clicked block's owner, but never the journal, which is not a path", () => {
    const { t } = fixture();
    expect(clickPath(t, 1111)).toBe("/hello.txt");
    expect(clickPath(t, 1125)).toBe("/bigger.txt");
    expect(clickPath(t, 69)).toBe("/");
    expect(clickPath(t, 1107)).toBeNull();
    expect(clickPath(t, 1126)).toBeNull();
    expect(clickPath(t, 3)).toBeNull();
    expect(clickPath(t, 90)).toBeNull();
  });
});

describe("the block-group map's geometry", () => {
  it("uses 4 px cells with a 1 px gap and the FAT map's 260 px scroll box", () => {
    expect([CELL, PITCH, MAX_HEIGHT]).toEqual([4, 5, 260]);
  });

  it("stacks one band per group: a header row, then the group's blocks wrapped to the width", () => {
    const { fs } = fixture();
    const m = layoutBands(fs.geo, 300);
    expect(m.cols).toBe(60);
    const rows = Math.ceil(8192 / 60); // 137, and the same for group 1's 8191 blocks
    expect(m.bands).toEqual([
      { group: 0, first: 1, count: 8192, top: 0, cellsTop: HEADER, rows },
      { group: 1, first: 8193, count: 8191, top: HEADER + rows * PITCH + BAND_GAP, cellsTop: 2 * HEADER + rows * PITCH + BAND_GAP, rows },
    ]);
    expect(m.height).toBe(2 * (HEADER + rows * PITCH) + BAND_GAP);
    expect(layoutBands(fs.geo, 0).cols).toBe(1); // before the first measurement
    // Headers wrapped onto two lines make every band's header row twice as tall.
    const two = layoutBands(fs.geo, 300, 2);
    expect(two.bands.map((b) => [b.top, b.cellsTop])).toEqual([[0, 2 * HEADER], [2 * HEADER + rows * PITCH + BAND_GAP, 4 * HEADER + rows * PITCH + BAND_GAP]]);
    expect(two.height).toBe(2 * (2 * HEADER + rows * PITCH) + BAND_GAP);
  });

  it("places a block's cell and finds the block under a point, and nothing between bands", () => {
    const { fs } = fixture();
    const m = layoutBands(fs.geo, 300);
    const band1 = m.bands[1].cellsTop;
    expect(cellOf(m, 1)).toEqual({ x: 0, y: HEADER });
    expect(cellOf(m, 2)).toEqual({ x: PITCH, y: HEADER });
    expect(cellOf(m, 61)).toEqual({ x: 0, y: HEADER + PITCH });
    expect(cellOf(m, 8193)).toEqual({ x: 0, y: band1 });
    expect(cellOf(m, 0)).toBeNull(); // the boot block is outside every group
    expect(cellOf(m, 16384)).toBeNull();
    for (const b of [1, 2, 60, 61, 1111, 8192, 8193, 16383]) {
      const c = cellOf(m, b)!;
      expect(blockAt(m, c.x, c.y), `block ${b}`).toBe(b);
      expect(blockAt(m, c.x + CELL, c.y + CELL), `block ${b}'s gap`).toBe(b); // the gap after a cell is the cell's
    }
    expect(blockAt(m, 0, HEADER - 1)).toBeNull();                 // group 0's header row
    expect(blockAt(m, 0, band1 - 1)).toBeNull();                  // group 1's header row
    const last0 = cellOf(m, 8192)!;
    expect(blockAt(m, last0.x + PITCH, last0.y)).toBeNull();      // past group 0's last block
    expect(blockAt(m, 300, HEADER)).toBeNull();                   // past the last column
    expect(blockAt(m, -1, HEADER)).toBeNull();
    expect(blockAt(m, 0, m.height)).toBeNull();
  });

  it("finds a band's visible rows within a vertical range, or null when the band misses it entirely", () => {
    // A synthetic band: header at y=[0,16), 10 rows of cells at y=[16,66) in steps of PITCH (5).
    const band = { group: 0, first: 1, count: 600, top: 0, cellsTop: HEADER, rows: 10 };
    expect(visibleRows(band, 100, 200)).toBeNull();               // fully above the range (range starts past the band)
    expect(visibleRows(band, 0, 10)).toBeNull();                  // fully below the range (range ends before the band's rows start)
    expect(visibleRows(band, 20, 30)).toEqual({ first: 0, last: 2 });   // partially visible at the top edge
    expect(visibleRows(band, 50, 80)).toEqual({ first: 6, last: 9 });   // partially visible at the bottom edge
    expect(visibleRows(band, 0, 100)).toEqual({ first: 0, last: 9 });   // the whole band fits inside the range
    expect(visibleRows(band, 21, 26)).toEqual({ first: 1, last: 1 });   // a range that exactly spans one row
  });

  it("clamps a scroll position to what the map can actually show", () => {
    expect(clampScroll(-50, 1000, 260)).toBe(0);           // never negative
    expect(clampScroll(300, 1000, 260)).toBe(300);         // within range: unchanged
    expect(clampScroll(900, 1000, 260)).toBe(740);         // past the end: mapHeight - viewportHeight
    expect(clampScroll(5000, 1000, 260)).toBe(740);        // far past the end: still clamped to the same max
    expect(clampScroll(50, 200, 260)).toBe(0);              // the map is shorter than the viewport: always 0
  });
});

describe("the block-group map's colours and marks", () => {
  it("colours metadata by region kind, every journal block amber, owned blocks by owner, free blocks as the hairline", () => {
    const { fs, t } = fixture();
    const fills = blockFills(t, fs.geo, fs.journalBlocks);
    expect(fills.length).toBe(16384);
    expect([fills[0], fills[1], fills[3], fills[5], fills[68]]).toEqual([COLOR_BOOT, COLOR_BOOT, COLOR_TABLE, COLOR_BOOT, COLOR_BOOT]);
    expect([fills[69], fills[70]]).toEqual([COLOR_DIR, COLOR_DIR]);        // the root and lost+found
    expect([fills[82], fills[1105], fills[1106], fills[1110]]).toEqual([COLOR_JOURNAL, COLOR_JOURNAL, COLOR_JOURNAL, COLOR_JOURNAL]);
    expect(fills[1111]).toBe(colorIndexForPath("/hello.txt"));
    expect([fills[1112], fills[1125]]).toEqual([colorIndexForPath("/bigger.txt"), colorIndexForPath("/bigger.txt")]);
    expect([fills[1126], fills[16383]]).toEqual([FILL_FREE, FILL_FREE]);
    expect([fills[8193], fills[8195]]).toEqual([COLOR_BOOT, COLOR_TABLE]);
    // Whatever attribution says, a block the adapter lists as the journal's is amber.
    expect(blockFills(t, fs.geo, [1126])[1126]).toBe(COLOR_JOURNAL);
  });

  it("marks every indirect row: the journal's pointer blocks and a file's", () => {
    const { t } = fixture();
    expect(indirectBlocks(t)).toEqual([1106, 1107, 1108, 1109, 1110, 1125]);
  });

  it("lists the blocks a set of byte ranges touches, once each, within the disk", () => {
    expect(blocksTouched([{ start: 5 * BLOCK + 10, end: 7 * BLOCK }], BLOCK, 16384)).toEqual([5, 6]);
    expect(blocksTouched([{ start: 0, end: 1 }, { start: 100, end: 2 * BLOCK + 1 }], BLOCK, 16384)).toEqual([0, 1, 2]);
    expect(blocksTouched([{ start: 16383 * BLOCK, end: 16385 * BLOCK }], BLOCK, 16384)).toEqual([16383]);
    expect(blocksTouched([{ start: 8, end: 8 }], BLOCK, 16384)).toEqual([]);
    expect(blocksTouched([], BLOCK, 16384)).toEqual([]);
  });
});
