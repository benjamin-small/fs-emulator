<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import { volume } from "../state/volume.svelte";
  import { selection } from "../state/selection.svelte";

  const PLAY_MS = 700;
  const PLAY_MS_REDUCED = 1500;
  const LABEL_MAX = 14;

  let playing = $state(false);
  let timer: ReturnType<typeof setInterval> | null = null;

  // Recenter the dump on whatever the step changed, wherever the cursor moved from
  // (slider drag, Prev/Next, Play, or the `[`/`]` shortcuts in App.svelte). `jumpTo`
  // reads `selection.scrollTarget` to bump its nonce, so calling it untracked keeps
  // this effect's dependencies limited to `volume.cursor`/`history` — otherwise the
  // effect would also depend on the field it writes and retrigger itself forever.
  $effect(() => {
    const rec = volume.history[volume.cursor];
    if (rec && rec.changes.length) untrack(() => selection.jumpTo(rec.changes[0].offset));
  });

  function reducedMotion(): boolean {
    return typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  function stop() {
    playing = false;
    if (timer !== null) { clearInterval(timer); timer = null; }
  }

  function tick() {
    if (volume.atLatest) { stop(); return; }
    volume.seek(volume.cursor + 1);
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
    volume.seek(Number((e.currentTarget as HTMLInputElement).value));
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
      <button onclick={() => volume.seek(volume.cursor - 1)} disabled={volume.cursor <= 0}>Prev</button>
      <button onclick={togglePlay} disabled={!playing && volume.atLatest}>{playing ? "Pause" : "Play"}</button>
      <button onclick={() => volume.seek(volume.cursor + 1)} disabled={volume.atLatest}>Next</button>
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
          onclick={() => volume.seek(i)}
          title={rec.op}
        >{truncate(rec.op)}</button>
      {/each}
    </div>
  {/if}
</div>
