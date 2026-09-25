import { describe, expect, it } from "vitest";
import { createScrollKeeper } from "../src/core/keepScroll";

/** A scroll box: `scrollTop` plus the one event the keeper listens to. `scroll(to)` is a user
 *  scroll (moves and fires); assigning `scrollTop` directly is what `display: none` does. */
function box(top = 0) {
  const listeners = new Set<() => void>();
  return {
    scrollTop: top,
    listeners,
    addEventListener(_type: "scroll", fn: () => void) { listeners.add(fn); },
    removeEventListener(_type: "scroll", fn: () => void) { listeners.delete(fn); },
    scroll(to: number) {
      this.scrollTop = to;
      for (const fn of listeners) fn();
    },
  };
}

/** An injected `requestAnimationFrame`: callbacks wait until `flush()`. */
function frames() {
  const queue: (() => void)[] = [];
  return {
    raf: (fn: () => void) => { queue.push(fn); },
    get pending() { return queue.length; },
    flush() { while (queue.length) queue.shift()!(); },
  };
}

describe("createScrollKeeper", () => {
  it("restores the position a hide zeroed, in the frame after the box is shown again", () => {
    const el = box();
    const f = frames();
    const keeper = createScrollKeeper(el, true, f.raf);
    el.scroll(340);
    keeper.update(false);
    el.scrollTop = 0; // display: none zeroes a scroll box without a scroll event
    keeper.update(true);
    expect(el.scrollTop).toBe(0); // not before layout: the box has no height yet
    f.flush();
    expect(el.scrollTop).toBe(340);
  });

  it("starts from the box's position when it is created", () => {
    const el = box(120);
    const f = frames();
    const keeper = createScrollKeeper(el, true, f.raf);
    keeper.update(false);
    el.scrollTop = 0;
    keeper.update(true);
    f.flush();
    expect(el.scrollTop).toBe(120);
  });

  it("ignores scroll events while hidden and before the restore runs", () => {
    const el = box();
    const f = frames();
    const keeper = createScrollKeeper(el, true, f.raf);
    el.scroll(340);
    keeper.update(false);
    el.scroll(0);
    keeper.update(true);
    el.scroll(0); // a scroll event at the zeroed position, before the frame
    f.flush();
    expect(el.scrollTop).toBe(340);
  });

  it("keeps recording once restored", () => {
    const el = box();
    const f = frames();
    const keeper = createScrollKeeper(el, true, f.raf);
    el.scroll(340);
    keeper.update(false);
    keeper.update(true);
    f.flush();
    el.scroll(500);
    keeper.update(false);
    el.scrollTop = 0;
    keeper.update(true);
    f.flush();
    expect(el.scrollTop).toBe(500);
  });

  it("schedules nothing unless the box goes from hidden to shown", () => {
    const el = box();
    const f = frames();
    const keeper = createScrollKeeper(el, true, f.raf);
    keeper.update(true);
    expect(f.pending).toBe(0);
    keeper.update(false);
    keeper.update(false);
    expect(f.pending).toBe(0);
    keeper.update(true);
    expect(f.pending).toBe(1);
  });

  it("skips the restore when the box is hidden again before the frame", () => {
    const el = box();
    const f = frames();
    const keeper = createScrollKeeper(el, true, f.raf);
    el.scroll(340);
    keeper.update(false);
    el.scrollTop = 0;
    keeper.update(true);
    keeper.update(false);
    f.flush();
    expect(el.scrollTop).toBe(0);
  });

  it("restores a box created hidden to where it was when created", () => {
    const el = box(0);
    const f = frames();
    const keeper = createScrollKeeper(el, false, f.raf);
    el.scroll(90); // ignored: hidden
    keeper.update(true);
    f.flush();
    expect(el.scrollTop).toBe(0);
  });

  it("stops listening and drops a pending restore on destroy", () => {
    const el = box();
    const f = frames();
    const keeper = createScrollKeeper(el, true, f.raf);
    el.scroll(340);
    keeper.update(false);
    el.scrollTop = 0;
    keeper.update(true);
    keeper.destroy();
    expect(el.listeners.size).toBe(0);
    f.flush();
    expect(el.scrollTop).toBe(0);
  });
});
