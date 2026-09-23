<script lang="ts">
  import { volume } from "../state/volume.svelte";

  // Sentence-case, verb-first-adjacent copy per the design spec ("Disk full. Free
  // space or use a smaller file."); falls back to the raw wasm message for codes not
  // listed here (e.g. an unexpected InvalidGeometry from a hand-typed format option).
  // Two entries name the mounted family (`volume.adapter.name`, "FAT16" today), so the
  // table is derived and follows a format or load that binds another adapter.
  const FRIENDLY: Record<string, string> = $derived({
    DiskFull: "Disk full. Free space or use a smaller file.",
    DirectoryFull: "This folder is full. Remove an entry or use a different folder.",
    AlreadyExists: "Something already exists at that path.",
    NotFound: "Nothing exists at that path.",
    InvalidPath: "That path isn't valid.",
    InvalidName: `That name isn't valid for ${volume.adapter.name}.`,
    NotADirectory: "That path is a file, not a folder.",
    IsADirectory: "That path is a folder, not a file.",
    DirectoryNotEmpty: "That folder still has files in it.",
    FileTooLarge: "That file is too large for this disk.",
    InvalidGeometry: "Those format options don't add up to a valid disk.",
    CorruptImage: `That image doesn't look like a valid ${volume.adapter.name} volume.`,
    OutOfBounds: "That range runs past the end of the disk.",
    Unsupported: "That isn't supported yet.",
  });
  const message = $derived.by(() => {
    const s = volume.status;
    if (!s) return "";
    // A CorruptImage status while the volume is actually corrupt came from a raw write, not
    // from loading a foreign image, so say what is wrong with this volume instead of the
    // load-time phrase above.
    if (s.code === "CorruptImage" && volume.corruption) return volume.corruption;
    return (s.code && FRIENDLY[s.code]) || s.text;
  });
</script>
<div class="status" role="status">
  {#if !volume.atLatest && volume.history.length}
    <span>Viewing step {volume.cursor + 1} of {volume.history.length}</span>
    <button onclick={() => volume.backToNow()}>Back to now</button>
  {/if}
  <!-- Shown whenever the volume is unmounted, status or no status: a failed op does not make
       the corruption go away, and the two say different things. -->
  {#if volume.corruption}<span>Volume not mounted: {volume.corruption}</span>{/if}
  {#if volume.status}<span class="err">{message}{#if volume.status.code}{" "}<span class="muted">{volume.status.code}</span>{/if}</span>{/if}
</div>
