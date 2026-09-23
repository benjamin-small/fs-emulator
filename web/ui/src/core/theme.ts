/** The explorer's two colour themes. Dark is the default; light is the opt-in that the
 *  switch at the right of the top bar remembers. */
export type Theme = "dark" | "light";

export const DEFAULT_THEME: Theme = "dark";

/**
 * localStorage key for the remembered theme. `index.html` reads the same key in an inline
 * script so a remembered light theme is on the root before the first paint; the two
 * spellings must stay identical (tests/theme.test.ts checks the HTML for this one).
 */
export const THEME_KEY = "fs-explorer.theme";

/**
 * Parse a persisted theme. Only the exact string "light" opts out of the default: absent,
 * empty, or hand-edited values fall back to dark rather than to whatever the OS prefers,
 * because the explorer no longer follows `prefers-color-scheme` at all.
 */
export function parseStoredTheme(raw: string | null): Theme {
  return raw === "light" ? "light" : DEFAULT_THEME;
}

export function otherTheme(theme: Theme): Theme {
  return theme === "dark" ? "light" : "dark";
}

/** Put the theme on the document root, where `tokens.css` keys on `[data-theme]`. Dark
 *  gets the attribute too, so the root always says which theme is live. */
export function applyTheme(root: { dataset: Record<string, string | undefined> }, theme: Theme): void {
  root.dataset.theme = theme;
}
