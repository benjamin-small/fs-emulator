<script lang="ts">
  import { all } from "../scenarios";
  import { scenarios } from "../state/scenarios.svelte";

  let selectedId = $state(all[0]?.id ?? "");

  // The picker only starts a lesson. Everything about the running lesson — the step text,
  // Prev/Next/Close, and the `n`/`p` keys — lives in LessonPanel, a normal panel at the top
  // of the right column: no overlay, nothing dimmed, nothing made inert.
  function startScenario() {
    const s = all.find((s) => s.id === selectedId);
    if (s) scenarios.start(s);
  }
</script>

<div class="scenario-controls">
  <!-- The visible label names the select on its own; an `aria-label` would override it and
       leave the accessible name out of step with the words on screen. -->
  <label class="scenario-label" for="scenario-select">Learning scenarios</label>
  <select id="scenario-select" bind:value={selectedId}>
    {#each all as s (s.id)}<option value={s.id}>{s.title}</option>{/each}
  </select>
  <!-- LessonPanel returns focus here when the card closes, so the id is load-bearing. -->
  <button id="scenario-start" onclick={startScenario}>Start</button>
</div>
