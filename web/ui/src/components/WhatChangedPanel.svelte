<script lang="ts">
  import { groupEvents, type PhaseId } from "../core/eventPhases";
  import type { Interval } from "../core/intervals";
  import { changedSectors } from "../core/patch";
  import { formatRanges } from "../core/ranges";
  import { getWorkspace } from "../state/workspace.svelte";

  const { volume, selection, layers } = getWorkspace();

  const rec = $derived(volume.history[volume.cursor]);
  const ranges = $derived(rec ? formatRanges(changedSectors(rec.changes, volume.sectorSize)) : []);
  const phases = $derived(rec ? groupEvents(rec.events) : []);
  // The family's noun for a sector: "Sectors" on FAT, "Blocks" on ext.
  const noun = $derived.by(() => { volume.epoch; return volume.adapter.sector; });
  const cap = (s: string) => s[0].toUpperCase() + s.slice(1);

  /** The phases someone opened or closed, by id, kept while stepping: a phase not in here is open
   *  when it is the step's first and closed otherwise. */
  let toggled = $state<Partial<Record<PhaseId, boolean>>>({});
  const isOpen = (id: PhaseId, i: number) => toggled[id] ?? i === 0;

  /** Record a toggle someone made. Setting `open` from `isOpen` fires `toggle` too, but then the
   *  element already agrees with `isOpen` and nothing is recorded. */
  function onToggle(e: Event, id: PhaseId, i: number) {
    const open = (e.currentTarget as HTMLDetailsElement).open;
    if (open !== isOpen(id, i)) toggled[id] = open;
  }

  /** Outline `iv` on the ribbon and the map while the pointer or focus is on its button. */
  function hover(iv: Interval | null) {
    layers.hover = iv;
  }

  // A button can vanish from under the pointer when the step changes (a `[`/`]` scrub, a new
  // operation) without a `mouseleave`: clear the outline with the step it belonged to.
  $effect(() => {
    rec;
    return () => { layers.hover = null; };
  });
</script>

<section class="panel changed">
  <h2>What changed</h2>
  {#if !rec}
    <p class="muted">Nothing yet.</p>
  {:else}
    {#if ranges.length}
      <p class="muted changed-label">{cap(noun.plural)}</p>
      <div class="ranges">
        {#each ranges as r (r.first)}
          {@const bytes = { start: r.first * volume.sectorSize, end: (r.last + 1) * volume.sectorSize }}
          <button
            class="mono"
            aria-label="{cap(r.first === r.last ? noun.singular : noun.plural)} {r.label}"
            onclick={() => selection.jumpTo(bytes.start)}
            onmouseenter={() => hover(bytes)}
            onmouseleave={() => hover(null)}
            onfocus={() => hover(bytes)}
            onblur={() => hover(null)}
          >{r.label}</button>
        {/each}
      </div>
    {/if}
    {#each phases as phase, i (phase.id)}
      <details class="phase" open={isOpen(phase.id, i)} ontoggle={(e) => onToggle(e, phase.id, i)}>
        <summary class:warn={phase.id === "crash"}>{phase.label} · {phase.events.length} {phase.events.length === 1 ? "event" : "events"}</summary>
        <ol>
          {#each phase.events as ev (ev.index)}
            <li>
              {#if ev.region}
                {@const region = ev.region}
                <button
                  class="link"
                  onclick={() => selection.jumpTo(region.start)}
                  onmouseenter={() => hover(region)}
                  onmouseleave={() => hover(null)}
                  onfocus={() => hover(region)}
                  onblur={() => hover(null)}
                ><code>{ev.kind}</code> {ev.text}</button>
              {:else}
                <code>{ev.kind}</code> {ev.text}
              {/if}
            </li>
          {/each}
        </ol>
      </details>
    {/each}
  {/if}
</section>
