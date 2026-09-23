import type { Scenario } from "../state/scenarios.svelte";
import { scenario as directory } from "./directory";
import { scenario as deleteRemnants } from "./deleteRemnants";
import { scenario as fillDisk } from "./fillDisk";
import { scenario as format } from "./format";
import { scenario as fundamentals } from "./fundamentals";
import { scenario as longName } from "./longName";
import { scenario as overwriteGrows } from "./overwriteGrows";
import { scenario as shell } from "./shell";
import { scenario as smallFile } from "./smallFile";

// The fundamentals go first: the picker defaults to the first entry, and the tour is where a
// newcomer should start before watching individual operations.
export const all: Scenario[] = [fundamentals, format, smallFile, longName, overwriteGrows, deleteRemnants, fillDisk, directory, shell];
