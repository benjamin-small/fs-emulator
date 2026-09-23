/** Geometry for a floating card that the user drags around the page (the Lesson card). */
export interface Point {
  x: number;
  y: number;
}
export interface Size {
  w: number;
  h: number;
}

/** Gap kept between a floating card and the viewport edge. */
export const FLOAT_MARGIN = 8;

/**
 * Keep a card on screen: its top-left corner stays inside the viewport by the margin on
 * every side. A card larger than the viewport keeps its top-left corner at the margin,
 * so the bar you drag it by is always reachable. Rounded, so a dragged card never lands
 * on a fractional pixel.
 */
export function clampPosition(pos: Point, size: Size, viewport: Size, margin = FLOAT_MARGIN): Point {
  const maxX = Math.max(margin, viewport.w - size.w - margin);
  const maxY = Math.max(margin, viewport.h - size.h - margin);
  return {
    x: Math.round(Math.min(Math.max(pos.x, margin), maxX)),
    y: Math.round(Math.min(Math.max(pos.y, margin), maxY)),
  };
}

/**
 * Where a card goes before the user has moved it: the top-right corner of the region
 * it used to live in (the top of the right column), clamped like any other position.
 */
export function defaultPosition(size: Size, viewport: Size, top: number, rightInset = 16): Point {
  return clampPosition({ x: viewport.w - size.w - rightInset, y: top }, size, viewport);
}

/**
 * Parse a remembered position: a JSON object with finite numeric `x` and `y`. Anything
 * else (absent, hand-edited, an older shape) means "not moved yet", so the default applies.
 */
export function parseStoredPosition(raw: string | null): Point | null {
  if (raw === null) return null;
  try {
    const v: unknown = JSON.parse(raw);
    if (typeof v !== "object" || v === null) return null;
    const { x, y } = v as { x?: unknown; y?: unknown };
    if (typeof x !== "number" || typeof y !== "number" || !Number.isFinite(x) || !Number.isFinite(y)) return null;
    return { x, y };
  } catch {
    return null;
  }
}

/** Arrow-key step for the keyboard handle, in px. */
export const NUDGE_PX = 10;
export const NUDGE_FAST_PX = 40;

/** The position an arrow key moves a card to (Shift for the bigger step), or null for any
 *  other key so the caller leaves the event alone. */
export function nudge(pos: Point, key: string, shift: boolean): Point | null {
  const d = shift ? NUDGE_FAST_PX : NUDGE_PX;
  switch (key) {
    case "ArrowLeft": return { x: pos.x - d, y: pos.y };
    case "ArrowRight": return { x: pos.x + d, y: pos.y };
    case "ArrowUp": return { x: pos.x, y: pos.y - d };
    case "ArrowDown": return { x: pos.x, y: pos.y + d };
    default: return null;
  }
}
