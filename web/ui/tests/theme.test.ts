import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { applyTheme, DEFAULT_THEME, otherTheme, parseStoredTheme, THEME_KEY } from "../src/core/theme";

describe("parseStoredTheme", () => {
  it("defaults to dark when nothing is remembered", () => {
    expect(DEFAULT_THEME).toBe("dark");
    expect(parseStoredTheme(null)).toBe("dark");
  });

  it("only the exact remembered 'light' opts out of the default", () => {
    expect(parseStoredTheme("light")).toBe("light");
    for (const raw of ["", " light", "Light", "dark", "auto", "system"]) expect(parseStoredTheme(raw)).toBe("dark");
  });
});

describe("otherTheme", () => {
  it("flips between the two themes", () => {
    expect(otherTheme("dark")).toBe("light");
    expect(otherTheme("light")).toBe("dark");
  });
});

describe("applyTheme", () => {
  it("writes data-theme on the root for both themes", () => {
    const root = { dataset: {} as Record<string, string | undefined> };
    applyTheme(root, "light");
    expect(root.dataset.theme).toBe("light");
    applyTheme(root, "dark");
    expect(root.dataset.theme).toBe("dark");
  });
});

// The store applies the theme at runtime, but two files outside the module graph have to
// agree with it: the inline script in index.html (the pre-paint restore) and the tokens.
describe("theme wiring outside the module graph", () => {
  const here = new URL(".", import.meta.url);

  it("index.html restores a remembered light theme under the same key before first paint", () => {
    const html = readFileSync(new URL("../index.html", here), "utf8");
    expect(html).toContain(`localStorage.getItem("${THEME_KEY}")`);
    expect(html).toContain('dataset.theme = "light"');
  });

  it("the tokens key on data-theme with dark as the root default, not on the OS preference", () => {
    const css = readFileSync(new URL("../src/styles/tokens.css", here), "utf8");
    expect(css).not.toContain("prefers-color-scheme");
    expect(css).toMatch(/:root \{[^}]*color-scheme: dark;/);
    expect(css).toMatch(/:root\[data-theme="light"\] \{[^}]*color-scheme: light;/);
  });
});
