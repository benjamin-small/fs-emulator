import type { OpRecord, Volume } from "../lib/wasm";
import type { CrashPhase, FsAdapter, FsFamilyId } from "../fs/adapter";
import { selection } from "../state/selection.svelte";
import { volume } from "../state/volume.svelte";
import { statusToError, type ShellHost } from "./host";

/**
 * The app's ShellHost. Commands close over the returned object and read live store
 * fields through its getters on every call, so a host created once when the
 * terminal starts never goes stale.
 *
 * `run` and `format` mirror the Actions panel, and `setArmedPhase` the Journal panel:
 * VolumeStore never throws, it reports failure through `status` (and `run` returns null,
 * `setArmedPhase` false). The host turns that status back
 * into a thrown error so the shell prints it in red while StatusLine keeps showing
 * the same code, exactly as after a failed form action.
 */
export function createStoreHost(closeTerminal: () => void, applyPrompt: (prefix: string) => void): ShellHost {
  return {
    get vol(): Volume {
      return volume.vol;
    },
    get adapter(): FsAdapter {
      return volume.adapter;
    },
    get cursor(): number {
      return volume.cursor;
    },
    get historyLength(): number {
      return volume.history.length;
    },
    run(fn: (v: Volume) => OpRecord): OpRecord {
      const rec = volume.run(fn);
      if (rec) return rec;
      throw statusToError(volume.status);
    },
    format(family: FsFamilyId, options?: unknown): void {
      volume.format(family, options);
      // On failure VolumeStore.format sets status and leaves the disk alone. Read it
      // before select(null), whose side effect is to clear status.
      if (volume.status) throw statusToError(volume.status);
      selection.select(null);
    },
    setArmedPhase(phase: CrashPhase | null): void {
      if (!volume.setArmedPhase(phase)) throw statusToError(volume.status);
    },
    select(path: string | null): void {
      selection.select(path);
    },
    jumpTo(offset: number): void {
      selection.jumpTo(offset);
    },
    setPrompt(prefix: string): void {
      applyPrompt(prefix);
    },
    closeTerminal,
  };
}
