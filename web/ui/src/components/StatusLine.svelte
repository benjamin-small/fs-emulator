<script lang="ts">
  import { volume } from "../state/volume.svelte";
</script>
<div class="status" role="status">
  {#if !volume.atLatest && volume.history.length}
    <span>Viewing step {volume.cursor + 1} of {volume.history.length}</span>
    <button onclick={() => volume.backToNow()}>Back to now</button>
  {/if}
  {#if volume.status}<span class="err">{volume.status.text}{#if volume.status.code} <span class="muted">{volume.status.code}</span>{/if}</span>{/if}
</div>
