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
