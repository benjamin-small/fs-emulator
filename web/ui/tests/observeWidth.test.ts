import { afterEach, describe, expect, it, vi } from "vitest";
import { observeWidth } from "../src/core/observeWidth";

/** The observer the action creates, so a test can report a resize to it. */
let observer: { fire(width: number): void; disconnected: boolean } | null = null;

class FakeResizeObserver {
  disconnected = false;
  private readonly cb: (entries: { contentRect: { width: number } }[]) => void;
  constructor(cb: (entries: { contentRect: { width: number } }[]) => void) {
    this.cb = cb;
    observer = this;
  }
  fire(width: number) { this.cb([{ contentRect: { width } }]); }
  observe() {}
  disconnect() { this.disconnected = true; }
}

function element(width: number): HTMLElement {
  return { getBoundingClientRect: () => ({ width }) } as unknown as HTMLElement;
}

describe("observeWidth", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    observer = null;
  });

  it("reports the width now and the content width on every resize", () => {
    vi.stubGlobal("ResizeObserver", FakeResizeObserver);
    const seen: number[] = [];
    const action = observeWidth(element(220), (w) => seen.push(w));
    observer!.fire(180);
    expect(seen).toEqual([220, 180]);
    action.destroy?.();
    expect(observer!.disconnected).toBe(true);
  });

  it("ignores a 0 width, so a map in a hidden tab keeps its last width", () => {
    vi.stubGlobal("ResizeObserver", FakeResizeObserver);
    const seen: number[] = [];
    observeWidth(element(0), (w) => seen.push(w)); // mounted inside a hidden tab
    observer!.fire(220);
    observer!.fire(0); // its tab was hidden: display: none
    observer!.fire(220);
    expect(seen).toEqual([220, 220]);
  });
});
