import type { Component } from "svelte";
import type { FsFamilyId } from "./adapter";
import FatMap from "./fat16/FatMap.svelte";
import FormatForm from "./fat16/FormatForm.svelte";
import BlockGroupMap from "./ext/BlockGroupMap.svelte";
import ExtFormatForm from "./ext/ExtFormatForm.svelte";
import JournalPanel from "./ext/JournalPanel.svelte";

/**
 * The Svelte panels of each family, looked up by `volume.adapter.id`: `map` is the panel App
 * renders under the Files tree (the FAT map, the block-group map), `format` the body of the
 * Actions panel's Format details for the family its Filesystem select names, and `extras` the
 * family's further panels, rendered under the map in order (FAT has none; ext has the Journal
 * panel, which on ext2 says there is no journal). They live here rather than on the adapter
 * because `vitest.config.ts` has no Svelte plugin: an adapter that imported a `.svelte` file
 * could not be loaded by the node tests.
 */
export const PANELS: Record<FsFamilyId, { map: Component; format: Component; extras: Component[] }> = {
  fat16: { map: FatMap, format: FormatForm, extras: [] },
  ext: { map: BlockGroupMap, format: ExtFormatForm, extras: [JournalPanel] },
};
