import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { applyTheme, DEFAULT_THEME, otherTheme, parseStoredTheme, THEME_KEY } from "../src/core/theme";
import { COLOR_FILE_BASE, COLOR_JOURNAL, COLOR_TABLE, COLOR_TABLE_ALT, FILE_HUE_COUNT } from "../src/core/palette";

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

  it("the journal amber keeps 25° of hue or 20 % of lightness from every file hue and table violet, in both themes", () => {
    const css = readFileSync(new URL("../src/styles/tokens.css", here), "utf8");
    const own = (block: string | undefined) =>
      Object.fromEntries([...(block ?? "").matchAll(/--own-(\d+): (#[0-9a-f]{6})/g)].map((m) => [Number(m[1]), m[2]]));
    const dark: Record<number, string> = own(css.match(/:root \{([^}]*)\}/)?.[1]);
    const light: Record<number, string> = { ...dark, ...own(css.match(/:root\[data-theme="light"\] \{([^}]*)\}/)?.[1]) };
    /** Hue in degrees and HSL lightness in percent of a `#rrggbb` colour. */
    const hsl = (hex: string) => {
      const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
      const max = Math.max(r, g, b), min = Math.min(r, g, b), d = max - min;
      const h = d === 0 ? 0 : max === r ? ((g - b) / d + 6) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
      return { hue: h * 60, lightness: ((max + min) / 2) * 100 };
    };
    const others = [COLOR_TABLE, COLOR_TABLE_ALT, ...Array.from({ length: FILE_HUE_COUNT }, (_, i) => COLOR_FILE_BASE + i)];
    expect([dark[COLOR_JOURNAL], light[COLOR_JOURNAL]]).toEqual(["#d4a017", "#b7791f"]);
    for (const [theme, colours] of [["dark", dark], ["light", light]] as const) {
      const amber = hsl(colours[COLOR_JOURNAL]);
      for (const i of others) {
        const other = hsl(colours[i]);
        const apart = Math.abs(amber.hue - other.hue);
        const hue = Math.min(apart, 360 - apart), lightness = Math.abs(amber.lightness - other.lightness);
        expect(hue >= 25 || lightness >= 20, `${theme}: --own-${COLOR_JOURNAL} against --own-${i} (${hue.toFixed(1)}°, ${lightness.toFixed(1)} %)`).toBe(true);
      }
    }
  });
});
