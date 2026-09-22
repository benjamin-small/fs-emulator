<script lang="ts">
  import { volume } from "../state/volume.svelte";

  // Sentence-case, verb-first-adjacent copy per the design spec ("Disk full. Free
  // space or use a smaller file."); falls back to the raw wasm message for codes not
  // listed here (e.g. an unexpected InvalidGeometry from a hand-typed format option).
  const FRIENDLY: Record<string, string> = {
    DiskFull: "Disk full. Free space or use a smaller file.",
    DirectoryFull: "This folder is full. Remove an entry or use a different folder.",
    AlreadyExists: "Something already exists at that path.",
    NotFound: "Nothing exists at that path.",
    InvalidPath: "That path isn't valid.",
    InvalidName: "That name isn't valid for FAT16.",
    NotADirectory: "That path is a file, not a folder.",
    IsADirectory: "That path is a folder, not a file.",
    DirectoryNotEmpty: "That folder still has files in it.",
    FileTooLarge: "That file is too large for this disk.",
    InvalidGeometry: "Those format options don't add up to a valid disk.",
    CorruptImage: "That image doesn't look like a valid FAT16 volume.",
    Unsupported: "That isn't supported yet.",
  };
  const message = $derived.by(() => {
    const s = volume.status;
    if (!s) return "";
    return (s.code && FRIENDLY[s.code]) || s.text;
  });
</script>
<div class="status" role="status">
  {#if !volume.atLatest && volume.history.length}
    <span>Viewing step {volume.cursor + 1} of {volume.history.length}</span>
    <button onclick={() => volume.backToNow()}>Back to now</button>
  {/if}
  {#if volume.status}<span class="err">{message}{#if volume.status.code} <span class="muted">{volume.status.code}</span>{/if}</span>{/if}
</div>
