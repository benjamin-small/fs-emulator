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
  import TerminalPanel from "./components/TerminalPanel.svelte";
  import Timeline from "./components/Timeline.svelte";
  import { inTextEntry } from "./core/keys";
  import { focusHistoryStep } from "./state/navigate.svelte";
  import { terminal } from "./state/terminal.svelte";
  import { volume } from "./state/volume.svelte";

  /** Scrub to `n` and recenter the dump on what that step changed, like the Timeline's
   *  own controls. Scrubbing is explicit navigation; running an operation is not, and
   *  leaves the dump where it is. */
  function step(n: number) {
    volume.seek(n);
    focusHistoryStep(n);
  }

  // `[` / `]` scrub the timeline and `/` jumps to the path field, all from anywhere
  // except text entry, so typing a path or file content in the Actions panel isn't
  // hijacked. `n` / `p` (scenario step) are handled by ScenarioPanel itself, since
  // they only apply while a scenario is running. The terminal drawer counts as text
  // entry too (see core/keys.ts for the Ctrl-B chord case).
  function onKeydown(e: KeyboardEvent) {
    // A modifier means the chord belongs to the browser or the OS (Cmd-` cycles windows on
    // macOS, Ctrl-[ is Escape in some setups), so none of these are ours to swallow.
    if (e.metaKey || e.ctrlKey || e.altKey || inTextEntry(e)) return;
    if (e.key === "`") {
      // Toggle the terminal from anywhere outside text entry. Inside the terminal the
      // key is typed (xterm cancels it), so closing is exit / Close / Escape on the bar.
      e.preventDefault();
      terminal.toggle();
    } else if (e.key === "[") step(Math.max(0, volume.cursor - 1));
    else if (e.key === "]") step(volume.cursor + 1);
    else if (e.key === "/") {
      e.preventDefault();
      document.getElementById("action-path")?.focus();
    }
  }
</script>
<svelte:window onkeydown={onKeydown} />
<div class="app" style:--term-h="{terminal.height}px">
  <header class="topbar">
    <h1>FAT explorer</h1>
    <button
      id="terminal-toggle"
      type="button"
      aria-pressed={terminal.open}
      aria-controls="terminal-drawer"
      title="Toggle the terminal (`)"
      onclick={() => terminal.toggle()}
    >Terminal</button>
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
      <StepPanel />
      <StringsPanel />
      <Inspector />
    </aside>
  </div>
  <footer id="timeline-slot"><Timeline /></footer>
  <TerminalPanel />
</div>
