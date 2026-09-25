import type { ExtGeometry, ExtGroup } from "../../lib/wasm";
import { attrAtSector, type AttributionTable } from "../../core/attribution";
import { cellAt, cellRect, gridCols, gridRows } from "../../core/grid";
import type { Interval } from "../../core/intervals";
import { COLOR_JOURNAL } from "../../core/palette";
import { isPseudoOwner } from "../adapter";

/**
 * The block-group map's model, kept out of the component so the node tests can check it: the
 * text it draws, where each block's cell sits, and what colour and marks each cell gets. On ext
 * the disk's sector is the block (see extSpace), so a block number is also the attribution
 * table's sector number.
 */

/** A cell's side, the pitch from one cell to the next (a 1 px gap), one line of a band's header,
 *  the space between bands, and the scroll box's maximum height (the FAT map's). */
export const CELL = 4, GAP = 1, PITCH = CELL + GAP, HEADER = 16, BAND_GAP = 8, MAX_HEIGHT = 260;

/** The fill of a free block: the hairline colour rather than a palette index. */
export const FILL_FREE = 255;

/** One block group's band: its header row at `top`, then `rows` rows of cells from `cellsTop`. */
export interface Band { group: number; first: number; count: number; top: number; cellsTop: number; rows: number }
export interface MapLayout { cols: number; bands: Band[]; height: number }

/** `Block groups · 2 groups · 16,384 blocks` ("1 group" on a one-group disk). */
export function mapHeading(geo: ExtGeometry): string {
  const groups = geo.groups.length;
  return `Block groups · ${groups.toLocaleString()} ${groups === 1 ? "group" : "groups"} · ${geo.totalBlocks.toLocaleString()} blocks`;
}

/** A band's header: `group 0 · blocks 1–8192 · 7,082 free`, from the adapter's cached geometry. */
export function bandHeader(g: ExtGroup): string {
  return `group ${g.index} · blocks ${g.firstBlock}–${g.firstBlock + g.blockCount - 1} · ${g.freeBlocks.toLocaleString()} free`;
}

/** A band header broken at its ` · ` separators into as few lines as fit `width` (a part wider
 *  than the width still gets a line of its own); `measure` is the canvas's text width. The
 *  sidebar is narrower than a one-line header in the map's font. */
export function wrapHeader(text: string, width: number, measure: (s: string) => number): string[] {
  const lines: string[] = [];
  let line = "";
  for (const part of text.split(" · ")) {
    const joined = line ? `${line} · ${part}` : part;
    if (line && measure(joined) > width) { lines.push(line); line = part; } else line = joined;
  }
  if (line) lines.push(line);
  return lines;
}

/** One band per group, stacked, each group's blocks wrapped to `width` under a header of
 *  `headerLines` lines. Block 0 (the boot block on a 1 KiB-block disk) lies outside every group
 *  and has no cell. */
export function layoutBands(geo: ExtGeometry, width: number, headerLines = 1): MapLayout {
  const cols = gridCols(width, CELL, GAP);
  const header = headerLines * HEADER;
  const bands: Band[] = [];
  let top = 0;
  for (const g of geo.groups) {
    const rows = gridRows(g.blockCount, cols);
    bands.push({ group: g.index, first: g.firstBlock, count: g.blockCount, top, cellsTop: top + header, rows });
    top += header + rows * PITCH + BAND_GAP;
  }
  return { cols, bands, height: Math.max(1, top - BAND_GAP) };
}

/** The top-left corner of block `b`'s cell, or null when no band holds it. */
export function cellOf(m: MapLayout, b: number): { x: number; y: number } | null {
  const band = m.bands.find((x) => b >= x.first && b < x.first + x.count);
  if (!band) return null;
  const c = cellRect(b - band.first, m.cols, CELL, GAP);
  return { x: c.x, y: band.cellsTop + c.y };
}

/** The block whose cell (or the gap after it) is under `(x, y)`; null over a header row, past a
 *  band's last block, or outside the map. */
export function blockAt(m: MapLayout, x: number, y: number): number | null {
  for (const band of m.bands) {
    if (y < band.cellsTop || y >= band.cellsTop + band.rows * PITCH) continue;
    const i = cellAt(x, y - band.cellsTop, m.cols, band.count, CELL, GAP);
    return i === null ? null : band.first + i;
  }
  return null;
}

/** The scroll offset to actually paint and hit-test at, clamped to `[0, mapHeight -
 *  viewportHeight]` (or 0 when the map is shorter than the viewport). A raw scroll position can
 *  point past the map right after a format that shrinks the disk (a 256 MB map scrolled near its
 *  bottom, reformatted to 16 MB) and before the next real scroll event corrects the state; every
 *  caller that converts between canvas coordinates and map coordinates uses this, not the raw
 *  scroll state, so a click always targets what the map is actually showing. */
export function clampScroll(scrollTop: number, mapHeight: number, viewportHeight: number): number {
  return Math.max(0, Math.min(scrollTop, Math.max(0, mapHeight - viewportHeight)));
}

/** The row indices (0-based within `band`) whose cells intersect the vertical range
 *  `[top, bottom)`; null when the band's cell rows do not intersect it at all. A large disk
 *  (256 MB: 32 groups, tens of thousands of CSS px tall) is painted one screenful at a time, so
 *  the canvas never grows past the browser's canvas-size limit; this is the row range that
 *  screenful covers for one band. */
export function visibleRows(band: Band, top: number, bottom: number): { first: number; last: number } | null {
  const rowsTop = band.cellsTop;
  const rowsBottom = band.cellsTop + band.rows * PITCH;
  if (bottom <= rowsTop || top >= rowsBottom) return null;
  const first = Math.max(0, Math.floor((top - band.cellsTop) / PITCH));
  const last = Math.min(band.rows - 1, Math.ceil((bottom - band.cellsTop) / PITCH) - 1);
  return { first, last };
}

/** Each block's fill, indexed by block: metadata by region kind (the table's `colorForRegion`),
 *  every block in `journalBlocks` `COLOR_JOURNAL`, an owned data or directory block its owner's
 *  colour, and a free block `FILL_FREE`. */
export function blockFills(t: AttributionTable, geo: ExtGeometry, journalBlocks: readonly number[]): Uint8Array {
  const fills = new Uint8Array(geo.totalBlocks).fill(FILL_FREE);
  for (let b = 0; b < geo.totalBlocks; b++) {
    const a = attrAtSector(t, b);
    if (!a.free) fills[b] = a.colorIndex;
  }
  for (const b of journalBlocks) if (b >= 0 && b < geo.totalBlocks) fills[b] = COLOR_JOURNAL;
  return fills;
}

/** The blocks of every owner row with `role: "indirect"` (the map's 2 px dot), in owner order. */
export function indirectBlocks(t: AttributionTable): number[] {
  return t.owners.filter((o) => o.role === "indirect").map((o) => o.unit);
}

/** Every block the byte ranges touch, ascending, each once, clipped to the disk. */
export function blocksTouched(ranges: readonly Interval[], blockSize: number, totalBlocks: number): number[] {
  const out = new Set<number>();
  for (const r of ranges) {
    if (r.end <= r.start) continue;
    const last = Math.min(totalBlocks - 1, Math.floor((r.end - 1) / blockSize));
    for (let b = Math.floor(r.start / blockSize); b <= last; b++) out.add(b);
  }
  return [...out].sort((a, b) => a - b);
}

/** The hover caption: `block ${b} · ${regionName}`, then ` · ${ownerPath}` when owned or
 *  ` · free` for a free data block. */
export function blockCaption(t: AttributionTable, b: number): string {
  const a = attrAtSector(t, b);
  const tail = a.ownerPath !== undefined ? ` · ${a.ownerPath}` : a.free ? " · free" : "";
  return `block ${b} · ${a.regionName}${tail}`;
}

/** The path a click on block `b` selects: its owner, except a pseudo-owner such as the
 *  journal's `<journal>` rows, which names no file in the tree (a click there only jumps). */
export function clickPath(t: AttributionTable, b: number): string | null {
  const path = attrAtSector(t, b).ownerPath;
  return path === undefined || isPseudoOwner(path) ? null : path;
}
