import type { Volume } from "../lib/wasm";
import type { FsAdapter, FsFamily, FsFamilyId } from "./adapter";
import { fat16 } from "./fat16";

/** Every registered family, by id. A new family is added here and in `fs/panels.ts`. */
export const FAMILIES: Record<FsFamilyId, FsFamily> = { fat16 };
export const DEFAULT_FAMILY: FsFamilyId = "fat16";

/** The family whose `name` is this `Volume.fsType()` string ("FAT16" -> "fat16"). Throws for a
 *  type no adapter handles; the store's `load` turns that into a status line like any other
 *  load failure. When FAT32 shares the FAT adapter, this is the one place to remap. */
export function familyIdOf(fsType: string): FsFamilyId {
  for (const family of Object.values(FAMILIES)) if (family.name === fsType) return family.id;
  throw new Error(`no adapter for ${fsType}`);
}

export function adapterFor(vol: Volume): FsAdapter {
  return FAMILIES[familyIdOf(vol.fsType())].bind(vol);
}
