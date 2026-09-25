import type { Volume } from "../lib/wasm";
import type { FsAdapter, FsFamily, FsFamilyId } from "./adapter";
import { ext } from "./ext";
import { fat16 } from "./fat16";

/** Every registered family, by id, in the order the Format details' Filesystem select lists
 *  them. A new family is added here and in `fs/panels.ts`. */
export const FAMILIES: Record<FsFamilyId, FsFamily> = { fat16, ext };
export const DEFAULT_FAMILY: FsFamilyId = "fat16";

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
