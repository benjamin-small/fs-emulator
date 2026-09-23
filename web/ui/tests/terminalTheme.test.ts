import { describe, expect, it } from "vitest";
import { THEME_TOKENS, themeFromTokens } from "../src/core/terminalTheme";

/** A stand-in for `getComputedStyle(document.documentElement)`: only what the helper reads. */
const style = (values: Record<string, string>) => ({
  getPropertyValue: (name: string) => values[name] ?? "",
});

const light = {
  "--panel": "#ffffff",
  "--ink": "#111827",
  "--focus": "#378add",
  "--font-mono": '"IBM Plex Mono", ui-monospace, monospace',
};

describe("themeFromTokens", () => {
  it("maps the four tokens onto xterm's theme and font", () => {
    const { theme, fontFamily } = themeFromTokens(style(light));
    expect(theme.background).toBe("#ffffff");
    expect(theme.foreground).toBe("#111827");
    expect(theme.cursor).toBe("#378add");
    // The block cursor paints `cursor` under `cursorAccent`, so the accent is the panel
    // colour: the character under the cursor reads as reversed video, not as focus-on-ink.
    expect(theme.cursorAccent).toBe("#ffffff");
    expect(fontFamily).toBe('"IBM Plex Mono", ui-monospace, monospace');
  });

  it("tints the selection from --focus rather than using the flat cursor colour", () => {
    const { theme } = themeFromTokens(style(light));
    // Selection sits behind unchanged foreground text, so it must stay translucent.
    expect(theme.selectionBackground).toBe("#378add59");
  });

  it("follows the dark tokens with no other change", () => {
    const { theme } = themeFromTokens(
      style({ ...light, "--panel": "#1b2027", "--ink": "#e6eaf0", "--focus": "#85b7eb" }),
    );
    expect(theme).toEqual({
      background: "#1b2027",
      foreground: "#e6eaf0",
      cursor: "#85b7eb",
      cursorAccent: "#1b2027",
      selectionBackground: "#85b7eb59",
    });
  });

  it("trims the whitespace getPropertyValue keeps around a token's value", () => {
    const { theme, fontFamily } = themeFromTokens(
      style({ "--panel": " #fff ", "--ink": "  #000", "--focus": "#abc ", "--font-mono": " Menlo, monospace " }),
    );
    expect(theme.background).toBe("#fff");
    expect(theme.foreground).toBe("#000");
    expect(theme.cursor).toBe("#abc");
    expect(fontFamily).toBe("Menlo, monospace");
  });

  it("omits every key whose token is empty, so xterm keeps its own default", () => {
    const { theme, fontFamily } = themeFromTokens(style({}));
    expect(theme).toEqual({});
    expect(fontFamily).toBeUndefined();
    // Absent, not `undefined`: xterm reads `'background' in theme` nowhere, but an explicit
    // `undefined` would still overwrite a colour set by an earlier `setTheme`.
    expect("background" in theme).toBe(false);
  });

  it("drops only the missing half when --focus is empty but the rest is set", () => {
    const { theme } = themeFromTokens(style({ ...light, "--focus": "" }));
    expect(theme).toEqual({ background: "#ffffff", foreground: "#111827" });
  });

  it("keeps the cursor but omits the selection when --focus is not 6-digit hex", () => {
    // `#abc`, `rgb(...)`, `color-mix(...)` and a named colour are all valid CSS that xterm
    // would choke on with `59` glued to the end; the cursor still takes them as they are.
    for (const focus of ["#abc", "rgb(55, 138, 221)", "color-mix(in srgb, blue 50%, white)", "dodgerblue", "#378addff"]) {
      const { theme } = themeFromTokens(style({ ...light, "--focus": focus }));
      expect(theme.cursor, focus).toBe(focus);
      expect(theme.cursorAccent, focus).toBe("#ffffff");
      expect("selectionBackground" in theme, focus).toBe(false);
    }
  });

  it("accepts uppercase 6-digit hex for the selection", () => {
    const { theme } = themeFromTokens(style({ ...light, "--focus": "#378ADD" }));
    expect(theme.selectionBackground).toBe("#378ADD59");
  });

  it("names the tokens it reads, and reads exactly those", () => {
    expect(THEME_TOKENS).toEqual(["--panel", "--ink", "--focus", "--font-mono"]);
    const asked: string[] = [];
    themeFromTokens({ getPropertyValue: (name) => { asked.push(name); return ""; } });
    expect(asked).toEqual([...THEME_TOKENS]);
  });
});
