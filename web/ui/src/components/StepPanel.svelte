<script lang="ts">
  import { volume } from "../state/volume.svelte";
  import { selection } from "../state/selection.svelte";
  import { changedSectors } from "../core/patch";

  const rec = $derived(volume.history[volume.cursor]);
  const sectors = $derived(rec ? changedSectors(rec.changes, volume.sectorSize) : []);
</script>

<section class="panel step">
  <h2>Step</h2>
  {#if !rec}
    <p class="muted">Run an action to see what it changes.</p>
  {:else}
    <p>Step {volume.cursor + 1} of {volume.history.length}</p>
    <p class="mono">{rec.op}</p>
    {#if sectors.length}
      <div class="btn-row">
        {#each sectors as sector (sector)}
          <button onclick={() => selection.jumpTo(sector * volume.sectorSize)}>Sector {sector}</button>
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
