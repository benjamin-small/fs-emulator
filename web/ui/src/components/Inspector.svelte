<script lang="ts">
  import { volume } from "../state/volume.svelte";
  import { selection } from "../state/selection.svelte";
  import { attrAtOffset } from "../core/attribution";
  import type { Annotation } from "../lib/wasm";
  import { describeRange, formatRange, formatValue } from "../core/annotationFormat";

  const offset = $derived(selection.hoverOffset ?? selection.cursorOffset);
  const attr = $derived(offset === null ? null : attrAtOffset(volume.attribution, offset));
  const memo = new Map<string, Annotation[]>();
  const annotations = $derived.by(() => {
    if (attr === null) return [];
    const key = `${attr.sector}:${volume.epoch}`;
    let a = memo.get(key);
    if (!a) { if (memo.size > 64) memo.clear(); a = volume.adapter.annotateSector(attr.sector); memo.set(key, a); }
    return a;
  });
  const inSector = $derived(offset === null ? -1 : offset % volume.sectorSize);
  const hex = (n: number) => "0x" + n.toString(16);
  const cap = (s: string) => s[0].toUpperCase() + s.slice(1);

  // What the family's table says about the unit under the cursor ("FAT: end of chain"), or
  // null when it has nothing to say. `describeUnit` and `trace` are adapter methods over
  // plain caches, so both deriveds read `volume.epoch` first (the rule in fs/adapter.ts).
  const unitNote = $derived.by(() => {
    volume.epoch;
    return attr === null || attr.unit === undefined ? null : volume.adapter.describeUnit(attr.unit);
  });

  // The places a selected file lives on disk, in the family's words, each one a jump
  // target unless its offset is null (FAT: its directory entry, its FAT chain, and the
  // first cluster of its data).
  const trace = $derived.by(() => {
    volume.epoch;
    return selection.path ? volume.adapter.trace(selection.path) : [];
  });
</script>
<section class="panel inspector">
  <h2>At this byte</h2>
  {#if offset === null || attr === null}
    <p class="muted">Hover or click a byte in the dump.</p>
  {:else}
    <dl class="facts">
      <dt>Offset</dt><dd class="mono">{hex(offset)} · {offset.toLocaleString()}</dd>
      <dt>Sector</dt><dd class="mono">{attr.sector} · {attr.regionName}</dd>
      {#if attr.unit !== undefined}<dt>{cap(volume.adapter.unit.singular)}</dt><dd class="mono">{attr.unit}{#if unitNote} · {unitNote}{/if}</dd>{/if}
      {#if attr.ownerPath}<dt>Owner</dt><dd><button class="link" onclick={() => selection.select(attr.ownerPath!)}>{attr.ownerPath}</button></dd>{:else if attr.regionKind === "data"}<dt>Owner</dt><dd class="muted">free</dd>{/if}
    </dl>
    {#if !volume.atLatest}<p class="muted">Annotations describe the latest state, not the step you are viewing.</p>{/if}
    <ul class="annotations">
      <!-- Offsets and integer values in hex, like the dump; the decimal is the tooltip. -->
      {#each annotations as a}
        {@const v = formatValue(a.value)}
        <li class:hit={inSector >= a.range.start && inSector < a.range.end}>
          <span class="muted" title={describeRange(a.range)}>{formatRange(a.range, volume.sectorSize)}</span>
          {a.label}: <span title={v.title}>{v.text}</span>
        </li>
      {/each}
    </ul>
  {/if}
  {#if selection.path}
    <div class="selected-file">
      <h3>Selected file</h3>
      <p class="mono path">{selection.path}</p>
      <ul class="trace">
        {#each trace as row}
          {@const off = row.offset}
          {#if off !== null}
            <li><button class="link" onclick={() => selection.jumpTo(off)}>{row.label}</button></li>
          {:else}
            <li class="muted">{row.label}</li>
          {/if}
        {/each}
      </ul>
      <button onclick={() => selection.select(null)}>Clear selection</button>
    </div>
  {/if}
</section>
