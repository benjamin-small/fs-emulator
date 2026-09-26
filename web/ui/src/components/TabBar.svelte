<script lang="ts">
  import { FAMILIES, FAMILY_IDS } from "../fs";
  import type { FsFamilyId } from "../fs/adapter";
  import { workspaces } from "../state/workspace.svelte";

  /** The top bar's family tabs, one per `FAMILY_IDS`, in the WAI-ARIA tabs pattern with
   *  automatic activation: the focused tab is the shown one. Only the active tab is in the Tab
   *  order (roving tabindex); Left/Right move between tabs and wrap, Home/End go to the ends. A
   *  tab whose workspace is running a lesson carries a `lesson` badge, the active tab too, so
   *  the tabs' widths do not jump on a switch. */
  const buttons: HTMLButtonElement[] = $state([]);

  /** Whether `id`'s workspace exists and has a lesson running. Reading `opened` does not
   *  create a workspace: that happens only in `workspaces.activate`. */
  function lessonRunning(id: FsFamilyId): boolean {
    return workspaces.opened.find((w) => w.id === id)?.scenarios.current != null;
  }

  function onKeydown(e: KeyboardEvent, i: number) {
    const n = FAMILY_IDS.length;
    let next: number;
    if (e.key === "ArrowLeft") next = (i - 1 + n) % n;
    else if (e.key === "ArrowRight") next = (i + 1) % n;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = n - 1;
    else return;
    e.preventDefault();
    workspaces.activate(FAMILY_IDS[next]);
    buttons[next]?.focus();
  }
</script>

<div class="tabbar" role="tablist" aria-label="Filesystem">
  {#each FAMILY_IDS as id, i (id)}
    {@const selected = workspaces.activeId === id}
    <!-- A tab whose workspace does not exist yet has no panel to point at until it is first
         activated. -->
    <button
      bind:this={buttons[i]}
      type="button"
      class="tab"
      role="tab"
      id="tab-{id}"
      aria-selected={selected}
      aria-controls={workspaces.opened.some((w) => w.id === id) ? `workspace-${id}` : undefined}
      tabindex={selected ? 0 : -1}
      onclick={() => {
        workspaces.activate(id);
        // Safari and Firefox on macOS do not focus a button on click; the tabs pattern keeps focus
        // on the activated tab, and the Lesson card leaves it there only if it is.
        buttons[i]?.focus();
      }}
      onkeydown={(e) => onKeydown(e, i)}
    >
      {FAMILIES[id].name}
      {#if lessonRunning(id)}<span class="tab-badge">lesson<span class="sr-only"> running</span></span>{/if}
    </button>
  {/each}
</div>
