import type { ITheme } from "@benjamin-small/browser-terminal";

/**
 * The design tokens the terminal follows. Exported so a test can assert the list rather
 * than re-spelling it, and so a rename in `src/styles/tokens.css` has one place to land.
 */
export const THEME_TOKENS = ["--panel", "--ink", "--focus", "--font-mono"] as const;

/**
 * Selection alpha, appended to `--focus` as an `#rrggbbaa` suffix. 0x59 is 35%: light
 * enough that unchanged foreground text stays readable through it, dark enough to see.
 * xterm parses 8-digit hex, and the shorthand `--focus` values in tokens.css are 6-digit.
 */
const SELECTION_ALPHA = "59";

/**
 * browser-terminal 0.3.0 styles its panes from `CreateOptions.terminal` and `setTheme`,
 * so the explorer hands it the same tokens the rest of the UI uses instead of overriding
 * xterm's DOM from `app.css`. Pure over a `CSSStyleDeclaration`-shaped object: the caller
 * passes `getComputedStyle(document.documentElement)`, a test passes a fake.
 *
 * An empty token (the property is not set, or the call happens before the stylesheet has
 * applied) leaves its key off the theme entirely, so xterm keeps its own default; a later
 * `setTheme` with the token present still wins, which an explicit `undefined` would not
 * guarantee across xterm versions.
 */
export function themeFromTokens(style: { getPropertyValue(name: string): string }): { theme: ITheme; fontFamily: string | undefined } {
  // getPropertyValue keeps the whitespace after the `:` in the declaration.
  const token = (name: string): string => style.getPropertyValue(name).trim();
  const panel = token("--panel");
  const ink = token("--ink");
  const focus = token("--focus");
  const fontFamily = token("--font-mono");

  const theme: ITheme = {};
  if (panel) theme.background = panel;
  if (ink) theme.foreground = ink;
  if (focus) {
    theme.cursor = focus;
    // The block cursor fills its cell with `cursor` and draws the character in
    // `cursorAccent`, so the accent is the panel colour: reversed video, never
    // focus-blue text on a focus-blue block.
    if (panel) theme.cursorAccent = panel;
    theme.selectionBackground = `${focus}${SELECTION_ALPHA}`;
  }
  return { theme, fontFamily: fontFamily === "" ? undefined : fontFamily };
}
