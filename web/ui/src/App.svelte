<script lang="ts">
  import LessonPanel from "./components/LessonPanel.svelte";
  import ScenarioPanel from "./components/ScenarioPanel.svelte";
  import StatusLine from "./components/StatusLine.svelte";
  import TerminalPanel from "./components/TerminalPanel.svelte";
  import WorkspaceScope from "./components/WorkspaceScope.svelte";
  import WorkspaceView from "./components/WorkspaceView.svelte";
  import { inTextEntry } from "./core/keys";
  import { terminal } from "./state/terminal.svelte";
  import { theme } from "./state/theme.svelte";
  import { workspaces } from "./state/workspace.svelte";

  // `[` / `]` scrub the active tab's timeline and `/` jumps to its path field, all from
  // anywhere except text entry, so typing a path or file content in the Actions panel isn't
  // hijacked. `n` / `p` (scenario step) are handled by LessonPanel itself, which only
  // exists while a lesson is running. The terminal drawer counts as text
  // entry too (see core/keys.ts for the Ctrl-B chord case).
  function onKeydown(e: KeyboardEvent) {
    // A modifier means the chord belongs to the browser or the OS (Cmd-` cycles windows on
    // macOS, Ctrl-[ is Escape in some setups), so none of these are ours to swallow.
    if (e.metaKey || e.ctrlKey || e.altKey || inTextEntry(e)) return;
    const ws = workspaces.active;
    if (e.key === "`") {
      // Toggle the terminal from anywhere outside text entry. Inside the terminal the
      // key is typed (xterm cancels it), so closing is exit / Close / Escape on the bar.
      e.preventDefault();
      terminal.toggle();
    } else if (e.key === "[") ws.goToStep(Math.max(0, ws.volume.cursor - 1));
    else if (e.key === "]") ws.goToStep(ws.volume.cursor + 1);
    else if (e.key === "/") {
      e.preventDefault();
      document.getElementById(`action-path-${workspaces.activeId}`)?.focus();
    }
  }
</script>
<svelte:window onkeydown={onKeydown} />
<div class="app" class:term-side={terminal.placement === "side"} style:--term-h="{terminal.height}px" style:--term-w="{terminal.width}px">
  <header class="topbar">
    <h1>fs explorer</h1>
    <button
      id="terminal-toggle"
      type="button"
      aria-pressed={terminal.open}
      aria-controls="terminal-drawer"
      title="Toggle the terminal (`)"
      onclick={() => terminal.toggle()}
    >Terminal</button>
    <!-- The picker and the status line belong to the active tab: rebuilt over its workspace on
         a switch. -->
    {#key workspaces.active}
      <WorkspaceScope ws={workspaces.active}>
        <div id="scenario-slot"><ScenarioPanel /></div>
        <StatusLine />
      </WorkspaceScope>
    {/key}
    <button
      id="theme-toggle"
      type="button"
      role="switch"
      class="switch"
      aria-checked={theme.isDark}
      title="Switch between dark and light"
      onclick={() => theme.toggle()}
    >
      <span>Dark mode</span>
      <span class="switch-track" aria-hidden="true"><span class="switch-knob"></span></span>
    </button>
  </header>
  <!-- One body per opened tab, kept mounted so each keeps its panels' state; only the active
       one is shown. -->
  {#each workspaces.opened as ws (ws.id)}
    <div id="workspace-{ws.id}" class="workspace" hidden={!ws.active}><WorkspaceView {ws} /></div>
  {/each}
  <TerminalPanel />
  <!-- Floating (position: fixed), so its place in the DOM is only reading order: after
       everything it talks about. One card for the page, over the active tab's lesson. -->
  {#key workspaces.active}
    <WorkspaceScope ws={workspaces.active}>
      {#if workspaces.active.scenarios.current}<LessonPanel />{/if}
    </WorkspaceScope>
  {/key}
</div>
