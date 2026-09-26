<script lang="ts">
  import { getWorkspace } from "../../state/workspace.svelte";
  import { DEFAULTS, SIZES, checkFormat, defaultInodesPerGroup, defaultJournalBlocks, formOptions } from "./format";

  const { volume, selection } = getWorkspace();

  // Start from the disk every ext lesson runs on (ext3, 16 MB, ordered, default inode and
  // journal sizes); a blank number field means "the default", shown as its placeholder.
  let variant = $state<"ext2" | "ext3">(DEFAULTS.variant);
  let totalBlocks = $state<number>(DEFAULTS.totalBlocks);
  let inodesPerGroup = $state<number | null | undefined>(undefined);
  let label = $state<string>(DEFAULTS.label);
  let journalMode = $state<"ordered" | "data">("ordered");
  let journalBlocks = $state<number | null | undefined>(undefined);

  const options = $derived(formOptions({ variant, totalBlocks, inodesPerGroup, label, journalMode, journalBlocks }));
  /** The problem the options have, if any; it disables the button. */
  const check = $derived(checkFormat(options));

  function formatDisk() {
    volume.format("ext", options);
    // On success VolumeStore.format already cleared status and reset the selection; on
    // failure it set status and left the disk alone, so selection.select(null) must not run
    // here (it would wipe that status unconditionally, same as storeHost.format's guard).
    if (!volume.status) selection.select(null);
  }
</script>

<label class="field">
  Variant
  <select bind:value={variant}>
    <option value="ext2">ext2</option>
    <option value="ext3">ext3</option>
  </select>
</label>
<label class="field">
  Size
  <select bind:value={totalBlocks}>
    {#each SIZES as s}<option value={s.totalBlocks}>{s.label}</option>{/each}
  </select>
</label>
<label class="field">
  Inodes per group
  <input class="mono" type="number" min="16" max="8192" step="8" placeholder={String(defaultInodesPerGroup(totalBlocks))} bind:value={inodesPerGroup} />
</label>
<label class="field">
  Label
  <input class="mono" type="text" maxlength="16" bind:value={label} />
</label>
{#if variant === "ext3"}
  <label class="field">
    Journal mode
    <select bind:value={journalMode}>
      <option value="ordered">ordered</option>
      <option value="data">data</option>
    </select>
  </label>
  <label class="field">
    Journal blocks
    <input class="mono" type="number" min="1024" placeholder={String(defaultJournalBlocks(totalBlocks) ?? "")} bind:value={journalBlocks} />
  </label>
{/if}
{#if check.problem}<p class="format-check warn">{check.problem}</p>{/if}
<button onclick={formatDisk} disabled={check.problem !== null}>Format disk</button>
