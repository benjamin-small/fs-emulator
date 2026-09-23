import { applyTheme, otherTheme, parseStoredTheme, THEME_KEY, type Theme } from "../core/theme";

/**
 * Colour theme: dark by default, flipped by the switch in the top bar. `set` puts the
 * attribute on the document root synchronously rather than from an effect, so anything
 * that reads the tokens right after a change — the terminal pushing its xterm theme —
 * already sees the new values. The document access is guarded so the module stays
 * importable under node.
 */
export class ThemeStore {
  current = $state<Theme>(parseStoredTheme(readStoredTheme()));

  constructor() {
    // index.html restores a remembered light theme before first paint; this keeps the
    // root in step with the store from the start on a host page without that script.
    if (typeof document !== "undefined") applyTheme(document.documentElement, this.current);
  }

  get isDark(): boolean {
    return this.current === "dark";
  }

  toggle() {
    this.set(otherTheme(this.current));
  }

  set(theme: Theme) {
    this.current = theme;
    if (typeof document !== "undefined") applyTheme(document.documentElement, theme);
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      // Private mode or a full quota: the choice lasts until the next reload.
    }
  }
}

function readStoredTheme(): string | null {
  try {
    if (typeof localStorage === "undefined") return null;
    return localStorage.getItem(THEME_KEY);
  } catch {
    return null;
  }
}

export const theme = new ThemeStore();
