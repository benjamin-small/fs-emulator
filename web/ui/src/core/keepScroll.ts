import type { Action } from "svelte/action";

/** What `createScrollKeeper` needs of a scroll box; an `HTMLElement` is one. */
export interface Scrollable {
  scrollTop: number;
  addEventListener(type: "scroll", fn: () => void): void;
  removeEventListener(type: "scroll", fn: () => void): void;
}

/**
 * Keeps a scroll box's position across a hide. A tab's body is hidden with `display: none`,
 * which zeroes every scroll box in it without a scroll event, so the position is recorded on
 * scroll only while the box is shown and put back when it is shown again. The restore waits a
 * frame (`raf`): straight after `hidden` is removed the box has no layout yet and would clamp
 * the position to 0. Scroll events between showing and the restore are ignored for the same
 * reason; a hide before the frame skips the restore.
 */
export function createScrollKeeper(
  el: Scrollable,
  shown: boolean,
  raf: (fn: () => void) => void = (fn) => requestAnimationFrame(fn),
): { update(shown: boolean): void; destroy(): void } {
  let saved = el.scrollTop;
  let visible = shown;
  let restoring = false;
  let destroyed = false;

  const onScroll = () => {
    if (visible && !restoring) saved = el.scrollTop;
  };
  el.addEventListener("scroll", onScroll);

  return {
    update(next: boolean) {
      const wasVisible = visible;
      visible = next;
      if (!next || wasVisible) return;
      restoring = true;
      raf(() => {
        if (destroyed || !visible || !restoring) return;
        restoring = false;
        el.scrollTop = saved;
      });
    },
    destroy() {
      destroyed = true;
      el.removeEventListener("scroll", onScroll);
    },
  };
}

/** `<div use:keepScroll={ws.active}>`: `createScrollKeeper` as a Svelte action. */
export const keepScroll: Action<HTMLElement, boolean> = (el, shown) => createScrollKeeper(el, shown);
