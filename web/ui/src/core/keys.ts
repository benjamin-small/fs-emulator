/** The pieces of an event target the shortcut guard reads. Structural rather than
 *  `HTMLElement` so vitest (node, no DOM) can pass plain objects. */
export interface EntryTarget {
  tagName?: string;
  isContentEditable?: boolean;
  closest?: (selector: string) => unknown;
}

/** The one method of `Event` the guard needs. */
export interface PathEvent {
  composedPath(): EventTarget[];
}

/**
 * True when a keydown started somewhere that consumes typing: an input, textarea,
 * select, contenteditable, or anywhere inside the terminal drawer. Reads
 * `composedPath()[0]`, the original target, so a `window` listener sees the real
 * element the key was typed into.
 *
 * Why the drawer clause: xterm cancels ordinary keydowns, so typing in the terminal
 * never reaches `window`, but the Ctrl-B prefix chord and the key after it bubble
 * with the xterm helper textarea as target. Without this, Ctrl-B `[` would scrub the
 * timeline and Ctrl-B `n` would advance a scenario.
 */
export function inTextEntry(e: PathEvent): boolean {
  const t = e.composedPath()[0] as EntryTarget | undefined;
  if (!t) return false;
  const tag = t.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
  if (t.isContentEditable === true) return true;
  return typeof t.closest === "function" && t.closest(".terminal-drawer") != null;
}
