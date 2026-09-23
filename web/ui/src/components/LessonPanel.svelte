<script lang="ts">
  import { tick } from "svelte";
  import { attrAtSector } from "../core/attribution";
  import { clampPosition, defaultPosition, nudge, type Point } from "../core/floating";
  import { inTextEntry } from "../core/keys";
  import { describeFocus } from "../core/lesson";
  import { lessonWindow } from "../state/lessonWindow.svelte";
  import { scenarios } from "../state/scenarios.svelte";
  import { volume } from "../state/volume.svelte";

  /** Below this viewport width the card docks to the bottom of the screen instead of
   *  floating: there is no room to drag it anywhere useful. Same breakpoint as the
   *  columns collapsing in app.css. */
  const DOCK_MAX_WIDTH = 760;

  // App.svelte renders this only while a scenario is running, so every read below has a
  // current scenario behind it; the `?.`s are for the type checker, not for a real case.
  const isLast = $derived(!!scenarios.current && scenarios.index === scenarios.current.steps.length - 1);
  const lookAt = $derived(
    describeFocus(scenarios.focus, volume.geometry, (sector) => attrAtSector(volume.attribution, sector).regionName),
  );

  let titleEl = $state<HTMLHeadingElement>();
  let cardEl = $state<HTMLElement>();

  // Move focus to the card's title on every step, not just when the lesson opens: the
  // step text is what changed, and a keyboard or screen-reader user needs to land on it
  // rather than hunt for it. `tick()` waits for the new step to be in the DOM first.
  // Minimized, the title is hidden, so the dialog itself takes focus: its name is the
  // scenario title and its description the step's title and text, which aria still reads
  // from the hidden elements, so the new step is announced either way.
  $effect(() => {
    void scenarios.index;
    void tick().then(() => (lessonWindow.minimized ? cardEl : titleEl)?.focus());
  });

  /** Finish and Close remove this card while focus is on the button that removed it, which
   *  would drop focus to the document body. Hand it to the picker's Start button instead —
   *  the pattern TerminalPanel.close uses for `#terminal-toggle`. The button is in the
   *  topbar and outlives the card, so focusing it synchronously is safe. */
  function closeLesson() {
    scenarios.stop();
    document.getElementById("scenario-start")?.focus();
  }

  function advance() {
    if (isLast) closeLesson();
    else scenarios.next();
  }

  function onWindowKeydown(e: KeyboardEvent) {
    // A modifier means the chord belongs to the browser or the OS (Cmd-N, Cmd-P), so a
    // scenario step is not ours to advance; the same guard App.svelte's handler uses.
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (!scenarios.current || inTextEntry(e)) return;
    if (e.key === "n") advance();
    else if (e.key === "p") scenarios.prev();
  }

  // Escape inside the card closes it, the usual contract for a non-modal dialog. Keys
  // typed elsewhere on the page never reach this handler.
  function onCardKeydown(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    e.preventDefault();
    closeLesson();
  }

  // --- Placement -------------------------------------------------------------------
  // The card is `position: fixed`. Until the user moves it, it sits where the card used
  // to live, at the top of the right column; after that the store remembers the spot.
  // Both are clamped to the viewport on every resize so the bar stays reachable.
  let viewport = $state(readViewport());
  const docked = $derived(viewport.w <= DOCK_MAX_WIDTH);
  let fallback = $state<Point | null>(null);

  function readViewport() {
    return { w: window.innerWidth, h: window.innerHeight };
  }
  function cardSize() {
    return cardEl ? { w: cardEl.offsetWidth, h: cardEl.offsetHeight } : { w: 340, h: 240 };
  }
  function onResize() {
    viewport = readViewport();
    if (lessonWindow.pos) lessonWindow.setPos(clampPosition(lessonWindow.pos, cardSize(), viewport));
  }

  // The default spot is measured from the layout (the grid's top edge), after the card has
  // its size; re-measured whenever the viewport changes and the user has not moved it yet.
  $effect(() => {
    void viewport;
    if (lessonWindow.pos || docked) return;
    void tick().then(() => {
      if (lessonWindow.pos) return;
      const gridTop = document.querySelector(".grid")?.getBoundingClientRect().top ?? 96;
      fallback = defaultPosition(cardSize(), viewport, Math.round(gridTop));
    });
  });

  const placed = $derived(lessonWindow.pos ?? fallback);

  // Expanding a card parked near the bottom edge can push it off screen: re-clamp once the
  // new size is in the DOM. (Minimizing only shrinks it, which never needs a move.)
  $effect(() => {
    void lessonWindow.minimized;
    void tick().then(() => {
      if (lessonWindow.pos) lessonWindow.setPos(clampPosition(lessonWindow.pos, cardSize(), viewport));
    });
  });

  // --- Dragging by the bar ------------------------------------------------------------
  let drag: { pointerId: number; startX: number; startY: number; origin: Point } | null = null;
  let dragging = $state(false);

  function onBarDown(e: PointerEvent) {
    if (docked || e.button !== 0) return;
    // The bar's other buttons (minimize) click rather than drag; the handle does both.
    const target = e.target as HTMLElement;
    if (target.closest("button") && !target.closest(".handle")) return;
    const origin = placed ?? clampPosition({ x: 0, y: 0 }, cardSize(), viewport);
    drag = { pointerId: e.pointerId, startX: e.clientX, startY: e.clientY, origin };
    dragging = true;
    capture(e);
    e.preventDefault();
  }
  function onBarMove(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    const next = { x: drag.origin.x + (e.clientX - drag.startX), y: drag.origin.y + (e.clientY - drag.startY) };
    lessonWindow.setPos(clampPosition(next, cardSize(), viewport));
  }
  function onBarUp(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    drag = null;
    dragging = false;
    lessonWindow.commit();
    release(e);
  }

  /** Pointer capture keeps a drag alive when the pointer leaves the bar. Best effort: a
   *  pointer the browser no longer tracks throws, and the drag still works while the
   *  pointer stays over the bar (the same rule as TerminalPanel's handles). */
  function capture(e: PointerEvent) {
    try {
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    } catch {
      // No active pointer with that id: nothing to capture.
    }
  }
  function release(e: PointerEvent) {
    try {
      (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
    } catch {
      // Already released by the browser.
    }
  }

  /** The handle's keyboard equivalent of a drag: arrow keys move the card, Shift for
   *  bigger steps. Each press is committed, so there is no separate "drop". */
  function onHandleKeydown(e: KeyboardEvent) {
    if (docked) return;
    const from = placed ?? clampPosition({ x: 0, y: 0 }, cardSize(), viewport);
    const to = nudge(from, e.key, e.shiftKey);
    if (!to) return;
    e.preventDefault();
    lessonWindow.setPos(clampPosition(to, cardSize(), viewport));
    lessonWindow.commit();
  }
</script>

<svelte:window onkeydown={onWindowKeydown} onresize={onResize} />

<!-- A non-modal dialog: nothing behind it is covered, dimmed, or made inert, and it can be
     dragged by its bar (or moved with the arrow keys on the handle) to wherever it is out
     of the way of the bytes being talked about. -->
<div
  class="panel lesson"
  class:docked
  class:dragging
  role="dialog"
  aria-labelledby="lesson-title"
  tabindex="-1"
  bind:this={cardEl}
  onkeydown={onCardKeydown}
  style:left={!docked && placed ? `${placed.x}px` : null}
  style:top={!docked && placed ? `${placed.y}px` : null}
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="lesson-bar" onpointerdown={onBarDown} onpointermove={onBarMove} onpointerup={onBarUp} onpointercancel={onBarUp}>
    <p class="eyebrow">Lesson · step {scenarios.index + 1} of {scenarios.current?.steps.length ?? 0}</p>
    <!-- Minimized, the card is its bar and its buttons: enough to step through a lesson
         while the text stays out of the way of the bytes it points at. -->
    <button
      type="button"
      class="minimize"
      aria-expanded={!lessonWindow.minimized}
      aria-controls="lesson-body"
      aria-label={lessonWindow.minimized ? "Show the step text" : "Hide the step text"}
      title={lessonWindow.minimized ? "Show the step text" : "Hide the step text"}
      onclick={() => lessonWindow.toggleMinimized()}
    >{lessonWindow.minimized ? "+" : "−"}</button>
    {#if !docked}
      <button type="button" class="handle" aria-label="Move the lesson card (arrow keys; Shift for bigger steps)" title="Drag to move, or use the arrow keys" onkeydown={onHandleKeydown}>⋮⋮</button>
    {/if}
  </div>
  <div id="lesson-body" hidden={lessonWindow.minimized}>
    <!-- The title names the scenario, not the step, so it carries the step's own title and
         text as its description: the focus move above then announces the new step in one
         go. A polite live region on the text would race that announcement. -->
    <h2 id="lesson-title" tabindex="-1" aria-describedby="lesson-step-title lesson-step-text" bind:this={titleEl}>{scenarios.current?.title ?? ""}</h2>
    <h3 class="lesson-step-title" id="lesson-step-title">{scenarios.step?.title ?? ""}</h3>
    <p id="lesson-step-text">{scenarios.step?.text ?? ""}</p>
    {#if lookAt}
      <p class="look-at muted">Look at: {lookAt}</p>
    {/if}
  </div>
  <div class="btn-row" class:compact={lessonWindow.minimized}>
    <button onclick={() => scenarios.prev()} disabled={scenarios.index <= 0}>Prev</button>
    <button onclick={advance}>{isLast ? "Finish" : "Next"}</button>
    <button onclick={closeLesson}>Close</button>
  </div>
</div>
