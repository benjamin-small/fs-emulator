<script lang="ts">
  import { all } from "../scenarios";
  import { scenarios } from "../state/scenarios.svelte";

  let selectedId = $state(all[0]?.id ?? "");

  let overlayRect = $state<{ top: number; left: number; width: number } | null>(null);

  // The panel lives in the topbar slot but needs to visually sit over the top of the
  // right column, so it tracks that column's on-screen box instead of being a DOM
  // child of it. Re-measures on resize and whenever a scenario starts or ends.
  $effect(() => {
    if (!scenarios.current) { overlayRect = null; return; }
    const el = document.querySelector<HTMLElement>(".col.right");
    function measure() {
      if (!el) return;
      const r = el.getBoundingClientRect();
      overlayRect = { top: r.top, left: r.left, width: r.width };
    }
    measure();
    const ro = new ResizeObserver(measure);
    if (el) ro.observe(el);
    window.addEventListener("resize", measure);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", measure);
    };
  });

  const isLast = $derived(!!scenarios.current && scenarios.index === scenarios.current.steps.length - 1);

  function startScenario() {
    const s = all.find((s) => s.id === selectedId);
    if (s) scenarios.start(s);
  }

  function advance() {
    if (isLast) scenarios.stop();
    else scenarios.next();
  }

  function onKeydown(e: KeyboardEvent) {
    if (!scenarios.current) return;
    const tag = (e.target as HTMLElement | null)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
    if (e.key === "n") advance();
    else if (e.key === "p") scenarios.prev();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="scenario-controls">
  <select bind:value={selectedId} aria-label="Scenario">
    {#each all as s (s.id)}<option value={s.id}>{s.title}</option>{/each}
  </select>
  <button onclick={startScenario}>Start</button>
</div>

{#if scenarios.current && scenarios.step && overlayRect}
  <section
    class="panel scenario-overlay"
    style="top:{overlayRect.top}px; left:{overlayRect.left}px; width:{overlayRect.width}px;"
  >
    <h2>{scenarios.current.title}</h2>
    <p class="muted">Step {scenarios.index + 1} of {scenarios.current.steps.length}</p>
    <h3 class="scenario-step-title">{scenarios.step.title}</h3>
    <p>{scenarios.step.text}</p>
    <div class="btn-row">
      <button onclick={() => scenarios.prev()} disabled={scenarios.index <= 0}>Prev</button>
      <button onclick={advance}>{isLast ? "Finish" : "Next"}</button>
      <button onclick={() => scenarios.stop()}>Close</button>
    </div>
  </section>
{/if}
