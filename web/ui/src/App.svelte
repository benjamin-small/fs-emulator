<script lang="ts">
  import ActionsPanel from "./components/ActionsPanel.svelte";
  import DirTree from "./components/DirTree.svelte";
  import FatMap from "./components/FatMap.svelte";
  import HexView from "./components/HexView.svelte";
  import Inspector from "./components/Inspector.svelte";
  import Ribbon from "./components/Ribbon.svelte";
  import ScenarioPanel from "./components/ScenarioPanel.svelte";
  import StatusLine from "./components/StatusLine.svelte";
  import StepPanel from "./components/StepPanel.svelte";
  import StringsPanel from "./components/StringsPanel.svelte";
  import Timeline from "./components/Timeline.svelte";
  import { volume } from "./state/volume.svelte";

  // `[` / `]` scrub the timeline and `/` jumps to the path field, all from anywhere
  // except a text field, so typing a path or file content in the Actions panel isn't
  // hijacked. `n` / `p` (scenario step) are handled by ScenarioPanel itself, since
  // they only apply while a scenario is running.
  function onKeydown(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement | null)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;
    if (e.key === "[") volume.seek(Math.max(0, volume.cursor - 1));
    else if (e.key === "]") volume.seek(volume.cursor + 1);
    else if (e.key === "/") {
      e.preventDefault();
      document.getElementById("action-path")?.focus();
    }
  }
</script>
<svelte:window onkeydown={onKeydown} />
<div class="app">
  <header class="topbar">
    <h1>FAT explorer</h1>
    <div id="scenario-slot"><ScenarioPanel /></div>
    <StatusLine />
  </header>
  <div id="ribbon-slot" class="ribbon-slot"><Ribbon /></div>
  <div class="grid">
    <aside class="col left">
      <DirTree />
      <FatMap />
      <ActionsPanel />
    </aside>
    <main class="col center"><HexView /></main>
    <aside class="col right">
      <Inspector />
      <StringsPanel />
      <StepPanel />
    </aside>
  </div>
  <footer id="timeline-slot"><Timeline /></footer>
</div>
