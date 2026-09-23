import { Volume, type ClusterOwner, type FatEntry, type FormatOptions, type FsError, type Geometry, type OpRecord, type Region } from "../lib/wasm";
import { buildAttribution, type AttributionTable } from "../core/attribution";
import { applyChanges, changedSectors, touchesBootSector } from "../core/patch";
import { rescanSectors, scanZeroSectors } from "../core/zeros";
import { selection } from "./selection.svelte";

export class VolumeStore {
  vol = $state.raw<Volume>(Volume.formatFat16(undefined));
  image = $state.raw<Uint8Array>(new Uint8Array(0));
  epoch = $state(0);
  geometry = $state.raw<Geometry>(this.vol.geometry());
  layout = $state.raw<Region[]>(this.vol.layout());
  owners = $state.raw<ClusterOwner[]>([]);
  fat = $state.raw<FatEntry[]>([]);
  zeros = $state.raw<Uint8Array>(new Uint8Array(0));
  history = $state.raw<OpRecord[]>([]);
  cursor = $state(-1);
  status = $state<{ text: string; code?: string } | null>(null);
  /** The `CorruptImage` message while a raw write has left the boot sector unparsable,
   *  `null` while mounted. Refreshed alongside `owners`/`fat`, so it tracks the volume
   *  through every op, format, and load. */
  corruption = $state<string | null>(null);

  attribution: AttributionTable = $derived(buildAttribution(this.geometry, this.layout, this.owners));
  sectorSize = $derived(this.geometry.bytesPerSector);
  atLatest = $derived(this.cursor === this.history.length - 1);

  constructor() { this.adopt(this.vol); }

  /** Take over a fresh Volume: full image copy, full scans, empty history. */
  private adopt(vol: Volume) {
    this.vol = vol;
    this.image = vol.image();
    this.geometry = vol.geometry();
    this.layout = vol.layout();
    this.zeros = scanZeroSectors(this.image, this.geometry.bytesPerSector);
    this.history = [];
    this.cursor = -1;
    this.refreshMeta();
    this.epoch++;
  }

  private refreshMeta() {
    this.owners = this.vol.clusterOwners();
    this.fat = this.vol.fatEntries(0);
    // `corruption()` will throw `NotFat` on a non-FAT volume in the future; fall back to
    // "not corrupt" rather than let that leave `this.corruption` stale.
    try { this.corruption = this.vol.corruption(); } catch { this.corruption = null; }
  }

  format(options?: FormatOptions) {
    try { this.adopt(Volume.formatFat16(options)); this.status = null; selection.reset(); } catch (e) { this.fail(e); }
  }

  load(bytes: Uint8Array) {
    try { this.adopt(Volume.fromImage(bytes)); this.status = null; selection.reset(); } catch (e) { this.fail(e); }
  }

  export(): Uint8Array { return this.vol.image(); }

  /** Run a mutation at the latest state. Returns the record, or null on failure (status set). */
  run(fn: (v: Volume) => OpRecord): OpRecord | null {
    if (!this.atLatest) this.backToNow();
    let rec: OpRecord;
    try { rec = fn(this.vol); } catch (e) { this.fail(e); return null; }
    applyChanges(this.image, rec.changes, "forward");
    rescanSectors(this.zeros, this.image, this.sectorSize, changedSectors(rec.changes, this.sectorSize));
    this.history = [...this.history, rec];
    this.cursor = this.history.length - 1;
    // A raw write into sector 0 may have been adopted as a new boot sector (the core
    // re-parses it), moving the root directory and data regions. Bytes per sector cannot
    // change (the core rejects that), so `sectorSize` and `zeros` stay valid.
    if (touchesBootSector(rec.changes)) { this.geometry = this.vol.geometry(); this.layout = this.vol.layout(); }
    this.refreshMeta();
    this.status = null;
    this.epoch++;
    if (import.meta.env.DEV) this.checkInvariant();
    return rec;
  }

  /** View the disk as it was after `step` (0-based). No wasm calls; patches the cached image.
   *  `geometry` and `layout` stay at the latest state, like the tree and the layers, so a
   *  rewound view of a boot-sector change shows the older bytes under the newest layout. */
  seek(step: number) {
    step = Math.max(-1, Math.min(step, this.history.length - 1));
    if (step === this.cursor) return;
    const touched = new Set<number>();
    if (step < this.cursor) {
      for (let i = this.cursor; i > step; i--) { applyChanges(this.image, this.history[i].changes, "reverse"); changedSectors(this.history[i].changes, this.sectorSize).forEach((s) => touched.add(s)); }
    } else {
      for (let i = this.cursor + 1; i <= step; i++) { applyChanges(this.image, this.history[i].changes, "forward"); changedSectors(this.history[i].changes, this.sectorSize).forEach((s) => touched.add(s)); }
    }
    rescanSectors(this.zeros, this.image, this.sectorSize, touched);
    this.cursor = step;
    // The error belonged to the state being left behind; keep it off the new one.
    this.status = null;
    this.epoch++;
  }

  backToNow() { this.seek(this.history.length - 1); }

  private fail(e: unknown) {
    const err = e as Partial<FsError>;
    this.status = { text: err.message ?? String(e), code: err.code };
  }

  private checkInvariant() {
    // Hoist both arrays into locals: `this.image` is a `$state.raw` field, so
    // reading it per byte costs a proxy/signal read on a multi-megabyte loop.
    const truth = this.vol.image(), mine = this.image;
    for (let i = 0; i < truth.length; i++) {
      if (truth[i] !== mine[i]) { console.error(`cached image drifted from the volume at offset ${i}`); return; }
    }
  }
}

export const volume = new VolumeStore();
