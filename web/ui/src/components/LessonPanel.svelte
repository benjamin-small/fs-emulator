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

  function advance() {
    if (isLast) scenarios.stop();
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
  <h2 id="lesson-title" tabindex="-1" bind:this={titleEl}>{scenarios.current?.title ?? ""}</h2>
  <h3 class="lesson-step-title">{scenarios.step?.title ?? ""}</h3>
  <p aria-live="polite">{scenarios.step?.text ?? ""}</p>
  {#if lookAt}
    <p class="look-at muted">Look at: {lookAt}</p>
  {/if}
  <div class="btn-row">
    <button onclick={() => scenarios.prev()} disabled={scenarios.index <= 0}>Prev</button>
    <button onclick={advance}>{isLast ? "Finish" : "Next"}</button>
    <button onclick={() => scenarios.stop()}>Close</button>
  </div>
</section>
