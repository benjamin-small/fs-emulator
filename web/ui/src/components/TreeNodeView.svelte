<script lang="ts">
  import type { TreeNode } from "../core/tree";
  import { clusterByteRange } from "../core/attribution";
  import { selection } from "../state/selection.svelte";
  import { volume } from "../state/volume.svelte";
  import TreeNodeView from "./TreeNodeView.svelte";

  let { node, depth }: { node: TreeNode; depth: number } = $props();
  let open = $state(true);

  const isSelected = $derived(selection.path === node.path);

  function pick() {
    selection.select(node.path);
    if (node.firstCluster >= 2) selection.jumpTo(clusterByteRange(volume.geometry, node.firstCluster).start);
  }
</script>

<div class="tree-node">
  <div class="tree-row" style:padding-left="{depth * 14}px">
    {#if node.isDir}
      <button class="disclosure" onclick={() => (open = !open)} aria-expanded={open} aria-label={open ? "Collapse" : "Expand"}>{open ? "▾" : "▸"}</button>
    {:else}
      <span class="disclosure-spacer"></span>
    {/if}
    <button class="row-btn" role="option" class:selected={isSelected} aria-selected={isSelected} onclick={pick}>
      <span class="name">{node.name}</span>
      <span class="meta mono muted">{node.size.toLocaleString()} B · {node.firstCluster >= 2 ? `first cluster ${node.firstCluster}` : "no data"}</span>
    </button>
  </div>
  {#if node.isDir && open}
    {#each node.children as child (child.path)}
      <TreeNodeView node={child} depth={depth + 1} />
    {/each}
  {/if}
</div>
