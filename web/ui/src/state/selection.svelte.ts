import { volume } from "./volume.svelte";

export class SelectionStore {
  path = $state<string | null>(null);
  cursorOffset = $state<number | null>(null);
  hoverOffset = $state<number | null>(null);
  stringsOn = $state(false);
  showRemnants = $state(false);
  expandedGaps = $state(new Set<number>());
  scrollTarget = $state<{ offset: number; nonce: number } | null>(null);

  /** Move the byte cursor and ask the dump to scroll there.
   *  Reads `scrollTarget` before writing it; callers inside an `$effect` must wrap
   *  the call in `untrack()` so the effect does not depend on its own write. */
  jumpTo(offset: number) { this.cursorOffset = offset; this.scrollTarget = { offset, nonce: (this.scrollTarget?.nonce ?? 0) + 1 }; }

  select(path: string | null) {
    this.path = path;
    // The old error described the action that failed, not this selection.
    volume.status = null;
  }

  expandGap(startSector: number) { const s = new Set(this.expandedGaps); s.add(startSector); this.expandedGaps = s; }

  /** Drop everything tied to the bytes of a particular disk (after a format or load). */
  reset() {
    this.path = null;
    this.cursorOffset = null;
    this.hoverOffset = null;
    this.expandedGaps = new Set();
    this.scrollTarget = null;
  }
}

export const selection = new SelectionStore();
