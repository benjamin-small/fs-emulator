import type { Scenario } from "../state/scenarios.svelte";
import type { FsFamilyId } from "../fs/adapter";
import { FAMILIES } from "../fs";
import { scenario as crashRecover } from "./crashRecover";
import { scenario as directory } from "./directory";
import { scenario as deleteRemnants } from "./deleteRemnants";
import { scenario as extFundamentals } from "./extFundamentals";
import { scenario as fillDisk } from "./fillDisk";
import { scenario as format } from "./format";
import { scenario as fundamentals } from "./fundamentals";
import { scenario as journaledWrite } from "./journaledWrite";
import { scenario as longName } from "./longName";
import { scenario as overwriteGrows } from "./overwriteGrows";
import { scenario as shell } from "./shell";
import { scenario as smallFile } from "./smallFile";

// The fundamentals go first: the picker defaults to the first entry, and the tour is where a
// newcomer should start before watching individual operations. The ext lessons follow the FAT
// ones, the ext tour first for the same reason.
export const all: Scenario[] = [
  fundamentals, format, smallFile, longName, overwriteGrows, deleteRemnants, fillDisk, directory, shell,
  extFundamentals, journaledWrite, crashRecover,
];

export interface ScenarioGroup { family: FsFamilyId; label: string; scenarios: Scenario[] }

/** The picker's `<optgroup>`s: one per registered family that has lessons, in registry order,
 *  labelled with the family's display name, each holding its lessons in `list` order. */
export function scenarioGroups(list: readonly Scenario[] = all): ScenarioGroup[] {
  return Object.values(FAMILIES)
    .map((f) => ({ family: f.id, label: f.name, scenarios: list.filter((s) => s.family === f.id) }))
    .filter((g) => g.scenarios.length > 0);
}
