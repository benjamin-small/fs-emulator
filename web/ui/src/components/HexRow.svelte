<script lang="ts">
  import type { Attr } from "../core/attribution";
  interface Cell { hex: string; asc: string; cls: string; title: string }
  let { offset, attr, cells, sectorStart, clusterStart, label = "" }: { offset: number; attr: Attr; cells: Cell[]; sectorStart: boolean; clusterStart: boolean; label?: string } = $props();
  const hex8 = $derived(offset.toString(16).padStart(8, "0"));
</script>
<div class="row own-{attr.colorIndex}" class:sector-start={sectorStart} class:cluster-start={clusterStart} data-offset={offset}>
  <span class="off mono">{hex8}</span>
  <span class="hex mono">{#each cells as c, i}<span class={c.cls} data-off={offset + i} title={c.title}>{c.hex}</span>{/each}</span>
  <span class="asc mono">{#each cells as c, i}<span class={c.cls} data-off={offset + i}>{c.asc}</span>{/each}</span>
  {#if label}<span class="lbl muted">{label}</span>{/if}
</div>
