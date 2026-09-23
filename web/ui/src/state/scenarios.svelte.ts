import type { OpRecord, Volume } from "../lib/wasm";
import type { FsAdapter, FsFamilyId } from "../fs/adapter";
import { ScenarioCursor } from "../core/scenarioCursor";
import { selection } from "./selection.svelte";
import { volume } from "./volume.svelte";

export interface StepFocus {
  offset?: number;
  sector?: number;
  unit?: number;
  path?: string | null;
  showRemnants?: boolean;
  strings?: boolean;
}

/** One step of a lesson. `O` is the family's format options: a step that carries `format`
 *  re-formats the disk with them before its action runs. Actions and function focuses get
 *  the adapter bound to the volume they run on, so a scenario reads geometry and ownership
 *  through the seam (`fs.unitSize`, `fs.entrySlots(path)`, ...) and never through a
 *  family-specific wasm call. */
export interface Step<O = unknown> {
  title: string;
  text: string;
  action?: (v: Volume, fs: FsAdapter) => OpRecord;
  format?: O;
  focus?: StepFocus | ((fs: FsAdapter) => StepFocus);
}

export interface Scenario<O = unknown> {
  id: string;
  title: string;
  summary: string;
  /** The family `start()` formats before the first step; every step runs on that family. */
  family: FsFamilyId;
  steps: Step<O>[];
}

/** Drives `volume`/`selection` through a scenario's steps, one at a time. */
export class ScenarioRunner {
  current = $state<Scenario | null>(null);
  index = $state(-1);
  readonly step: Step | null = $derived(this.current ? (this.current.steps[this.index] ?? null) : null);
  /** The current step's focus with any `(fs: FsAdapter) => StepFocus` already resolved, so
   *  the Lesson card can describe where it pointed the UI without resolving it a second time
   *  (the function form reads the live adapter, and would answer differently later). */
  focus = $state<StepFocus | null>(null);

  /** Step bookkeeping (which steps have run, and the volume cursor each one left). */
  private cursor: ScenarioCursor | null = null;

  /** Begin `s` from a clean default disk of its family, then apply its first step. */
  start(s: Scenario) {
    volume.format(s.family); // also resets the selection
    selection.reset();
    this.current = s;
    this.index = -1;
    this.cursor = new ScenarioCursor(s.steps.length);
    this.next();
  }

  /** Advance to the next step. A step that has already run is replayed by seeking the
   *  timeline back to where its run left the disk, never by running it again. No-op
   *  past the last step. */
  next() {
    if (!this.current || !this.cursor) return;
    const plan = this.cursor.next();
    if (!plan) return;
    this.index = this.cursor.index;
    const step = this.current.steps[this.index];
    if ("seekTo" in plan) {
      volume.seek(plan.seekTo);
      this.applyFocus(step);
      return;
    }
    this.applyStep(step);
    this.cursor.advance(volume.cursor);
  }

  /** Step back: show the disk as it was after the previous step and re-apply its focus.
   *  A step that formatted the disk cannot be rewound through (the format threw the old
   *  history away), so Prev clamps there. */
  prev() {
    if (!this.current || !this.cursor || this.index <= 0) return;
    if (this.current.steps[this.index].format) return;
    const target = this.cursor.back();
    this.index = this.cursor.index;
    if (target !== null) volume.seek(target);
    this.applyFocus(this.current.steps[this.index]);
  }

  stop() {
    this.current = null;
    this.index = -1;
    this.cursor = null;
    this.focus = null;
    selection.showRemnants = false;
    selection.stringsOn = false;
  }

  private applyStep(step: Step) {
    // `next()` is the only caller and returns early without a current scenario.
    if (step.format) volume.format(this.current!.family, step.format);
    if (step.action) volume.run((v) => step.action!(v, volume.adapter));
    this.applyFocus(step);
  }

  /** Every path that lands on a step ends here — a fresh run, a Next that seeks to a step
   *  that already ran, and Prev — so this is where the resolved focus is published. */
  private applyFocus(step: Step) {
    const focus = typeof step.focus === "function" ? step.focus(volume.adapter) : step.focus;
    this.focus = focus ?? null;
    if (!focus) return;
    if (focus.path !== undefined) selection.select(focus.path);
    if (focus.offset !== undefined) selection.jumpTo(focus.offset);
    else if (focus.sector !== undefined) selection.jumpTo(focus.sector * volume.sectorSize);
    else if (focus.unit !== undefined) selection.jumpTo(volume.adapter.unitByteRange(focus.unit).start);
    if (focus.showRemnants !== undefined) selection.showRemnants = focus.showRemnants;
    if (focus.strings !== undefined) selection.stringsOn = focus.strings;
  }
}

export const scenarios = new ScenarioRunner();
