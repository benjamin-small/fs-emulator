import { ScrollNonces } from "../core/scrollNonce";
import { volume } from "./volume.svelte";

export class SelectionStore {
  path = $state<string | null>(null);
  cursorOffset = $state<number | null>(null);
  hoverOffset = $state<number | null>(null);
  stringsOn = $state(false);
  showRemnants = $state(false);
  expandedGaps = $state(new Set<number>());
  scrollTarget = $state<{ offset: number; nonce: number } | null>(null);
  // One counter for the store's life: `reset()` leaves it running, because the dump remembers
  // the last nonce it scrolled to across a reset (see `ScrollNonces`).
  private readonly nonces = new ScrollNonces();

  /** Move the byte cursor and ask the dump to scroll there, even to the offset it is at: every
   *  request carries a fresh nonce. Reads no state, so an `$effect` may call it. */
  jumpTo(offset: number) { this.cursorOffset = offset; this.scrollTarget = { offset, nonce: this.nonces.next() }; }

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
