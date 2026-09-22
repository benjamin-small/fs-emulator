import { clampHeight, TERM_DEFAULT_PX } from "../core/terminalHeight";

const HEIGHT_KEY = "fs-explorer.terminal.height";

export type TerminalReady = "idle" | "loading" | "ready" | "error";

/**
 * Drawer state. DOM-free on purpose: focus moves and the browser-terminal instance
 * live in TerminalPanel.svelte, so this module stays importable anywhere.
 */
export class TerminalStore {
  open = $state(false);
  /** Drawer height in px, persisted; App.svelte feeds it to `.app` as `--term-h`. */
  height = $state(TERM_DEFAULT_PX);
  /** Bumped by openAndFocus(); TerminalPanel refocuses the shell whenever it changes. */
  focusNonce = $state(0);
  ready = $state<TerminalReady>("idle");
  error = $state<string | null>(null);

  constructor() {
    this.height = clampHeight(readStoredHeight(), viewportHeight());
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

  /** Clamp to the current viewport and remember the result across reloads. */
  setHeight(px: number) {
    this.height = clampHeight(px, viewportHeight());
    try {
      localStorage.setItem(HEIGHT_KEY, String(this.height));
    } catch {
      // Private mode or a full quota: the height simply does not persist.
    }
  }
}

function viewportHeight(): number {
  return typeof window === "undefined" ? 800 : window.innerHeight;
}

function readStoredHeight(): number {
  try {
    if (typeof localStorage === "undefined") return TERM_DEFAULT_PX;
    return Number(localStorage.getItem(HEIGHT_KEY) ?? TERM_DEFAULT_PX);
  } catch {
    return TERM_DEFAULT_PX;
  }
}

export const terminal = new TerminalStore();
