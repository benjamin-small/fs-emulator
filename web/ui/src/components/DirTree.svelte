<script lang="ts">
  import { buildTree } from "../core/tree";
  import { selection } from "../state/selection.svelte";
  import { volume } from "../state/volume.svelte";
  import TreeNodeView from "./TreeNodeView.svelte";

  const tree = $derived((volume.epoch, buildTree(volume.vol, volume.owners)));
</script>

<section class="panel dirtree">
  <h2>Files</h2>
  <label class="remnants">
    <input type="checkbox" bind:checked={selection.showRemnants} />
    Show remnants
  </label>
  <div class="tree-root">
    <TreeNodeView node={tree} depth={0} />
  </div>
</section>
