import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Layout rules that exist to stop a regression the unit tests cannot see. Each one names the
// symptom it guards against; the browser check in the commit that added it is the proof.
describe("app.css layout guards", () => {
  const css = readFileSync(new URL("../src/app.css", import.meta.url), "utf8");
  const rule = (selector: string) => {
    const m = css.match(new RegExp(`^${selector.replace(/[.[\]()]/g, (ch) => `\\${ch}`)} \\{([^}]*)\\}`, "m"));
    return m ? m[1] : null;
  };

  it("bounds the app column so children with pixel widths cannot push the page sideways", () => {
    // An implicit `auto` column grows to the max-content of the ribbon canvas and xterm's
    // screen layers, which are sized from their last layout; the window then never shrinks.
    expect(rule(".app")).toContain("grid-template-columns: minmax(0, 1fr)");
  });

  it("gives the side terminal column an explicit width without redefining the main column", () => {
    const side = rule(".app.term-side");
    expect(side).toContain("grid-auto-columns: var(--term-w");
    expect(side).not.toContain("grid-template-columns");
  });
});
