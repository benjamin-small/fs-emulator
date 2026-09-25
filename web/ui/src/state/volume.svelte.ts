import { Volume, type FsError, type OpRecord, type Region } from "../lib/wasm";
import { buildAttribution, type AttributionTable } from "../core/attribution";
import { applyChanges, changedSectors } from "../core/patch";
import { rescanSectors, scanZeroSectors } from "../core/zeros";
import { FAMILIES, adapterFor, familyIdOf } from "../fs";
import type { CrashPhase, FsAdapter, FsFamilyId } from "../fs/adapter";
import type { SelectionStore } from "./selection.svelte";

/** One tab's disk: the mounted volume, its cached image, and its timeline. Every format and
 *  every mount is of the tab's `family`; the constructor mounts that family's default disk. */
export class VolumeStore {
  readonly family: FsFamilyId;
  // `vol` and `adapter` are set by `adopt` in the constructor, before anything can read them.
  vol = $state.raw<Volume>()!;
  /** The per-family view of `vol`: owners, tables, unit arithmetic. Its caches are plain
   *  fields, not runes, so a `$derived` that calls a method on it must read `epoch` first or
   *  it keeps answering from the previous op (the rule stated in fs/adapter.ts). `adopt`
   *  re-binds it for a new volume; `refreshMeta` re-reads it after every op. */
  adapter = $state.raw<FsAdapter>()!;
  image = $state.raw<Uint8Array>(new Uint8Array(0));
  epoch = $state(0);
  layout = $state.raw<Region[]>([]);
  sectorSize = $state(0);
  totalSectors = $state(0);
  zeros = $state.raw<Uint8Array>(new Uint8Array(0));
  history = $state.raw<OpRecord[]>([]);
  cursor = $state(-1);
  status = $state<{ text: string; code?: string } | null>(null);
  /** News for the status line that is not an error; cleared with `status`. */
  notice = $state<string | null>(null);
  /** The `CorruptImage` message while a raw write has left the on-disk metadata unparsable,
   *  `null` while mounted. Refreshed alongside the adapter, so it tracks the volume
   *  through every op, format, and load. */
  corruption = $state<string | null>(null);
  /** True while the journal holds an unfinished transaction (the adapter's `needsRecovery`;
   *  always false on FAT and ext2). Refreshed with the adapter, like `corruption`. */
  needsRecovery = $state(false);
  /** The crash phase armed in the mounted journal, or null (always null without a journal).
   *  Arming is not an op, so it moves neither the timeline nor `epoch`: this field is how the
   *  Journal panel shows a phase the terminal's `crash` armed, and the reverse. `refreshMeta`
   *  re-reads it, because the next op uses the phase up and a format or load binds a new journal. */
  armedPhase = $state<CrashPhase | null>(null);

  // `epoch` is read first on purpose: `adapter.owners` is a plain field, so nothing else
  // in this expression would re-run it after an op.
  attribution: AttributionTable = $derived.by(() => { this.epoch; return buildAttribution(this.adapter, this.layout, this.adapter.owners); });
  atLatest = $derived(this.cursor === this.history.length - 1);

  private readonly selection: SelectionStore;
  /** Called after every adopt (a format or a mount): the workspace puts its shell back at /mnt. */
  private readonly onAdopt: () => void;

  constructor(family: FsFamilyId, selection: SelectionStore, onAdopt: () => void) {
    this.family = family;
    this.selection = selection;
    this.onAdopt = onAdopt;
    this.adopt(FAMILIES[family].format());
  }

  /** Take over a fresh Volume: bind its adapter, full image copy, full scans, empty history.
   *  The family and the adapter are resolved before any field is assigned, so a `fsType()` of
   *  another family, or with no registered adapter, throws before the store is half-switched
   *  to the new volume. */
  private adopt(vol: Volume) {
    const id = familyIdOf(vol.fsType());
    if (id !== this.family) throw new Error(`this tab mounts ${this.family}, not ${id}`);
    const adapter = adapterFor(vol);
    this.vol = vol;
    this.adapter = adapter;
    this.image = vol.image();
    this.layout = vol.layout();
    this.sectorSize = vol.sectorSize();
    this.totalSectors = vol.sectorCount();
    this.zeros = scanZeroSectors(this.image, this.sectorSize);
    this.history = [];
    this.cursor = -1;
    this.refreshMeta();
    this.epoch++;
    this.onAdopt();
  }

  private refreshMeta() {
    this.adapter.refresh();
    // `corruption()` is on the `FileSystem` trait, so every family answers it and it cannot
    // throw `NotFat`; no guard is needed.
    this.corruption = this.vol.corruption();
    this.needsRecovery = this.adapter.needsRecovery;
    this.armedPhase = this.adapter.journal?.phase() ?? null;
  }

  /** Arm a crash in the mounted journal, so its next op stops at `phase`, or disarm it (`null`).
   *  Without a journal there is nothing to arm. Returns false, with `status` set, when the volume
   *  refuses. */
  setArmedPhase(phase: CrashPhase | null): boolean {
    const journal = this.adapter.journal;
    if (!journal) return true;
    try {
      if (phase === null) journal.disarm();
      else journal.arm(phase);
    } catch (e) { this.report(e); return false; }
    this.armedPhase = journal.phase();
    return true;
  }

  /** A fresh volume of the tab's family with that family's options. `family` is kept for the
   *  callers that name one (the Format forms, `mkfs`, a lesson step); another family is refused
   *  into `status`, like any other format failure. */
  format(family: FsFamilyId = this.family, options?: unknown) {
    try {
      if (!Object.hasOwn(FAMILIES, family)) throw new Error(`no filesystem family "${family}"`);
      if (family !== this.family) throw new Error(`this tab formats ${this.family}, not ${family}`);
      this.mount(FAMILIES[family].format(options));
    } catch (e) {
      this.report(e);
    }
  }

  /** Take over `vol` as the tab's disk, clearing the messages and the selection. Throws, and
   *  leaves the store alone, when `vol` is another family's or no adapter handles its type. */
  mount(vol: Volume) {
    this.adopt(vol);
    this.clearMessages();
    this.selection.reset();
  }

  /** Mount an image. An unregistered or another family's `fsType()` makes `mount` throw, which
   *  lands in `status` like any other load failure. */
  load(bytes: Uint8Array) {
    try { this.mount(Volume.fromImage(bytes)); } catch (e) { this.report(e); }
  }

  export(): Uint8Array { return this.vol.image(); }

  /** Run a mutation at the latest state. Returns the record, or null on failure (status set). */
  run(fn: (v: Volume) => OpRecord): OpRecord | null {
    if (!this.atLatest) this.backToNow();
    let rec: OpRecord;
    try { rec = fn(this.vol); } catch (e) { this.report(e); return null; }
    applyChanges(this.image, rec.changes, "forward");
    rescanSectors(this.zeros, this.image, this.sectorSize, changedSectors(rec.changes, this.sectorSize));
    this.history = [...this.history, rec];
    this.cursor = this.history.length - 1;
    // A raw write into the family's metadata may have been adopted by the core (FAT
    // re-parses the boot sector), moving regions: only then is `layout` re-read. The
    // adapter's `refresh()` below re-reads the geometry on every op. Bytes per sector cannot
    // change (the core rejects that), so `sectorSize` and `zeros` stay valid.
    if (this.adapter.touchesMetadata(rec.changes)) this.layout = this.vol.layout();
    this.refreshMeta();
    this.clearMessages();
    this.epoch++;
    if (import.meta.env.DEV) this.checkInvariant();
    return rec;
  }

  /** View the disk as it was after `step` (0-based). No wasm calls; patches the cached image.
   *  The adapter and `layout` stay at the latest state, like the tree and the layers, so a
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
    this.clearMessages();
    this.epoch++;
  }

  backToNow() { this.seek(this.history.length - 1); }

  /** Put `e` (a wasm `FsError` or any Error) on the status line. */
  report(e: unknown) {
    const err = e as Partial<FsError>;
    this.status = { text: err.message ?? String(e), code: err.code };
  }

  /** Clear the status line: the error and the notice. */
  clearMessages() {
    this.status = null;
    this.notice = null;
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
