/**
 * The cell grids the maps draw on a canvas (the FAT map's clusters, each band of the ext
 * block-group map, the Journal panel's ring): `count` square cells of side `cell`, each
 * followed by a `gap`, filled left to right and wrapped to a width. The maths is pure so the
 * node tests can check it; the drawing helpers are the marks every map puts on a cell.
 */

/** Size `canvas` to `width` × `height` CSS pixels at the device's pixel ratio, so what a map
 *  draws stays sharp, and hand back its context scaled to CSS pixels and cleared (null when the
 *  canvas has no 2D context). Every canvas map starts its paint with it. */
export function prepareCanvas(canvas: HTMLCanvasElement, width: number, height: number): CanvasRenderingContext2D | null {
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(width * dpr);
  canvas.height = Math.round(height * dpr);
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, width, height);
  return ctx;
}

/** How many cells fit across `width`: never fewer than one, so a map lays out before its
 *  first width measurement. */
export function gridCols(width: number, cell: number, gap: number): number {
  return Math.max(1, Math.floor(width / (cell + gap)));
}

/** How many rows `count` cells take in `cols` columns: never fewer than one. */
export function gridRows(count: number, cols: number): number {
  return Math.max(1, Math.ceil(count / cols));
}

/** The top-left corner of cell `index`. */
export function cellRect(index: number, cols: number, cell: number, gap: number): { x: number; y: number } {
  const pitch = cell + gap;
  return { x: (index % cols) * pitch, y: Math.floor(index / cols) * pitch };
}

/** The index of the cell under `(px, py)`, the gap after a cell counting as the cell; null
 *  left of or above the grid, right of its last column, or past its last cell. */
export function cellAt(px: number, py: number, cols: number, count: number, cell: number, gap: number): number | null {
  const pitch = cell + gap;
  const col = Math.floor(px / pitch), row = Math.floor(py / pitch);
  if (col < 0 || col >= cols || row < 0) return null;
  const i = row * cols + col;
  return i < count ? i : null;
}

/** A 1 px outline just inside a cell (the diff, a chain's cells). */
export function outlineCell(ctx: CanvasRenderingContext2D, x: number, y: number, cell: number, color: string): void {
  ctx.strokeStyle = color;
  ctx.lineWidth = 1;
  ctx.strokeRect(x + 0.5, y + 0.5, cell - 1, cell - 1);
}

/** A 2 px dot in the middle of a cell (FAT's end-of-chain mark, ext's pointer blocks). */
export function dotCell(ctx: CanvasRenderingContext2D, x: number, y: number, cell: number, color: string): void {
  const inset = (cell - 2) / 2;
  ctx.fillStyle = color;
  ctx.fillRect(x + inset, y + inset, 2, 2);
}

/** A dashed outline: the cell holding the byte the mouse is over in the hex dump. */
export function dashCell(ctx: CanvasRenderingContext2D, x: number, y: number, cell: number, color: string): void {
  ctx.save();
  ctx.setLineDash([1, 1]);
  outlineCell(ctx, x, y, cell, color);
  ctx.restore();
}

/** The selected file's chain: each cell outlined, then the cells' centres joined in order by a
 *  line `joinWidth` px wide. */
export function drawChain(ctx: CanvasRenderingContext2D, cells: readonly { x: number; y: number }[], cell: number, color: string, joinWidth: number): void {
  for (const c of cells) outlineCell(ctx, c.x, c.y, cell, color);
  ctx.lineWidth = joinWidth;
  ctx.beginPath();
  cells.forEach((c, i) => {
    const cx = c.x + cell / 2, cy = c.y + cell / 2;
    if (i === 0) ctx.moveTo(cx, cy); else ctx.lineTo(cx, cy);
  });
  ctx.stroke();
}
