<script module lang="ts">
  import { FAMILY_IDS } from "../fs";
  import type { FsFamilyId } from "../fs/adapter";
  import { defaultLessonFor, lessonsFor } from "../scenarios";

  /** Each tab's picked lesson, seeded with its fundamentals. Module state because App rebuilds
   *  the picker over the active workspace on every switch: coming back to a tab restores its
   *  choice. */
  const picked = $state<Record<FsFamilyId, string>>(
    Object.fromEntries(FAMILY_IDS.map((id) => [id, defaultLessonFor(id).id])) as Record<FsFamilyId, string>,
  );
</script>

<script lang="ts">
  import { getWorkspace } from "../state/workspace.svelte";

  const ws = getWorkspace();
  const { volume, scenarios } = ws;

  // The tab's own lessons, flat, in registry order; the first is its fundamentals.
  const lessons = lessonsFor(ws.id);

  // The picker only starts a lesson. Everything about the running lesson — the step text,
  // Prev/Next/Close, and the `n`/`p` keys — lives in LessonPanel, the floating Lesson card.
  function startScenario() {
    const s = lessons.find((s) => s.id === picked[ws.id]);
    if (!s) return;
    // The list holds the tab's lessons only, so the runner's refusal of another family's
    // lesson cannot fire from here; it still lands on the status line if it ever does.
    try {
      scenarios.start(s);
    } catch (e) {
      volume.report(e);
    }
  }
</script>

<div class="scenario-controls">
  <!-- The visible label names the select on its own; an `aria-label` would override it and
       leave the accessible name out of step with the words on screen. -->
  <label class="scenario-label" for="scenario-select">Learning scenarios</label>
  <select id="scenario-select" bind:value={picked[ws.id]}>
    {#each lessons as s (s.id)}<option value={s.id}>{s.title}</option>{/each}
  </select>
  <!-- LessonPanel returns focus here when the card closes, so the id is load-bearing. -->
  <button id="scenario-start" onclick={startScenario}>Start</button>
</div>
