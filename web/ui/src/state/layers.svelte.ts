import { clusterByteRange } from "../core/attribution";
import { findEntrySlots } from "../core/direntry";
import { buildChain } from "../core/fatchain";
import { intervalsToSectors, normalize, type Interval } from "../core/intervals";
import { selection } from "./selection.svelte";
import { volume } from "./volume.svelte";

export class LayersStore {
  /** Extra selection ranges other stores contribute (Task 6 adds the entry slot). */
  extraSel = $state.raw<Interval[]>([]);
  str = $state.raw<Interval[]>([]);
  visible = $state.raw<Interval>({ start: 0, end: 0 });

  diff: Interval[] = $derived.by(() => {
    const rec = volume.history[volume.cursor];
    return rec ? normalize(rec.changes.map((c) => ({ start: c.offset, end: c.offset + c.after.length }))) : [];
  });

  chain: number[] = $derived.by(() => {
    const path = selection.path;
    if (!path) return [];
    const owner = volume.owners.find((o) => o.path === path);
    return owner ? buildChain(volume.fat, owner.firstCluster) : [];
  });

  /** Byte range of the selected path's LFN + short directory entries, if any (Task 6). */
  entry: Interval | null = $derived(selection.path ? (volume.epoch, findEntrySlots(volume.vol, volume.geometry, volume.fat, volume.owners, selection.path)) : null);

  sel: Interval[] = $derived(normalize([...this.chain.map((c) => clusterByteRange(volume.geometry, c)), ...(this.entry ? [this.entry] : []), ...this.extraSel]));

  pinnedSectors: Set<number> = $derived.by(() => {
    const s = new Set<number>([...intervalsToSectors(this.diff, volume.sectorSize), ...intervalsToSectors(this.sel, volume.sectorSize)]);
    if (selection.cursorOffset !== null) s.add(Math.floor(selection.cursorOffset / volume.sectorSize));
    for (const g of selection.expandedGaps) for (let i = 0; i < 64; i++) s.add(g + i);
    return s;
  });
}

export const layers = new LayersStore();
