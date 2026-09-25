<script lang="ts">
  import { FAMILIES } from "../fs";
  import { PANELS } from "../fs/panels";
  import { getWorkspace, workspaces } from "../state/workspace.svelte";

  const ws = getWorkspace();
  const { volume, selection } = ws;

  /** The tab's Format form, from the panel registry: a tab formats its own family only. */
  const FormatPanel = PANELS[volume.family].format;
  /** The types that form makes: "FAT16", "ext2 / ext3". */
  const formatTypes = FAMILIES[volume.family].fsTypes.join(" / ");

  let path = $state("/Hello world.txt");
  let content = $state("Hello from the browser");
  let bytes = $state<Uint8Array | null>(null);
  let fileName = $state<string | null>(null);
  let bytesInput = $state<HTMLInputElement>();

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

  async function onLoadImage(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0] ?? null;
    if (!file) return;
    // An image of the other family opens in that family's tab (`loadImage` says so there).
    try {
      workspaces.loadImage(new Uint8Array(await file.arrayBuffer()), file.name, ws);
    } catch (err) {
      volume.report(err);
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
      <input id="action-path-{ws.id}" class="mono" type="text" bind:value={path} />
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
      <summary>Format ({formatTypes})</summary>
      <FormatPanel />
    </details>

    <label class="field">
      Load image
      <input type="file" onchange={onLoadImage} aria-describedby="load-hint-{ws.id}" />
    </label>
    <p id="load-hint-{ws.id}" class="muted load-hint">FAT16 and ext images each open in their own tab.</p>
    <button onclick={exportImage}>Export image</button>
  </fieldset>
</section>
