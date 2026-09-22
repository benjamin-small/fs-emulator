<script lang="ts">
  import { volume } from "./state/volume.svelte";
  import { buildTree } from "./core/tree";
  const tree = $derived((volume.epoch, buildTree(volume.vol, volume.owners)));
</script>
<main class="panel" style="margin: 16px">
  <h2>FAT explorer</h2>
  <p class="muted">{volume.vol.fsType()} · {volume.geometry.totalSectors} sectors · epoch {volume.epoch}</p>
  <button onclick={() => volume.run((v) => v.createFile(`/FILE${volume.history.length}.TXT`, new TextEncoder().encode("hello")))}>Add file</button>
  <ul class="mono">{#each tree.children as n}<li>{n.name} · {n.size} bytes · cluster {n.firstCluster}</li>{/each}</ul>
  {#if volume.status}<p style="color: var(--diff-ink)">{volume.status.text} <span class="muted">{volume.status.code}</span></p>{/if}
</main>
