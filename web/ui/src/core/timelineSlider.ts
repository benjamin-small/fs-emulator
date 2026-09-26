/**
 * The step under the pointer on the Operation bar's slider: `steps` ticks spread evenly across
 * `width` pixels (the first at 0, the last at `width`), and `x` picks the nearest, clamped to the
 * ends. With one step or none, or before the slider has a width, it is step 0.
 */
export function stepAtPointer(x: number, width: number, steps: number): number {
  if (steps <= 1 || width <= 0) return 0;
  const step = Math.round((x / width) * (steps - 1));
  return Math.max(0, Math.min(steps - 1, step));
}
