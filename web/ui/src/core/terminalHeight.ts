/** Smallest useful drawer: the bar plus about five terminal rows. */
export const TERM_MIN_PX = 120;
/** The drawer never takes more than this share of the viewport. */
export const TERM_MAX_FRACTION = 0.6;
/** Height before the user has dragged the bar. */
export const TERM_DEFAULT_PX = 220;

/**
 * Clamp a requested drawer height to 120px..60% of the viewport, as an integer.
 * The floor wins when the viewport is so short that 60% is under 120px. A
 * non-finite request (a corrupt localStorage value) falls back to the default
 * before clamping, so the store never persists NaN.
 */
export function clampHeight(px: number, innerHeight: number): number {
  const wanted = Number.isFinite(px) ? px : TERM_DEFAULT_PX;
  const max = Math.floor(innerHeight * TERM_MAX_FRACTION);
  return Math.max(TERM_MIN_PX, Math.min(Math.round(wanted), max));
}

/**
 * Parse a persisted drawer height. Absent, empty, whitespace, or non-numeric values (a
 * cleared or hand-edited localStorage entry; note `Number("")` is 0) fall back to the
 * default. Deliberately does not clamp: the stored value is the user's choice, and
 * `clampHeight` narrows it to the current viewport only for rendering, so a height
 * chosen on a taller screen comes back when there is room for it again.
 */
export function parseStoredHeight(raw: string | null): number {
  if (raw === null) return TERM_DEFAULT_PX;
  const trimmed = raw.trim();
  if (trimmed === "") return TERM_DEFAULT_PX;
  const n = Number(trimmed);
  return Number.isFinite(n) ? n : TERM_DEFAULT_PX;
}
