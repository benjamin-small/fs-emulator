<script lang="ts">
  import { PANELS } from "../fs/panels";
  import { setWorkspace, type Workspace } from "../state/workspace.svelte";
  import ActionsPanel from "./ActionsPanel.svelte";
  import DirTree from "./DirTree.svelte";
  import HexView from "./HexView.svelte";
  import Inspector from "./Inspector.svelte";
  import Ribbon from "./Ribbon.svelte";
  import StepPanel from "./StepPanel.svelte";
  import StringsPanel from "./StringsPanel.svelte";
  import Timeline from "./Timeline.svelte";

  /** One tab's body: everything under the top bar, over the tab's own stores. App keeps one
   *  per opened tab mounted and hides the others, so each keeps its component state. */
  let { ws }: { ws: Workspace } = $props();
  // A workspace is bound to its view for the view's life (App keys the list on `ws.id`), so
  // context is set once, at init.
  // svelte-ignore state_referenced_locally
  setWorkspace(ws);

  /** The map panel of the tab's family (the FAT map, the block-group map), from the panel
   *  registry. The family's extra panels follow it in order. */
  const MapPanel = $derived(PANELS[ws.id].map);
</script>

<!-- The current step sits with the other controls, under the picker and the Terminal
     button, so the right column is left to state and data (Lesson, Strings, Inspector). The
     slots are classes, not ids: every opened tab has its own. -->
<div class="step-slot"><StepPanel /></div>
<div class="ribbon-slot"><Ribbon /></div>
<div class="grid">
  <aside class="col left">
    <DirTree />
    <MapPanel />
    {#each PANELS[ws.id].extras as Extra}<Extra />{/each}
    <ActionsPanel />
  </aside>
  <main class="col center"><HexView /></main>
  <aside class="col right">
    <StringsPanel />
    <Inspector />
  </aside>
</div>
<footer class="timeline-slot"><Timeline /></footer>
