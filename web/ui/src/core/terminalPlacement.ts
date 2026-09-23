import { parseStoredPx } from "./terminalHeight";

/** Where the terminal lives: a column down the right side of a wide viewport, or a drawer
 *  along the bottom of a narrow one. */
export type TerminalPlacement = "side" | "bottom";

/**
 * Narrowest viewport that gets the side column. The explorer's three columns need about
 * 1000px (220 + 260 plus a dump wide enough for sixteen bytes and their labels), and a
 * terminal column that shows an `xxd` line unwrapped needs about 500px; below their sum
 * the drawer along the bottom is the better use of the space. Measured on the viewport,
 * not the screen, so a narrow window on a big monitor still gets the drawer.
 */
export const TERM_SIDE_MIN_VIEWPORT = 1500;

export function placementFor(innerWidth: number): TerminalPlacement {
  return innerWidth >= TERM_SIDE_MIN_VIEWPORT ? "side" : "bottom";
}

/** Narrowest useful column: about 44 columns of 12px mono. */
export const TERM_W_MIN_PX = 320;
/** The column never takes more than half the viewport. */
export const TERM_W_MAX_FRACTION = 0.5;
/** Width before the user has dragged the edge: an `xxd` line (67 characters) fits. */
export const TERM_W_DEFAULT_PX = 500;

/**
 * Clamp a requested column width to 320px..50% of the viewport, as an integer. The floor
 * wins when the viewport is so narrow that half of it is under 320px (which only happens
 * below the side-placement threshold anyway). A non-finite request falls back to the
 * default before clamping, so the store never persists NaN.
 */
export function clampWidth(px: number, innerWidth: number): number {
  const wanted = Number.isFinite(px) ? px : TERM_W_DEFAULT_PX;
  const max = Math.floor(innerWidth * TERM_W_MAX_FRACTION);
  return Math.max(TERM_W_MIN_PX, Math.min(Math.round(wanted), max));
}

/** Parse a persisted column width; same rules as `parseStoredHeight`, and likewise
 *  unclamped so a width chosen on a wider screen comes back when there is room again. */
export function parseStoredWidth(raw: string | null): number {
  return parseStoredPx(raw, TERM_W_DEFAULT_PX);
}
