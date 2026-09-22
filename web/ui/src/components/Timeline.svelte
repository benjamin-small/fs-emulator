<script lang="ts">
  import { onDestroy } from "svelte";
  import { focusHistoryStep } from "../state/navigate.svelte";
  import { volume } from "../state/volume.svelte";

  const PLAY_MS = 700;
  const PLAY_MS_REDUCED = 1500;
  const LABEL_MAX = 14;

  let playing = $state(false);
  let timer: ReturnType<typeof setInterval> | null = null;

  // Every explicit navigation in this panel goes through here: seek, then recenter the
  // dump on what that step changed. There is deliberately no effect on `volume.cursor`
  // doing this — running a command (from Actions or the terminal) moves the cursor too,
  // and an operation must never yank the dump away from what someone was reading. It
  // highlights its changes in place instead; the dump moves only for a step someone
  // picked here, for the `[`/`]` shortcuts, or for a click on the ribbon or the tree.
  function goTo(step: number) {
    volume.seek(step);
    focusHistoryStep(step);
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

  function onSlide(e: Event) {
    goTo(Number((e.currentTarget as HTMLInputElement).value));
  }

  function truncate(op: string): string {
    return op.length > LABEL_MAX ? op.slice(0, LABEL_MAX - 1) + "…" : op;
  }
</script>

<div class="timeline">
  {#if volume.history.length === 0}
    <p class="muted">No operations yet</p>
  {:else}
    <div class="tl-controls">
      <button onclick={() => goTo(volume.cursor - 1)} disabled={volume.cursor <= 0}>Prev</button>
      <button onclick={togglePlay} disabled={!playing && volume.atLatest}>{playing ? "Pause" : "Play"}</button>
      <button onclick={() => goTo(volume.cursor + 1)} disabled={volume.atLatest}>Next</button>
      <input
        class="tl-range"
        type="range"
        min="0"
        max={volume.history.length - 1}
        value={volume.cursor}
        oninput={onSlide}
        aria-label="Timeline step"
      />
    </div>
    <div class="tl-steps">
      {#each volume.history as rec, i (i)}
        <button
          type="button"
          class="tl-step mono"
          class:current={i === volume.cursor}
          onclick={() => goTo(i)}
          title={rec.op}
        >{truncate(rec.op)}</button>
      {/each}
    </div>
  {/if}
</div>
