<script lang="ts">
  import { buildTree } from "../core/tree";
  import { selection } from "../state/selection.svelte";
  import { volume } from "../state/volume.svelte";
  import TreeNodeView from "./TreeNodeView.svelte";

  const tree = $derived((volume.epoch, buildTree(volume.vol, volume.owners)));
</script>

<section class="panel dirtree">
  <h2>Files</h2>
  {#if volume.corruption}<p class="muted stale-note">Boot sector does not parse; the tree is unavailable until a raw write repairs it.</p>{/if}
  {#if !volume.atLatest}<p class="muted stale-note">Shows the latest state, not the step you are viewing.</p>{/if}
  <label class="remnants">
    <input type="checkbox" bind:checked={selection.showRemnants} />
    Show remnants
    {#if !volume.atLatest}<span class="muted">(latest state only)</span>{/if}
  </label>
  <div class="tree-root">
    <TreeNodeView node={tree} depth={0} />
  </div>
</section>
