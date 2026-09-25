<script lang="ts">
  import { FAMILIES } from "../fs";
  import type { FsFamilyId } from "../fs/adapter";
  import { PANELS } from "../fs/panels";
  import { getWorkspace } from "../state/workspace.svelte";

  const ws = getWorkspace();
  const { volume, selection } = ws;

  /** The family the Format details will format: the mounted one until the Filesystem select
   *  picks another, and back to the mounted one whenever that changes (a format, a load). */
  const mounted = $derived(volume.adapter.id);
  let family = $state<FsFamilyId>(volume.adapter.id);
  $effect(() => { family = mounted; });

  /** The chosen family's Format form, from the panel registry. */
  const FormatPanel = $derived(PANELS[family].format);

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
      <summary>Format</summary>
      <label class="field">
        Filesystem
        <select id="format-family" bind:value={family}>
          {#each Object.values(FAMILIES) as f}<option value={f.id}>{f.name}</option>{/each}
        </select>
      </label>
      <FormatPanel />
    </details>

    <label class="field">
      Load image
      <input type="file" onchange={onLoadImage} />
    </label>
    <button onclick={exportImage}>Export image</button>
  </fieldset>
</section>
