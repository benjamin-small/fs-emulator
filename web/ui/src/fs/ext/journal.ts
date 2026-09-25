import type { JournalInfo, OpRecord, Volume } from "../../lib/wasm";
import { CRASH_PHASES, type CrashPhase, type JournalCapability, type JournalRingBlock, type JournalState } from "../adapter";

/**
 * The ext3 journal behind the seam's `JournalCapability`. `state()` answers from `info`, which
 * the adapter points at its cached `JournalInfo` (refreshed with it); `blocks()` reads the
 * volume every call, so callers memoise it per `volume.epoch`. The phase strings are wasm's
 * `armCrash` phases, which are already the seam's `CrashPhase`, so they pass through unchanged.
 * `recover()` returns the volume's op for the caller to run through the timeline.
 */
export class ExtJournal implements JournalCapability {
  private readonly vol: Volume;
  private readonly info: () => JournalInfo;

  constructor(vol: Volume, info: () => JournalInfo) {
    this.vol = vol;
    this.info = info;
  }

  state(): JournalState {
    const { mode, sequence, head, start, maxlen, firstBlock, maxTransaction, needsRecovery } = this.info();
    return { mode, sequence, head, start, maxlen, firstBlock, maxTransaction, needsRecovery };
  }

  blocks(): JournalRingBlock[] {
    return this.vol.journalBlocks();
  }

  arm(phase: CrashPhase): void {
    this.vol.armCrash(phase);
  }

  disarm(): void {
    this.vol.disarmCrash();
  }

  phase(): CrashPhase | null {
    const p = this.vol.crashPhase();
    return CRASH_PHASES.find((c) => c === p) ?? null;
  }

  recover(): OpRecord {
    return this.vol.recover();
  }
}
