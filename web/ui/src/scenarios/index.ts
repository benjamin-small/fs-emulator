import type { Scenario } from "../state/scenarios.svelte";
import { scenario as directory } from "./directory";
import { scenario as deleteRemnants } from "./deleteRemnants";
import { scenario as fillDisk } from "./fillDisk";
import { scenario as format } from "./format";
import { scenario as longName } from "./longName";
import { scenario as overwriteGrows } from "./overwriteGrows";
import { scenario as shell } from "./shell";
import { scenario as smallFile } from "./smallFile";

export const all: Scenario[] = [format, smallFile, longName, overwriteGrows, deleteRemnants, fillDisk, directory, shell];
