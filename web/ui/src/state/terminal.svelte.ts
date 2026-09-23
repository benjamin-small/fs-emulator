import { clampHeight, parseStoredHeight, TERM_DEFAULT_PX } from "../core/terminalHeight";

const HEIGHT_KEY = "fs-explorer.terminal.height";

export type TerminalReady = "idle" | "loading" | "ready" | "error";

/**
 * Drawer state. DOM-free on purpose: focus moves and the browser-terminal instance
 * live in TerminalPanel.svelte, so this module stays importable anywhere.
 */
export class TerminalStore {
  open = $state(false);
  /** The height the user chose, by dragging the bar or from a previous session. Clamping
   *  happens in `height`, never here, so a temporarily short window cannot ratchet the
   *  choice down: widen the window again and the drawer returns to this height. */
  desired = $state(TERM_DEFAULT_PX);
  /** Viewport height the rendered height is clamped against; `syncViewport()` refreshes it. */
  viewport = $state(viewportHeight());
  /** Rendered drawer height in px; App.svelte feeds it to `.app` as `--term-h`. */
  height = $derived(clampHeight(this.desired, this.viewport));
  /** Bumped by openAndFocus(); TerminalPanel refocuses the shell whenever it changes. */
  focusNonce = $state(0);
  ready = $state<TerminalReady>("idle");
  error = $state<string | null>(null);

  constructor() {
    this.desired = parseStoredHeight(readStoredHeight());
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

  /** Follow a drag. Clamped to what the window allows right now, since the gesture starts
   *  from the rendered height; nothing is written to storage until the pointer comes up. */
  setHeight(px: number) {
    this.desired = clampHeight(px, this.viewport);
  }

  /** End of a drag: remember the chosen height across reloads. The only writer of storage,
   *  so a resize — which is not a choice — never overwrites what the user picked. */
  commitHeight() {
    try {
      localStorage.setItem(HEIGHT_KEY, String(this.desired));
    } catch {
      // Private mode or a full quota: the height simply does not persist.
    }
  }

  /** The window changed size: re-derive the rendered height only. */
  syncViewport() {
    this.viewport = viewportHeight();
  }
}

function viewportHeight(): number {
  return typeof window === "undefined" ? 800 : window.innerHeight;
}

function readStoredHeight(): string | null {
  try {
    if (typeof localStorage === "undefined") return null;
    return localStorage.getItem(HEIGHT_KEY);
  } catch {
    return null;
  }
}

export const terminal = new TerminalStore();
