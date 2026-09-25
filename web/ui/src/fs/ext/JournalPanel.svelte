<script lang="ts">
  import { cellAt, cellRect, gridCols, gridRows, prepareCanvas } from "../../core/grid";
  import { observeWidth } from "../../core/observeWidth";
  import { selection } from "../../state/selection.svelte";
  import { volume } from "../../state/volume.svelte";
  import { CRASH_PHASES, CRASH_PHASE_LABELS, type CrashPhase } from "../adapter";
  import { CELL, GAP, MAX_HEIGHT } from "./blockMap";
  import { NEEDS_RECOVERY, NO_JOURNAL, armedText, journalFacts, journalHeading, ringCaption, ringFill } from "./journalRing";

  let canvas = $state<HTMLCanvasElement>();
  // The panel's content width, kept current by `observeWidth` on the wrapper so the ring wraps
  // to the sidebar's actual size.
  let width = $state(0);
  let hoverIndex = $state<number | null>(null);
  let phase = $state<CrashPhase>("after_commit");

  // The capability's `state()` answers from the adapter's cache and `blocks()` reads the
  // volume; neither is reactive, so everything that calls them reads `volume.epoch` first
  // (the rule in fs/adapter.ts). `blocks()` runs once per epoch, not once per paint or hover.
  const journal = $derived((volume.epoch, volume.adapter.journal));
  const info = $derived((volume.epoch, journal?.state()));
  const ring = $derived((volume.epoch, journal?.blocks() ?? []));
  // One cell per journal block in reading order, in the block-group map's cells.
  const cols = $derived(gridCols(width, CELL, GAP));
  const height = $derived(gridRows(ring.length, cols) * (CELL + GAP));
  const controlsOff = $derived(!volume.atLatest);

  // A format or load binds a new capability (or none, on ext2) under a canvas the mouse may
  // never have left; drop the old hover so the caption does not describe the old ring.
  $effect(() => {
    journal;
    hoverIndex = null;
  });

  $effect(() => {
    ring; cols; height;
    paint();
  });

  function paint() {
    if (!canvas) return;
    const ctx = prepareCanvas(canvas, Math.max(1, Math.floor(width)), height);
    if (!ctx) return;
    // Read once per paint so a light/dark switch shows on the next repaint.
    const style = getComputedStyle(canvas);
    const colors = new Map<string, string>();
    const colorOf = (token: string) => {
      let c = colors.get(token);
      if (c === undefined) colors.set(token, (c = style.getPropertyValue(token).trim()));
      return c;
    };
    for (const b of ring) {
      const { x, y } = cellRect(b.index, cols, CELL, GAP);
      const { token, alpha } = ringFill(b);
      ctx.globalAlpha = alpha;
      ctx.fillStyle = colorOf(token);
      ctx.fillRect(x, y, CELL, CELL);
    }
    ctx.globalAlpha = 1;
  }

  function onMove(e: MouseEvent) {
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    hoverIndex = cellAt(e.clientX - rect.left, e.clientY - rect.top, cols, ring.length, CELL, GAP);
  }
  function onLeave() { hoverIndex = null; }
  function onClick() {
    const b = hoverIndex === null ? undefined : ring[hoverIndex];
    if (b) selection.jumpTo(b.block * volume.sectorSize);
  }
  const caption = $derived.by(() => {
    const b = hoverIndex === null ? undefined : ring[hoverIndex];
    return b ? ringCaption(b) : "";
  });

  // The armed phase is the store's (`volume.armedPhase`), which the terminal's `crash` moves
  // too, so an arm from either side shows here at once. One button arms and disarms, so keyboard
  // focus stays on it when its label flips.
  function toggleArm() {
    volume.setArmedPhase(volume.armedPhase ? null : phase);
  }
  function recover() {
    const j = journal;
    if (j) volume.run(() => j.recover());
  }
</script>

<section class="panel journal">
  {#if journal && info}
    <h2>{journalHeading(info)}</h2>
    {#if !volume.atLatest}<p class="muted stale-note">Shows the latest state, not the step you are viewing.</p>{/if}
    <dl class="facts">
      {#each journalFacts(info) as [name, value] (name)}<dt>{name}</dt><dd class="mono">{value}</dd>{/each}
    </dl>
    <div class="journal-live" aria-live="polite">
      {#if volume.needsRecovery}<p class="journal-flag">{NEEDS_RECOVERY}</p>{/if}
    </div>
    <div class="journal-wrap" use:observeWidth={(w) => (width = w)} style:max-height="{MAX_HEIGHT}px">
      <canvas bind:this={canvas} onmousemove={onMove} onmouseleave={onLeave} onclick={onClick} aria-label="Journal ring"></canvas>
    </div>
    <p class="mono muted journal-caption">{caption || " "}</p>
    <div class="journal-controls" aria-live="polite">
      {#if volume.armedPhase}
        <span class="journal-armed">{armedText(volume.armedPhase)}</span>
      {:else}
        <select id="journal-phase" aria-label="Crash phase" bind:value={phase} disabled={controlsOff}>
          {#each CRASH_PHASES as p (p)}<option value={p}>{CRASH_PHASE_LABELS[p]}</option>{/each}
        </select>
      {/if}
      <button onclick={toggleArm} disabled={controlsOff}>{volume.armedPhase ? "Disarm" : "Arm"}</button>
      <button onclick={recover} disabled={controlsOff || !volume.needsRecovery}>Recover</button>
    </div>
  {:else}
    <p class="muted journal-none">{NO_JOURNAL}</p>
  {/if}
</section>
