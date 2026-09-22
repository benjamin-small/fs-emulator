import type { FormatOptions, OpRecord, Volume } from "../lib/wasm";
import { clusterByteRange } from "../core/attribution";
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

  /** Begin `s` from a clean default disk, then apply its first step. */
  start(s: Scenario) {
    volume.format();
    this.current = s;
    this.index = -1;
    this.next();
  }

  /** Advance to the next step, applying its format/action/focus. No-op past the last step. */
  next() {
    if (!this.current) return;
    const i = this.index + 1;
    if (i >= this.current.steps.length) return;
    this.index = i;
    this.applyStep(this.current.steps[i]);
  }

  /** Step back and re-apply that step's focus. The disk itself is not replayed. */
  prev() {
    if (!this.current || this.index <= 0) return;
    this.index--;
    this.applyFocus(this.current.steps[this.index]);
  }

  stop() {
    this.current = null;
    this.index = -1;
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
