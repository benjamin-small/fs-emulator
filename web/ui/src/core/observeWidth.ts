import type { ActionReturn } from "svelte/action";

/**
 * A Svelte action that reports an element's width now and its content width on every resize,
 * so a canvas map wraps to the column it sits in (a resizable layout, a narrower viewport)
 * rather than to a width read once at mount: `<div use:observeWidth={(w) => (width = w)}>`.
 * A 0 width is never reported: it is what an element in a hidden tab (`display: none`)
 * measures, and the map keeps its last width instead of repainting 1 px wide. The observer
 * goes when the element does.
 */
export function observeWidth(el: HTMLElement, onWidth: (width: number) => void): ActionReturn<(width: number) => void> {
  let report = onWidth;
  const measured = (width: number) => {
    if (width > 0) report(width);
  };
  measured(el.getBoundingClientRect().width);
  const ro = new ResizeObserver((entries) => measured(entries[0].contentRect.width));
  ro.observe(el);
  return {
    update(next) { report = next; },
    destroy() { ro.disconnect(); },
  };
}
