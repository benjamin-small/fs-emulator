<script lang="ts">
  import type { Snippet } from "svelte";
  import { setWorkspace, type Workspace } from "../state/workspace.svelte";

  /** Gives `children` the workspace `ws` through `getWorkspace()`, without any DOM of its own:
   *  App uses it for the pieces of the page chrome that belong to the active tab (the lesson
   *  picker, the status line, the lesson card), keyed on the tab so they are rebuilt on a
   *  switch. */
  let { ws, children }: { ws: Workspace; children: Snippet } = $props();
  // Context is set once, at init; App's `{#key}` makes a new scope for another workspace.
  // svelte-ignore state_referenced_locally
  setWorkspace(ws);
</script>

{@render children()}
