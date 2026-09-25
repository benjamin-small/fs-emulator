import type { Volume } from "../lib/wasm";
import type { FsAdapter, FsFamily, FsFamilyId } from "./adapter";
import { ext } from "./ext";
import { fat16 } from "./fat16";

/** Every registered family, by id, in the order of the top bar's tabs. A new family is added
 *  here and in `fs/panels.ts`. */
export const FAMILIES: Record<FsFamilyId, FsFamily> = { fat16, ext };
/** The tab opened when the URL hash names none. */
export const DEFAULT_FAMILY: FsFamilyId = "fat16";
/** The family ids in tab order; a family's workspace is created the first time its tab opens. */
export const FAMILY_IDS = Object.keys(FAMILIES) as FsFamilyId[];
/** Other hash spellings that open a family's tab (`#fat`, `#ext2`, `#ext3`); `parseRoute`
 *  looks these up before the ids themselves. */
export const ROUTE_ALIASES: Readonly<Record<string, FsFamilyId>> = { fat: "fat16", ext2: "ext", ext3: "ext" };

/** The family whose `fsTypes` lists this `Volume.fsType()` string ("FAT16" -> "fat16"; one
 *  family may bind several types, as ext binds ext2 and ext3). Throws for a type no adapter
 *  handles; the store's `load` turns that into a status line like any other load failure. */
export function familyIdOf(fsType: string): FsFamilyId {
  for (const family of Object.values(FAMILIES)) if (family.fsTypes.includes(fsType)) return family.id;
  throw new Error(`no adapter for ${fsType}`);
}

export function adapterFor(vol: Volume): FsAdapter {
  return FAMILIES[familyIdOf(vol.fsType())].bind(vol);
}
