import type { Scenario } from "../state/scenarios.svelte";
import type { FsFamilyId } from "../fs/adapter";
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

// Each family's fundamentals go first: a tab's picker defaults to its first lesson, and the
// tour is where a newcomer should start before watching individual operations. The ext
// lessons follow the FAT ones.
export const all: Scenario[] = [
  fundamentals, format, smallFile, longName, overwriteGrows, deleteRemnants, fillDisk, directory, shell,
  extFundamentals, journaledWrite, crashRecover,
];

/** A tab's lessons: the ones for `family`, in `list` order. The tab's picker lists these only. */
export function lessonsFor(family: FsFamilyId, list: readonly Scenario[] = all): Scenario[] {
  return list.filter((s) => s.family === family);
}

/** The lesson a tab's picker starts on: the first of its lessons, the family's fundamentals. */
export function defaultLessonFor(family: FsFamilyId, list: readonly Scenario[] = all): Scenario {
  const first = lessonsFor(family, list)[0];
  if (!first) throw new Error(`no lessons for ${family}`);
  return first;
}
