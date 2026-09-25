import type { OpRecord, Volume } from "../../src/lib/wasm";
import { adapterFor, FAMILIES } from "../../src/fs";
import type { FsAdapter } from "../../src/fs/adapter";
import type { Scenario } from "../../src/state/scenarios.svelte";

/**
 * What the ext lesson tests share, bound to one lesson: a step's text by title, a run of the
 * lesson's actions the way the runner makes it, and a step's focus as the runner resolves it.
 */
export function lessonHelpers(scenario: Scenario) {
  /** The text of the step with that title; an unknown title throws, so a renamed step fails. */
  const stepText = (title: string): string => {
    const s = scenario.steps.find((st) => st.title === title);
    if (!s) throw new Error(`no step titled "${title}"`);
    return s.text;
  };

  /** Run the lesson the way the runner does: a format of its family, then each action in order
   *  with the adapter refreshed after it, stopping after the step with the given title (after
   *  the last step when none is given). */
  const runThrough = (title?: string): { vol: Volume; fs: FsAdapter; records: OpRecord[] } => {
    const vol = FAMILIES[scenario.family].format();
    const fs = adapterFor(vol);
    const records: OpRecord[] = [];
    for (const step of scenario.steps) {
      if (step.action) {
        records.push(step.action(vol, fs));
        fs.refresh();
      }
      if (step.title === title) break;
    }
    return { vol, fs, records };
  };

  /** A step's focus as the runner resolves it: the function form gets the bound adapter. */
  const focusOf = (title: string, fs: FsAdapter) => {
    const s = scenario.steps.find((st) => st.title === title)!;
    return typeof s.focus === "function" ? s.focus(fs) : s.focus;
  };

  return { stepText, runThrough, focusOf };
}
