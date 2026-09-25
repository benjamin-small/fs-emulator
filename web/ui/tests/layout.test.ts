import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { COLOR_JOURNAL, COLOR_TABLE_ALT } from "../src/core/palette";

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

  it("caps the Step strip's height and scrolls it, so a long step cannot push the grid down", () => {
    // An ext3 Add file lists 18 block buttons and 30 events; uncapped, the strip grew to 571 px in a
    // 760 px window. 210 px is about six rows of buttons, more than the FAT actions fill.
    const step = rule(".step");
    expect(step).toContain("max-height: 210px");
    expect(step).toContain("overflow: auto");
  });

  it("gives every palette colour, the journal's included, a dump row stripe and tint", () => {
    // HexRow classes a row `own-${colorIndex}`; an index with no rule here draws a row with
    // neither its left stripe nor its tint, so the rules run up to the last index, COLOR_JOURNAL.
    expect(COLOR_JOURNAL).toBeGreaterThan(COLOR_TABLE_ALT);
    for (let i = 1; i <= COLOR_JOURNAL; i++) {
      expect(rule(`.row.own-${i}`), `.row.own-${i}`).toContain(`border-left-color: var(--own-${i});`);
      expect(css).toContain(`.row.own-${i} .b { background: color-mix(in srgb, var(--own-${i}) 14%, transparent); }`);
    }
  });

  it("scrolls the block-group map inside its own box, never sideways", () => {
    // The canvas is sized from the last measured width, so between a resize and the observer's
    // next paint it can be wider than the column; hidden overflow keeps that from showing a
    // horizontal scrollbar, and the vertical scroll is what keeps a 256 MB disk's bands to 260 px.
    const wrap = rule(".bgmap-wrap");
    expect(wrap).toContain("overflow-y: auto");
    expect(wrap).toContain("overflow-x: hidden");
    // An inline canvas sits on the text baseline, leaving a strip under the last band that makes
    // the box scroll a few pixels even when the bands fit.
    expect(rule(".bgmap-wrap canvas")).toContain("display: block");
  });

  it("lets the Format details' inputs and selects fill the Actions column", () => {
    // The ext form's number inputs and the Filesystem select would otherwise keep their intrinsic
    // widths: ragged against the other fields, and able to overflow the 220 px left column, which
    // then scrolls sideways.
    expect(rule(".actions .field input, .actions .field textarea, .actions .field select")).toContain("width: 100%");
  });
});
