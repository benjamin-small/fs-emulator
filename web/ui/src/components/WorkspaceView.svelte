<script lang="ts">
  import { PANELS } from "../fs/panels";
  import { setWorkspace, type Workspace } from "../state/workspace.svelte";
  import ActionsPanel from "./ActionsPanel.svelte";
  import DirTree from "./DirTree.svelte";
  import HexView from "./HexView.svelte";
  import Inspector from "./Inspector.svelte";
  import OperationBar from "./OperationBar.svelte";
  import Ribbon from "./Ribbon.svelte";
  import StringsPanel from "./StringsPanel.svelte";
  import WhatChangedPanel from "./WhatChangedPanel.svelte";

  /** One tab's body: everything under the top bar, over the tab's own stores. App keeps one
   *  per opened tab mounted and hides the others, so each keeps its component state. */
  let { ws }: { ws: Workspace } = $props();
  // A workspace is bound to its view for the view's life (App keys the list on `ws.id`), so
  // context is set once, at init.
  // svelte-ignore state_referenced_locally
  setWorkspace(ws);

  /** The map panel of the tab's family (the FAT map, the block-group map), from the panel
   *  registry. The family's aside panels (ext: the Journal) follow What changed at the top of
   *  the right column. */
  const MapPanel = $derived(PANELS[ws.id].map);
</script>

<!-- The Operation bar (the timeline's controls and the step in one line) sits under the top
     bar; what the step wrote heads the right column (What changed), then the family's aside
     panels (ext's Journal), Strings, and the Inspector. The slots are classes, not ids: every
     opened tab has its own. -->
<div class="opbar-slot"><OperationBar /></div>
<div class="ribbon-slot"><Ribbon /></div>
<div class="grid">
  <aside class="col left">
    <DirTree />
    <MapPanel />
    <ActionsPanel />
  </aside>
  <main class="col center"><HexView /></main>
  <aside class="col right">
    <WhatChangedPanel />
    {#each PANELS[ws.id].aside as Aside}<Aside />{/each}
    <StringsPanel />
    <Inspector />
  </aside>
</div>
