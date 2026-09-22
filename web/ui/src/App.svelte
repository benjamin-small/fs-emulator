<script lang="ts">
  import ActionsPanel from "./components/ActionsPanel.svelte";
  import DirTree from "./components/DirTree.svelte";
  import HexView from "./components/HexView.svelte";
  import Inspector from "./components/Inspector.svelte";
  import StatusLine from "./components/StatusLine.svelte";
</script>
<div class="app">
  <header class="topbar">
    <h1>FAT explorer</h1>
    <div id="scenario-slot"></div>
    <StatusLine />
  </header>
  <div id="ribbon-slot" class="ribbon-slot"></div>
  <div class="grid">
    <aside class="col left">
      <DirTree />
      <section class="panel"><h2>FAT map</h2><p class="muted">Task 8</p></section>
      <ActionsPanel />
    </aside>
    <main class="col center"><HexView /></main>
    <aside class="col right">
      <Inspector />
      <section class="panel"><h2>Strings</h2><p class="muted">Task 9</p></section>
      <section class="panel"><h2>Step</h2><p class="muted">Task 7</p></section>
    </aside>
  </div>
  <footer id="timeline-slot"></footer>
</div>
