import type { ITheme } from "@benjamin-small/browser-terminal";

/**
 * The design tokens the terminal follows, in the order they are read. The lookups below
 * are driven from this list, so it cannot fall out of step with them, and a rename in
 * `src/styles/tokens.css` has one place to land.
 */
export const THEME_TOKENS = ["--panel", "--ink", "--focus", "--font-mono"] as const;

type ThemeToken = (typeof THEME_TOKENS)[number];

/**
 * Selection alpha, appended to `--focus` as an `#rrggbbaa` suffix. 0x59 is 35%: light
 * enough that unchanged foreground text stays readable through it, dark enough to see.
 */
const SELECTION_ALPHA = "59";

/**
 * The one `--focus` spelling the alpha suffix is valid for. Every other form CSS accepts —
 * `#abc`, `#rrggbbaa`, `rgb(...)`, `color-mix(...)`, a named colour — would become
 * nonsense with `59` glued on, so those keep the cursor and lose only the selection tint.
 */
const SIX_DIGIT_HEX = /^#[0-9a-f]{6}$/i;

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
  const read = {} as Record<ThemeToken, string>;
  for (const name of THEME_TOKENS) read[name] = style.getPropertyValue(name).trim();
  const panel = read["--panel"];
  const ink = read["--ink"];
  const focus = read["--focus"];
  const fontFamily = read["--font-mono"];

  const theme: ITheme = {};
  if (panel) theme.background = panel;
  if (ink) theme.foreground = ink;
  if (focus) {
    theme.cursor = focus;
    // The block cursor fills its cell with `cursor` and draws the character in
    // `cursorAccent`, so the accent is the panel colour: reversed video, never
    // focus-blue text on a focus-blue block.
    if (panel) theme.cursorAccent = panel;
    if (SIX_DIGIT_HEX.test(focus)) theme.selectionBackground = `${focus}${SELECTION_ALPHA}`;
  }
  return { theme, fontFamily: fontFamily === "" ? undefined : fontFamily };
}
