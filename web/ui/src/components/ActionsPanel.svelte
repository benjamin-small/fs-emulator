<script lang="ts">
  import { volume } from "../state/volume.svelte";
  import { selection } from "../state/selection.svelte";
  import type { FormatOptions } from "../lib/wasm";

  const SIZES: { label: string; totalSectors: number }[] = [
    { label: "4 MB", totalSectors: 8192 },
    { label: "16 MB", totalSectors: 32768 },
    { label: "64 MB", totalSectors: 131072 },
  ];
  const CLUSTER_SIZES = [1, 2, 4, 8];

  // FAT16 geometry constants, matching the core's formatter.
  const BYTES_PER_SECTOR = 512, ROOT_ENTRIES = 512, DIR_ENTRY = 32, RESERVED = 1, FAT_COPIES = 2;
  const FAT16_MIN_CLUSTERS = 4085, FAT16_MAX_CLUSTERS = 65524;

  let path = $state("/Hello world.txt");
  let content = $state("Hello from the browser");
  let bytes = $state<Uint8Array | null>(null);
  let fileName = $state<string | null>(null);
  let bytesInput = $state<HTMLInputElement>();

  // Mirror the mounted default disk (16 MB, 4 sectors per cluster) so opening the
  // form shows the geometry that is already on screen.
  let totalSectors = $state(32768);
  let sectorsPerCluster = $state(4);
  let volumeLabel = $state("");

  /** The cluster count these options would produce, by the core's rule: the smallest
   *  sectors-per-FAT that can index every cluster the leftover space yields. */
  function clusterCountFor(total: number, spc: number): number {
    const rootDirSectors = Math.ceil((ROOT_ENTRIES * DIR_ENTRY) / BYTES_PER_SECTOR);
    const entriesPerFatSector = BYTES_PER_SECTOR / 2; // FAT16 entries are 2 bytes
    for (let spf = 1; spf <= total; spf++) {
      const usable = total - RESERVED - FAT_COPIES * spf - rootDirSectors;
      if (usable <= 0) return 0;
      const clusters = Math.floor(usable / spc);
      if (spf * entriesPerFatSector >= clusters + 2) return clusters;
    }
    return 0;
  }

  const clusters = $derived(clusterCountFor(totalSectors, sectorsPerCluster));
  const clusterProblem = $derived(
    clusters < FAT16_MIN_CLUSTERS ? "too few for FAT16" : clusters > FAT16_MAX_CLUSTERS ? "too many for FAT16" : "",
  );

  function data(): Uint8Array {
    return bytes ?? new TextEncoder().encode(content);
  }

  async function onPickBytes(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0] ?? null;
    if (!file) { bytes = null; fileName = null; return; }
    bytes = new Uint8Array(await file.arrayBuffer());
    fileName = file.name;
  }

  /** Go back to the textarea's text as the file's content. */
  function clearBytes() {
    bytes = null;
    fileName = null;
    if (bytesInput) bytesInput.value = "";
  }

  function addFile() {
    if (volume.run((v) => v.createFile(path, data()))) selection.select(path);
  }
  function overwrite() {
    if (volume.run((v) => v.writeFile(path, data()))) selection.select(path);
  }
  function deleteFile() {
    if (volume.run((v) => v.deleteFile(path))) selection.select(null);
  }
  function newFolder() {
    if (volume.run((v) => v.createDir(path))) selection.select(path);
  }
  function removeFolder() {
    if (volume.run((v) => v.removeDir(path))) selection.select(null);
  }

  function formatDisk() {
    const options: FormatOptions = { totalSectors, sectorsPerCluster, volumeLabel };
    volume.format(options);
    selection.select(null);
  }

  async function onLoadImage(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0] ?? null;
    if (!file) return;
    try {
      volume.load(new Uint8Array(await file.arrayBuffer()));
    } catch (err) {
      volume.status = { text: err instanceof Error ? err.message : String(err) };
    }
  }

  function exportImage() {
    const blob = new Blob([new Uint8Array(volume.export())], { type: "application/octet-stream" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "volume.img";
    a.click();
    URL.revokeObjectURL(url);
  }
</script>

<section class="panel actions">
  <h2>Actions</h2>
  {#if !volume.atLatest}<p class="muted">Return to the latest step to make changes.</p>{/if}
  <fieldset disabled={!volume.atLatest}>
    <label class="field">
      Path
      <input id="action-path" class="mono" type="text" bind:value={path} />
    </label>
    <label class="field">
      Content
      <textarea class="mono" rows="3" bind:value={content} disabled={bytes !== null}></textarea>
    </label>
    <label class="field">
      Use a file's bytes
      <input type="file" bind:this={bytesInput} onchange={onPickBytes} />
    </label>
    {#if fileName}
      <p class="muted using-bytes">Using bytes from {fileName}<button onclick={clearBytes}>Clear</button></p>
    {/if}
    <div class="btn-row">
      <button onclick={addFile}>Add file</button>
      <button onclick={overwrite}>Overwrite</button>
      <button onclick={deleteFile}>Delete</button>
      <button onclick={newFolder}>New folder</button>
      <button onclick={removeFolder}>Remove folder</button>
    </div>

    <details class="format">
      <summary>Format</summary>
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
        {clusters.toLocaleString()} clusters{#if clusterProblem}{" "}· <span class="warn">{clusterProblem}</span>{/if}
      </p>
      <label class="field">
        Volume label
        <input class="mono" type="text" maxlength="11" bind:value={volumeLabel} />
      </label>
      <button onclick={formatDisk} disabled={!!clusterProblem}>Format disk</button>
    </details>

    <label class="field">
      Load image
      <input type="file" onchange={onLoadImage} />
    </label>
    <button onclick={exportImage}>Export image</button>
  </fieldset>
</section>
