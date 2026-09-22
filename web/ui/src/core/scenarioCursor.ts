/** What a `next()` should do: run the step for the first time, or — because the
 *  step already ran once — just put the disk back where that run left it. */
export type NextPlan = { run: true } | { seekTo: number };

/**
 * Step bookkeeping for scenario replay, kept free of runes so it can be unit
 * tested under Node.
 *
 * A scenario step is only ever *run* once. `advance()` records the volume cursor
 * the run left behind, and stepping back and forward again replays history by
 * seeking to those recorded cursors instead of re-running the action (which would
 * append duplicate operations to the timeline).
 */
export class ScenarioCursor {
  /** Index of the current step, or -1 before the first `next()`. */
  index = -1;
  /** `reached[i]` is `volume.cursor` after step `i` ran; index-aligned, dense. */
  private reached: number[] = [];

  constructor(private readonly stepCount: number) {}

  /** True when the step after the current one has never been run. */
  canRunNext(): boolean {
    return this.index + 1 < this.stepCount && this.index + 1 >= this.reached.length;
  }

  /** Move to the next step and say how to get there. `null` past the last step. */
  next(): NextPlan | null {
    const i = this.index + 1;
    if (i >= this.stepCount) return null;
    this.index = i;
    return i < this.reached.length ? { seekTo: this.reached[i] } : { run: true };
  }

  /** Record the volume cursor left by the step `next()` just said to run. */
  advance(cursorAfterRun: number): void {
    if (this.index >= 0) this.reached[this.index] = cursorAfterRun;
  }

  /** Step back; returns the volume cursor to restore, or `null` at the first step. */
  back(): number | null {
    if (this.index <= 0) return null;
    this.index--;
    return this.reached[this.index] ?? null;
  }
}
