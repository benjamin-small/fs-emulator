import { stepFocusOffset } from "../core/stepFocus";
import { selection } from "./selection.svelte";
import { volume } from "./volume.svelte";

/**
 * Recenter the dump on what step `step` changed. Running an operation never moves the
 * dump — only explicit navigation does — so this is called from the Timeline's controls
 * and from App.svelte's `[`/`]` shortcuts, right after `volume.seek(step)`.
 *
 * A step with no changes (or an index off either end of the history) leaves the dump
 * alone.
 */
export function focusHistoryStep(step: number) {
  const offset = stepFocusOffset(volume.history[step]);
  if (offset !== null) selection.jumpTo(offset);
}
