import type { Interval } from "./intervals";

/** The phases the What changed panel groups a record's events into. */
export type PhaseId = "directory" | "allocation" | "data" | "body" | "journal" | "checkpoint" | "cleanup" | "recovery" | "crash" | "other";

/** One event of an operation record: wasm's `EventRecord`, its region in bytes. */
export interface PhaseEvent { kind: string; text: string; region: Interval | null }

/** A phase and its events in record order, each with its index in the record. */
export interface Phase { id: PhaseId; label: string; events: (PhaseEvent & { index: number })[] }

/** Each phase's name, the start of its `<summary>`: "Journal · 9 events". */
export const PHASE_LABELS: Record<PhaseId, string> = {
  directory: "Directory entry",
  allocation: "Allocation",
  data: "Data",
  body: "Filesystem writes",
  journal: "Journal",
  checkpoint: "Checkpoint",
  cleanup: "Cleanup",
  recovery: "Recovery",
  crash: "Crash",
  other: "Other",
};

/** The journal's kinds (ext3) and their phases. A record with any of them is grouped ext3-style. */
const JOURNAL_PHASES: Readonly<Record<string, PhaseId>> = {
  recovery_flag_set: "journal",
  transaction_started: "journal",
  journal_block_written: "journal",
  checkpointed: "checkpoint",
  journal_emptied: "cleanup",
  recovery_flag_cleared: "cleanup",
  recovery_scanned: "recovery",
  replayed: "recovery",
  transaction_discarded: "recovery",
  crashed: "crash",
};

/** The filesystem's own kinds (FAT16, ext2, and ext3's writes before it journals them), grouped
 *  FAT-style. `dir_entry_deleted` and `directory_grown` are FAT's, `raw_write` any family's. */
const FS_PHASES: Readonly<Record<string, PhaseId>> = {
  dir_entry_written: "directory",
  dir_entry_removed: "directory",
  dir_entry_deleted: "directory",
  directory_grown: "directory",
  fat_entry_set: "allocation",
  cluster_allocated: "allocation",
  cluster_freed: "allocation",
  blocks_allocated: "allocation",
  blocks_freed: "allocation",
  bitmap_updated: "allocation",
  counters_updated: "allocation",
  inode_allocated: "allocation",
  inode_freed: "allocation",
  inode_written: "allocation",
  indirect_written: "allocation",
  data_written: "data",
  raw_write: "data",
};

/**
 * Group an operation's events into phases, listed in the order of each phase's first event.
 * A record with any journal kind is grouped ext3-style: every other kind is one "Filesystem
 * writes" phase (the writes the transaction then journals), and the journal's kinds are
 * Journal, Checkpoint, Cleanup, Recovery, or Crash. A record without one (FAT16, ext2, a raw
 * write) is grouped FAT-style: Directory entry, Allocation, Data, and Other for a kind this
 * table does not know.
 */
export function groupEvents(events: readonly PhaseEvent[]): Phase[] {
  const journaled = events.some((e) => e.kind in JOURNAL_PHASES);
  const phaseOf = (kind: string): PhaseId => {
    if (journaled) return JOURNAL_PHASES[kind] ?? "body";
    return FS_PHASES[kind] ?? "other";
  };
  const byId = new Map<PhaseId, Phase>();
  events.forEach((e, index) => {
    const id = phaseOf(e.kind);
    let phase = byId.get(id);
    if (!phase) byId.set(id, (phase = { id, label: PHASE_LABELS[id], events: [] }));
    phase.events.push({ ...e, index });
  });
  return [...byId.values()];
}
