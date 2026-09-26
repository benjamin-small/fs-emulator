import { intervalsToSectors, normalize, type Interval } from "../core/intervals";
import { GAP_REVEAL } from "../core/segments";
import type { SelectionStore } from "./selection.svelte";
import type { VolumeStore } from "./volume.svelte";

export class LayersStore {
  /** Extra selection ranges other stores contribute (Task 6 adds the entry slot). */
  extraSel = $state.raw<Interval[]>([]);
  str = $state.raw<Interval[]>([]);
  visible = $state.raw<Interval>({ start: 0, end: 0 });
  /** The bytes under the pointer or focus in the What changed panel (a range of blocks, an
   *  event's region): the ribbon and the map outline them in `--focus`. Null when none. */
  hover = $state.raw<Interval | null>(null);

  // The tab's stores. The deriveds below read them lazily, after the constructor has run; they
  // are all `$derived.by` so the type checker sees the reads happen inside a function.
  private readonly volume: VolumeStore;
  private readonly selection: SelectionStore;

  constructor(volume: VolumeStore, selection: SelectionStore) {
    this.volume = volume;
    this.selection = selection;
  }

  diff: Interval[] = $derived.by(() => {
    const rec = this.volume.history[this.volume.cursor];
    return rec ? normalize(rec.changes.map((c) => ({ start: c.offset, end: c.offset + c.after.length }))) : [];
  });

  // Every derived below calls the adapter, whose caches are plain fields, so each one reads
  // `volume.epoch` first (the rule in fs/adapter.ts). Without it a delete would leave the old
  // chain highlighted until something else changed.
  chain: number[] = $derived.by(() => {
    const path = this.selection.path;
    if (!path) return [];
    this.volume.epoch;
    return this.volume.adapter.chain(path);
  });

  /** Byte range of the selected path's directory entry slots (FAT: LFN + short), if any. */
  entry: Interval | null = $derived.by(() => (this.selection.path ? (this.volume.epoch, this.volume.adapter.entrySlots(this.selection.path)) : null));

  /** Deleted directory slots and dirty free units, when the "Show remnants" toggle is on. A
   *  family without `adapter.remnants` has none. Empty while the timeline is rewound: the
   *  directory walk asks the live volume, so its slots would be from the latest state while
   *  the bytes on screen are from an older one. */
  remnant: Interval[] = $derived.by(() => (this.selection.showRemnants && this.volume.atLatest ? (this.volume.epoch, this.volume.adapter.remnants?.(this.volume.zeros) ?? []) : []));

  sel: Interval[] = $derived.by(() => (this.volume.epoch, normalize([...this.chain.map((u) => this.volume.adapter.unitByteRange(u)), ...(this.entry ? [this.entry] : []), ...this.extraSel])));

  pinnedSectors: Set<number> = $derived.by(() => {
    const { volume, selection } = this;
    const s = new Set<number>([...intervalsToSectors(this.diff, volume.sectorSize), ...intervalsToSectors(this.sel, volume.sectorSize), ...intervalsToSectors(this.remnant, volume.sectorSize)]);
    if (selection.cursorOffset !== null) s.add(Math.floor(selection.cursorOffset / volume.sectorSize));
    for (const g of selection.expandedGaps) for (let i = 0; i < GAP_REVEAL; i++) s.add(g + i);
    return s;
  });
}
