<script lang="ts">
  import { getWorkspace } from "../../state/workspace.svelte";
  import { CLUSTER_SIZES, DEFAULTS, SIZES, checkFormat } from "./format";
  import type { Fat16FormatOptions } from "./index";

  const { volume, selection } = getWorkspace();

  // Mirror the mounted default disk (16 MB, 4 sectors per cluster) so opening the
  // form shows the geometry that is already on screen.
  let totalSectors = $state<number>(DEFAULTS.totalSectors);
  let sectorsPerCluster = $state<number>(DEFAULTS.sectorsPerCluster);
  let volumeLabel = $state<string>(DEFAULTS.volumeLabel);

  /** The cluster count these options would produce, by the core's rule, and the FAT16
   *  bound it breaks, if any. */
  const check = $derived(checkFormat({ totalSectors, sectorsPerCluster }));

  function formatDisk() {
    const options: Fat16FormatOptions = { totalSectors, sectorsPerCluster, volumeLabel };
    volume.format("fat16", options);
    selection.select(null);
  }
</script>

<label class="field">
  Size
  <select bind:value={totalSectors}>
    {#each SIZES as s}<option value={s.totalSectors}>{s.label}</option>{/each}
  </select>
</label>
<label class="field">
  Sectors per cluster
  <select bind:value={sectorsPerCluster}>
    {#each CLUSTER_SIZES as n}<option value={n}>{n}</option>{/each}
  </select>
</label>
<p class="cluster-count muted">
  {check.clusters.toLocaleString()} clusters{#if check.problem}{" "}· <span class="warn">{check.problem}</span>{/if}
</p>
<label class="field">
  Volume label
  <input class="mono" type="text" maxlength="11" bind:value={volumeLabel} />
</label>
<button onclick={formatDisk} disabled={check.problem !== null}>Format disk</button>
