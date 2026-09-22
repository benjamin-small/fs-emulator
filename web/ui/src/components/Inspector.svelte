<script lang="ts">
  import { volume } from "../state/volume.svelte";
  import { selection } from "../state/selection.svelte";
  import { layers } from "../state/layers.svelte";
  import { attrAtOffset, clusterByteRange } from "../core/attribution";
  import type { Annotation, FatEntry } from "../lib/wasm";

  const offset = $derived(selection.hoverOffset ?? selection.cursorOffset);
  const attr = $derived(offset === null ? null : attrAtOffset(volume.attribution, offset));
  const memo = new Map<string, Annotation[]>();
  const annotations = $derived.by(() => {
    if (attr === null) return [];
    const key = `${attr.sector}:${volume.epoch}`;
    let a = memo.get(key);
    if (!a) { if (memo.size > 64) memo.clear(); a = volume.vol.annotateSectorWith(attr.sector, volume.owners); memo.set(key, a); }
    return a;
  });
  const inSector = $derived(offset === null ? -1 : offset % volume.sectorSize);
  const hex = (n: number) => "0x" + n.toString(16);

  // The three places a selected file lives on disk, each one a jump target: its
  // directory entry, its FAT chain, and the first cluster of its data.
  const firstCluster = $derived(layers.chain[0] ?? null);
  const fatEntryOffset = $derived(
    firstCluster === null ? null : volume.geometry.reservedSectors * volume.geometry.bytesPerSector + firstCluster * 2,
  );
  const dataOffset = $derived(firstCluster === null ? null : clusterByteRange(volume.geometry, firstCluster).start);

  function describe(e: FatEntry): string {
    switch (e.kind) {
      case "free": return "free";
      case "next": return `next → ${e.cluster}`;
      case "endOfChain": return "end of chain";
      case "bad": return "bad";
      case "reserved": return "reserved";
    }
  }
</script>
<section class="panel inspector">
  <h2>At this byte</h2>
  {#if offset === null || attr === null}
    <p class="muted">Hover or click a byte in the dump.</p>
  {:else}
    <dl class="facts">
      <dt>Offset</dt><dd class="mono">{hex(offset)} · {offset.toLocaleString()}</dd>
      <dt>Sector</dt><dd class="mono">{attr.sector} · {attr.regionName}</dd>
      {#if attr.cluster !== undefined}<dt>Cluster</dt><dd class="mono">{attr.cluster}{#if volume.fat[attr.cluster]} · FAT: {describe(volume.fat[attr.cluster])}{/if}</dd>{/if}
      {#if attr.ownerPath}<dt>Owner</dt><dd><button class="link" onclick={() => selection.select(attr.ownerPath!)}>{attr.ownerPath}</button></dd>{:else if attr.regionKind === "data"}<dt>Owner</dt><dd class="muted">free</dd>{/if}
    </dl>
    {#if !volume.atLatest}<p class="muted">Annotations describe the latest state, not the step you are viewing.</p>{/if}
    <ul class="annotations">
      {#each annotations as a}
        <li class:hit={inSector >= a.range.start && inSector < a.range.end}><span class="mono muted">{a.range.start}..{a.range.end}</span> {a.label}: <span class="mono">{a.value}</span></li>
      {/each}
    </ul>
  {/if}
  {#if selection.path}
    <div class="selected-file">
      <h3>Selected file</h3>
      <p class="mono path">{selection.path}</p>
      <ul class="trace">
        {#if layers.entry}
          <li><button class="link" onclick={() => selection.jumpTo(layers.entry!.start)}>Directory entry · offset {hex(layers.entry.start)}</button></li>
        {/if}
        {#if firstCluster !== null && fatEntryOffset !== null && dataOffset !== null}
          <li><button class="link" onclick={() => selection.jumpTo(fatEntryOffset!)}>FAT chain · {layers.chain.length} clusters starting at {firstCluster} · FAT entry at {hex(fatEntryOffset)}</button></li>
          <li><button class="link" onclick={() => selection.jumpTo(dataOffset!)}>Data · cluster {firstCluster} at {hex(dataOffset)}</button></li>
        {:else}
          <li class="muted">Data · no data clusters</li>
        {/if}
      </ul>
      <button onclick={() => selection.select(null)}>Clear selection</button>
    </div>
  {/if}
</section>
