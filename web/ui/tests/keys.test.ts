import { describe, expect, it } from "vitest";
import { inTextEntry } from "../src/core/keys";

// vitest runs under node: no DOM, so events and targets are plain objects shaped like
// the pieces inTextEntry reads (composedPath, tagName, isContentEditable, closest).
const ev = (...path: unknown[]) => ({ composedPath: () => path as EventTarget[] });
const el = (tagName: string, extra: Record<string, unknown> = {}) => ({ tagName, closest: () => null, ...extra });

describe("inTextEntry", () => {
  it("is true for form fields", () => {
    expect(inTextEntry(ev(el("INPUT")))).toBe(true);
    expect(inTextEntry(ev(el("TEXTAREA")))).toBe(true);
    expect(inTextEntry(ev(el("SELECT")))).toBe(true);
  });

  it("is true for contenteditable", () => {
    expect(inTextEntry(ev(el("DIV", { isContentEditable: true })))).toBe(true);
  });

  it("is true anywhere inside the terminal drawer", () => {
    const drawer = el("SECTION");
    const inside = el("DIV", { closest: (s: string) => (s === ".terminal-drawer" ? drawer : null) });
    expect(inTextEntry(ev(inside))).toBe(true);
  });

  it("is false for buttons, the body, and an empty path", () => {
    expect(inTextEntry(ev(el("BUTTON")))).toBe(false);
    expect(inTextEntry(ev(el("BODY")))).toBe(false);
    expect(inTextEntry(ev())).toBe(false);
  });

  it("looks at the original target, not a wrapper later in the path", () => {
    expect(inTextEntry(ev(el("TEXTAREA"), el("DIV"), el("BODY")))).toBe(true);
    expect(inTextEntry(ev(el("BUTTON"), el("TEXTAREA")))).toBe(false);
  });

  it("tolerates a target without closest (a text node or the window)", () => {
    expect(inTextEntry(ev({ tagName: undefined }))).toBe(false);
  });
});
