<script lang="ts">
  import { onDestroy } from "svelte";
  import { changedSectors } from "../core/patch";
  import { stepAtPointer } from "../core/timelineSlider";
  import { getWorkspace } from "../state/workspace.svelte";

  const ws = getWorkspace();
  const { volume } = ws;

  const PLAY_MS = 700;
  const PLAY_MS_REDUCED = 1500;

  let playing = $state(false);
  let timer: ReturnType<typeof setInterval> | null = null;
  /** The slider's tooltip for the step under the pointer; null until the pointer moves over it. */
  let pointerTitle = $state<string | null>(null);

  const rec = $derived(volume.history[volume.cursor]);
  const steps = $derived(volume.history.length);
  const sectorCount = $derived(rec ? changedSectors(rec.changes, volume.sectorSize).length : 0);
  // The family's noun for a sector: "sector"/"sectors" on FAT, "block"/"blocks" on ext.
  const noun = $derived.by(() => { volume.epoch; return volume.adapter.sector; });
  // The empty state names the mounted disk: "FAT16 · 16 MB · 512-byte sectors · 2 KiB clusters".
  const summary = $derived.by(() => { volume.epoch; return volume.adapter.summary(); });

  /** "3 of 7 · create_file /hello.txt": the slider's tooltip for step `i`. */
  function stepTitle(i: number): string {
    return `${i + 1} of ${steps} · ${volume.history[i].op}`;
  }

  // Every explicit navigation in this bar goes through here: seek, then recenter the dump on
  // what that step changed. There is deliberately no effect on `volume.cursor` doing this —
  // running a command (from Actions or the terminal) moves the cursor too, and an operation
  // must never yank the dump away from what someone was reading. It highlights its changes in
  // place instead; the dump moves only for a step someone picked here, for the `[`/`]`
  // shortcuts, or for a click on the ribbon, the tree, or the What changed panel.
  function goTo(step: number) {
    ws.goToStep(step);
  }

  function reducedMotion(): boolean {
    return typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  function stop() {
    playing = false;
    if (timer !== null) { clearInterval(timer); timer = null; }
  }

  function tick() {
    if (volume.atLatest) { stop(); return; }
    goTo(volume.cursor + 1);
    if (volume.atLatest) stop();
  }

  function play() {
    if (volume.atLatest) return;
    playing = true;
    timer = setInterval(tick, reducedMotion() ? PLAY_MS_REDUCED : PLAY_MS);
  }

  function togglePlay() {
    if (playing) stop(); else play();
  }

  onDestroy(stop);
  // Playback belongs to the tab on screen: hiding the tab stops it, so a hidden timeline never
  // steps on (and never moves its dump) behind another tab.
  $effect(() => {
    if (!ws.active) stop();
  });

  function onSlide(e: Event) {
    goTo(Number((e.currentTarget as HTMLInputElement).value));
  }

  /** Name the step under the pointer in the slider's tooltip, so a scrub can aim before it moves. */
  function onPointer(e: PointerEvent) {
    if (!steps) return;
    const rect = (e.currentTarget as HTMLInputElement).getBoundingClientRect();
    pointerTitle = stepTitle(stepAtPointer(e.clientX - rect.left, rect.width, steps));
  }
</script>

<!-- One row under the top bar: the timeline's controls, then the step in one line. What the
     step wrote is the What changed panel's, at the top of the right column. -->
<section class="panel opbar" aria-labelledby="opbar-heading-{ws.id}">
  <h2 id="opbar-heading-{ws.id}" class="sr-only">Operation</h2>
  <div class="opbar-controls">
    <button onclick={() => goTo(volume.cursor - 1)} disabled={volume.cursor <= 0}>Prev</button>
    <button onclick={togglePlay} disabled={!playing && volume.atLatest}>{playing ? "Pause" : "Play"}</button>
    <button onclick={() => goTo(volume.cursor + 1)} disabled={volume.atLatest}>Next</button>
  </div>
  <input
    class="tl-range"
    type="range"
    min="0"
    max={Math.max(0, steps - 1)}
    value={Math.max(0, volume.cursor)}
    disabled={steps === 0}
    list="opbar-ticks-{ws.id}"
    oninput={onSlide}
    onpointermove={onPointer}
    onpointerleave={() => (pointerTitle = null)}
    title={pointerTitle ?? (rec ? stepTitle(volume.cursor) : undefined)}
    aria-label="Timeline step"
  />
  <datalist id="opbar-ticks-{ws.id}">
    {#each volume.history as _, i (i)}<option value={i}></option>{/each}
  </datalist>
  <p class="opbar-summary">
    {#if !rec}
      <span class="mono">{summary}</span> · Run an action to see what it changes.
    {:else}
      <span class="counter">{volume.cursor + 1} of {steps}</span>
      · <span class="mono op">{rec.op}</span>
      {#if sectorCount}· <span>{sectorCount} {sectorCount === 1 ? noun.singular : noun.plural}</span>{/if}
      {#if rec.events.length}· <span>{rec.events.length} {rec.events.length === 1 ? "event" : "events"}</span>{/if}
    {/if}
  </p>
</section>
