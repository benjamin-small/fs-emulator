<script lang="ts">
  import { inTextEntry } from "../core/keys";
  import { all } from "../scenarios";
  import { scenarios } from "../state/scenarios.svelte";

  let selectedId = $state(all[0]?.id ?? "");

  let overlayRect = $state<{ top: number; left: number; width: number } | null>(null);
  let titleEl = $state<HTMLHeadingElement>();

  // The panel lives in the topbar slot but needs to visually sit over the top of the
  // right column, so it tracks that column's on-screen box instead of being a DOM
  // child of it. Re-measures on resize and whenever a scenario starts or ends.
  // While it is open the panels it covers are `inert`, so neither the mouse nor the
  // Tab key reaches controls hidden behind the overlay.
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
    const covered = el ? (Array.from(el.children) as HTMLElement[]) : [];
    if (el) {
      ro.observe(el);
      el.classList.add("scenario-open");
      for (const p of covered) p.inert = true;
    }
    window.addEventListener("resize", measure);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", measure);
      el?.classList.remove("scenario-open");
      for (const p of covered) p.inert = false;
    };
  });

  // Move focus to the overlay's title when a scenario opens (not on every step), so
  // a keyboard or screen-reader user lands in the dialog instead of behind it.
  $effect(() => {
    if (scenarios.current && titleEl) titleEl.focus();
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
    // A modifier means the chord belongs to the browser or the OS (Cmd-N, Cmd-P), so a
    // scenario step is not ours to advance; the same guard App.svelte's handler uses.
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (!scenarios.current || inTextEntry(e)) return;
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
  <!-- A `div`, not a `section`: a section's implicit "region" role cannot be
       overridden with the interactive "dialog" role. -->
  <div
    class="panel scenario-overlay"
    role="dialog"
    aria-labelledby="scenario-title"
    style="top:{overlayRect.top}px; left:{overlayRect.left}px; width:{overlayRect.width}px;"
  >
    <h2 id="scenario-title" tabindex="-1" bind:this={titleEl}>{scenarios.current.title}</h2>
    <p class="muted">Step {scenarios.index + 1} of {scenarios.current.steps.length}</p>
    <h3 class="scenario-step-title">{scenarios.step.title}</h3>
    <p>{scenarios.step.text}</p>
    <div class="btn-row">
      <button onclick={() => scenarios.prev()} disabled={scenarios.index <= 0}>Prev</button>
      <button onclick={advance}>{isLast ? "Finish" : "Next"}</button>
      <button onclick={() => scenarios.stop()}>Close</button>
    </div>
  </div>
{/if}
