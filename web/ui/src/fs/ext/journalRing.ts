import { COLOR_BOOT, COLOR_TABLE } from "../../core/palette";
import { CRASH_PHASE_LABELS, type CrashPhase, type JournalRingBlock, type JournalState } from "../adapter";

/**
 * The Journal panel's model, kept out of the component so the node tests can check it: the
 * text it prints and the colour of each journal block's cell. The ring is drawn in reading
 * order (journal index 0 first), one cell per block, on the shared grid (core/grid.ts) in the
 * block-group map's cell size and scroll box, so the two strips read alike.
 */

/** A block of a checkpointed (or discarded) transaction is drawn at this alpha; live ones full. */
export const STALE_ALPHA = 0.35;

export const NO_JOURNAL = "This volume has no journal.";
export const NEEDS_RECOVERY = "Needs recovery: the journal holds an unfinished transaction.";

/** `Journal · ordered mode`. */
export function journalHeading(s: JournalState): string {
  return `Journal · ${s.mode} mode`;
}

/** The facts list: `sequence`, `head`, `start`, and `blocks` (the ring's length, `maxlen`). */
export function journalFacts(s: JournalState): [string, string][] {
  return [["sequence", s.sequence.toLocaleString()], ["head", s.head.toLocaleString()], ["start", s.start.toLocaleString()], ["blocks", s.maxlen.toLocaleString()]];
}

/** The line that replaces the phase select while a crash is armed. */
export function armedText(phase: CrashPhase): string {
  return `Armed: the next change stops ${CRASH_PHASE_LABELS[phase]}`;
}

/** The CSS custom property each kind of journal block is drawn in. */
export const RING_COLORS: Record<JournalRingBlock["kind"], string> = {
  superblock: `--own-${COLOR_BOOT}`,
  descriptor: `--own-${COLOR_TABLE}`,
  copy: "--ink",
  commit: "--focus",
  revoke: "--diff",
  unused: "--hairline",
};

/** A cell's colour token and alpha. */
export function ringFill(b: JournalRingBlock): { token: string; alpha: number } {
  return { token: RING_COLORS[b.kind], alpha: b.stale ? STALE_ALPHA : 1 };
}

/** The hover caption: `journal block 2 (block 84) · copy · copy of block 1 · transaction 1 ·
 *  stale`; the home only on a copy, the transaction and its staleness only when there is one. */
export function ringCaption(b: JournalRingBlock): string {
  let s = `journal block ${b.index} (block ${b.block}) · ${b.kind}`;
  if (b.kind === "copy" && b.home !== null) s += ` · copy of block ${b.home}`;
  if (b.tid !== null) s += ` · transaction ${b.tid} · ${b.stale ? "stale" : "live"}`;
  return s;
}
