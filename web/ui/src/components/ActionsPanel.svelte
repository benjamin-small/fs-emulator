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

  let path = $state("/Hello world.txt");
  let content = $state("Hello from the browser");
  let bytes = $state<Uint8Array | null>(null);
  let fileName = $state<string | null>(null);

  let totalSectors = $state(SIZES[0].totalSectors);
  let sectorsPerCluster = $state(4);
  let volumeLabel = $state("");

  function data(): Uint8Array {
    return bytes ?? new TextEncoder().encode(content);
  }

  async function onPickBytes(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0] ?? null;
    if (!file) { bytes = null; fileName = null; return; }
    bytes = new Uint8Array(await file.arrayBuffer());
    fileName = file.name;
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
      <input type="file" onchange={onPickBytes} />
    </label>
    {#if fileName}<p class="muted">Using bytes from {fileName}.</p>{/if}
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
      <label class="field">
        Volume label
        <input class="mono" type="text" maxlength="11" bind:value={volumeLabel} />
      </label>
      <button onclick={formatDisk}>Format disk</button>
    </details>

    <label class="field">
      Load image
      <input type="file" onchange={onLoadImage} />
    </label>
    <button onclick={exportImage}>Export image</button>
  </fieldset>
</section>
