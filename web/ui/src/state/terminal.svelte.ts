import { clampHeight, parseStoredHeight, TERM_DEFAULT_PX } from "../core/terminalHeight";
import { clampWidth, parseStoredWidth, placementFor, TERM_W_DEFAULT_PX } from "../core/terminalPlacement";

const HEIGHT_KEY = "fs-explorer.terminal.height";
const WIDTH_KEY = "fs-explorer.terminal.width";

export type TerminalReady = "idle" | "loading" | "ready" | "error";

/**
 * Terminal state. DOM-free on purpose: focus moves and the browser-terminal instance
 * live in TerminalPanel.svelte, so this module stays importable anywhere.
 *
 * The terminal is a drawer along the bottom on narrow viewports and a column down the
 * right side on wide ones (`placement`); each has its own size, chosen by dragging and
 * remembered separately, so switching windows never overwrites the other.
 */
export class TerminalStore {
  open = $state(false);
  /** The height the user chose, by dragging the bar or from a previous session. Clamping
   *  happens in `height`, never here, so a temporarily short window cannot ratchet the
   *  choice down: widen the window again and the drawer returns to this height. */
  desired = $state(TERM_DEFAULT_PX);
  /** The column width the user chose, by dragging its edge or from a previous session;
   *  the same unclamped rule as `desired`. */
  desiredWidth = $state(TERM_W_DEFAULT_PX);
  /** Viewport size the rendered sizes are clamped against; `syncViewport()` refreshes it. */
  viewport = $state(viewportHeight());
  viewportWidth = $state(viewportWidth());
  /** Rendered drawer height in px; App.svelte feeds it to `.app` as `--term-h`. */
  height = $derived(clampHeight(this.desired, this.viewport));
  /** Rendered column width in px; App.svelte feeds it to `.app` as `--term-w`. */
  width = $derived(clampWidth(this.desiredWidth, this.viewportWidth));
  /** Side column or bottom drawer, from the viewport width alone. */
  placement = $derived(placementFor(this.viewportWidth));
  /** Bumped by openAndFocus(); TerminalPanel refocuses the shell whenever it changes. */
  focusNonce = $state(0);
  ready = $state<TerminalReady>("idle");
  error = $state<string | null>(null);

  constructor() {
    this.desired = parseStoredHeight(readStored(HEIGHT_KEY));
    this.desiredWidth = parseStoredWidth(readStored(WIDTH_KEY));
  }

  toggle() {
    if (this.open) this.close();
    else this.openAndFocus();
  }

  openAndFocus() {
    this.open = true;
    this.focusNonce++;
  }

  close() {
    this.open = false;
  }

  /** Follow a drag of the drawer bar. Clamped to what the window allows right now, since
   *  the gesture starts from the rendered height; nothing is written to storage until the
   *  pointer comes up. */
  setHeight(px: number) {
    this.desired = clampHeight(px, this.viewport);
  }

  /** End of a drag: remember the chosen height across reloads. The only writer of that
   *  entry, so a resize — which is not a choice — never overwrites what the user picked. */
  commitHeight() {
    persist(HEIGHT_KEY, this.desired);
  }

  /** Follow a drag of the column's edge; the width counterpart of `setHeight`. */
  setWidth(px: number) {
    this.desiredWidth = clampWidth(px, this.viewportWidth);
  }

  /** End of an edge drag: remember the chosen width across reloads. */
  commitWidth() {
    persist(WIDTH_KEY, this.desiredWidth);
  }

  /** The window changed size: re-derive the rendered sizes and the placement only. */
  syncViewport() {
    this.viewport = viewportHeight();
    this.viewportWidth = viewportWidth();
  }
}

function viewportHeight(): number {
  return typeof window === "undefined" ? 800 : window.innerHeight;
}

function viewportWidth(): number {
  return typeof window === "undefined" ? 1280 : window.innerWidth;
}

function readStored(key: string): string | null {
  try {
    if (typeof localStorage === "undefined") return null;
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function persist(key: string, px: number) {
  try {
    localStorage.setItem(key, String(px));
  } catch {
    // Private mode or a full quota: the size simply does not persist.
  }
}

export const terminal = new TerminalStore();
