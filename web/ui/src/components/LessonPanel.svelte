<script lang="ts">
  import { tick } from "svelte";
  import { attrAtSector } from "../core/attribution";
  import { inTextEntry } from "../core/keys";
  import { describeFocus } from "../core/lesson";
  import { scenarios } from "../state/scenarios.svelte";
  import { volume } from "../state/volume.svelte";

  // App.svelte renders this only while a scenario is running, so every read below has a
  // current scenario behind it; the `?.`s are for the type checker, not for a real case.
  const isLast = $derived(!!scenarios.current && scenarios.index === scenarios.current.steps.length - 1);
  const lookAt = $derived(
    describeFocus(scenarios.focus, volume.geometry, (sector) => attrAtSector(volume.attribution, sector).regionName),
  );

  let titleEl = $state<HTMLHeadingElement>();

  // Move focus to the card's title on every step, not just when the lesson opens: the
  // step text is what changed, and a keyboard or screen-reader user needs to land on it
  // rather than hunt for it. `tick()` waits for the new step to be in the DOM first.
  $effect(() => {
    void scenarios.index;
    void tick().then(() => titleEl?.focus());
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

  function onKeydown(e: KeyboardEvent) {
    // A modifier means the chord belongs to the browser or the OS (Cmd-N, Cmd-P), so a
    // scenario step is not ours to advance; the same guard App.svelte's handler uses.
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (!scenarios.current || inTextEntry(e)) return;
    if (e.key === "n") advance();
    else if (e.key === "p") scenarios.prev();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<section class="panel lesson" aria-labelledby="lesson-title">
  <p class="eyebrow">Lesson · step {scenarios.index + 1} of {scenarios.current?.steps.length ?? 0}</p>
  <!-- The title names the scenario, not the step, so it carries the step's own title and
       text as its description: the focus move below then announces the new step in one
       go. A polite live region on the text would race that announcement. -->
  <h2 id="lesson-title" tabindex="-1" aria-describedby="lesson-step-title lesson-step-text" bind:this={titleEl}>{scenarios.current?.title ?? ""}</h2>
  <h3 class="lesson-step-title" id="lesson-step-title">{scenarios.step?.title ?? ""}</h3>
  <p id="lesson-step-text">{scenarios.step?.text ?? ""}</p>
  {#if lookAt}
    <p class="look-at muted">Look at: {lookAt}</p>
  {/if}
  <div class="btn-row">
    <button onclick={() => scenarios.prev()} disabled={scenarios.index <= 0}>Prev</button>
    <button onclick={advance}>{isLast ? "Finish" : "Next"}</button>
    <button onclick={closeLesson}>Close</button>
  </div>
</section>
