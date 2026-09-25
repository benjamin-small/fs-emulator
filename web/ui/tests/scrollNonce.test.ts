import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { ScrollNonces } from "../src/core/scrollNonce";

describe("ScrollNonces", () => {
  it("numbers every request past the one before it", () => {
    const n = new ScrollNonces();
    expect([n.next(), n.next(), n.next()]).toEqual([1, 2, 3]);
  });

  it("is the selection store's one counter, and reset() leaves it counting", () => {
    // The store runs on runes, which the node tests cannot load, so this reads its source, as
    // layout.test.ts reads app.css. A counter made anew in reset() would restart at 1, the nonce
    // HexView last scrolled to, and the first jump after a lesson start would not scroll.
    const src = readFileSync(new URL("../src/state/selection.svelte.ts", import.meta.url), "utf8");
    expect(src.match(/new ScrollNonces\(\)/g)).toHaveLength(1);
    expect(src).toContain("nonce: this.nonces.next()");
    const at = src.indexOf("  reset() {");
    expect(at, "selection.svelte.ts has no `  reset() {`: update this scan").toBeGreaterThanOrEqual(0);
    const reset = src.slice(at);
    expect(reset.slice(0, reset.indexOf("\n  }\n"))).not.toMatch(/nonces|ScrollNonces/);
  });
});
