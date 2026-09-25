<script lang="ts">
  import { changedSectors } from "../core/patch";
  import { getWorkspace } from "../state/workspace.svelte";

  const ws = getWorkspace();
  const { volume, selection } = ws;

  const rec = $derived(volume.history[volume.cursor]);
  const sectors = $derived(rec ? changedSectors(rec.changes, volume.sectorSize) : []);
  // The buttons name a sector in the family's noun: "Sector 1" on FAT, "Block 1" on ext.
  const noun = $derived.by(() => { volume.epoch; const s = volume.adapter.sector.singular; return s[0].toUpperCase() + s.slice(1); });
</script>

<!-- A strip under the top bar: heading, counter, op, sector buttons, and events flow left
     to right and wrap, so the current step reads as one line on a wide window. -->
<section class="panel step" aria-labelledby="step-heading-{ws.id}">
  <h2 id="step-heading-{ws.id}">Step</h2>
  {#if !rec}
    <p class="muted">Run an action to see what it changes.</p>
  {:else}
    <p class="counter">{volume.cursor + 1} of {volume.history.length}</p>
    <p class="mono op">{rec.op}</p>
    {#if sectors.length}
      <div class="btn-row">
        {#each sectors as sector (sector)}
          <button onclick={() => selection.jumpTo(sector * volume.sectorSize)}>{noun} {sector}</button>
        {/each}
      </div>
    {/if}
    {#if rec.events.length}
      <ul class="events">
        {#each rec.events as ev, i (i)}
          <li>
            {#if ev.region}
              {@const start = ev.region.start}
              <button class="link" onclick={() => selection.jumpTo(start)}><code>{ev.kind}</code> {ev.text}</button>
            {:else}
              <code>{ev.kind}</code> {ev.text}
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>
