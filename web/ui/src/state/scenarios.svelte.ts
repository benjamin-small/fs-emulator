import type { FormatOptions, OpRecord, Volume } from "../lib/wasm";
import { clusterByteRange } from "../core/attribution";
import { ScenarioCursor } from "../core/scenarioCursor";
import { selection } from "./selection.svelte";
import { volume } from "./volume.svelte";

export interface StepFocus {
  offset?: number;
  sector?: number;
  cluster?: number;
  path?: string | null;
  showRemnants?: boolean;
  strings?: boolean;
}

export interface Step {
  title: string;
  text: string;
  action?: (v: Volume) => OpRecord;
  format?: FormatOptions;
  focus?: StepFocus | ((v: Volume) => StepFocus);
}

export interface Scenario {
  id: string;
  title: string;
  summary: string;
  steps: Step[];
}

/** Drives `volume`/`selection` through a scenario's steps, one at a time. */
export class ScenarioRunner {
  current = $state<Scenario | null>(null);
  index = $state(-1);
  readonly step: Step | null = $derived(this.current ? (this.current.steps[this.index] ?? null) : null);

  /** Step bookkeeping (which steps have run, and the volume cursor each one left). */
  private cursor: ScenarioCursor | null = null;

  /** Begin `s` from a clean default disk, then apply its first step. */
  start(s: Scenario) {
    volume.format(); // also resets the selection
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
    selection.showRemnants = false;
    selection.stringsOn = false;
  }

  private applyStep(step: Step) {
    if (step.format) volume.format(step.format);
    if (step.action) volume.run(step.action);
    this.applyFocus(step);
  }

  private applyFocus(step: Step) {
    const focus = typeof step.focus === "function" ? step.focus(volume.vol) : step.focus;
    if (!focus) return;
    if (focus.path !== undefined) selection.select(focus.path);
    if (focus.offset !== undefined) selection.jumpTo(focus.offset);
    else if (focus.sector !== undefined) selection.jumpTo(focus.sector * volume.geometry.bytesPerSector);
    else if (focus.cluster !== undefined) selection.jumpTo(clusterByteRange(volume.geometry, focus.cluster).start);
    if (focus.showRemnants !== undefined) selection.showRemnants = focus.showRemnants;
    if (focus.strings !== undefined) selection.stringsOn = focus.strings;
  }
}

export const scenarios = new ScenarioRunner();
