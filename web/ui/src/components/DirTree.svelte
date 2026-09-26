<script lang="ts">
  import { buildTree } from "../core/tree";
  import { getWorkspace } from "../state/workspace.svelte";
  import TreeNodeView from "./TreeNodeView.svelte";

  const { volume, selection } = getWorkspace();

  const tree = $derived((volume.epoch, buildTree(volume.vol, volume.adapter.owners)));
</script>

<section class="panel dirtree">
  <h2>Files</h2>
  {#if volume.corruption}<p class="muted stale-note">{volume.adapter.corruptNote}</p>{/if}
  {#if !volume.atLatest}<p class="muted stale-note">Shows the latest state, not the step you are viewing.</p>{/if}
  <!-- Only a family with "deleted things still on disk" has `remnants`; without it there is
       nothing for the toggle to show. -->
  {#if volume.adapter.remnants}
    <label class="remnants">
      <input type="checkbox" bind:checked={selection.showRemnants} />
      Show remnants
      {#if !volume.atLatest}<span class="muted">(latest state only)</span>{/if}
    </label>
  {/if}
  <div class="tree-root">
    <TreeNodeView node={tree} depth={0} />
  </div>
</section>
