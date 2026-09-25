<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  // browser-terminal renders into our element with no shadow DOM and no stylesheet of
  // its own, so the app loads xterm's CSS. (@xterm/xterm has no `exports` map; the
  // deep path resolves.)
  import "@xterm/xterm/css/xterm.css";
  // Type-only: the runtime import is the lazy `import()` in ensureCreated, so the
  // library's wasm loads only when someone opens the drawer.
  import type { BrowserTerminal } from "@benjamin-small/browser-terminal";
  import { themeFromTokens } from "../core/terminalTheme";
  import { commandSetOf, registerCommands, registrationChange, tabBanner, type Registration } from "../shell/register";
  import type { ShellHost } from "../shell/host";
  import { createRedirectHandler } from "../shell/redirect";
  import { createStoreHost } from "../shell/storeHost.svelte";
  import { promptFor } from "../shell/vfs";
  import { terminal } from "../state/terminal.svelte";
  import { theme } from "../state/theme.svelte";
  import { workspaces, type Workspace } from "../state/workspace.svelte";

  let mountEl = $state<HTMLDivElement>();
  let bt: BrowserTerminal | null = null;
  let creating: Promise<void> | null = null;

  // One terminal for the page, following the active tab. Each tab's workspace gets its own
  // host the first time the terminal follows it, kept for the life of the terminal: a host
  // reads its workspace's stores live, so it never goes stale. The working directory is the
  // workspace's own `vfs`, so a switch keeps each tab's.
  const hosts = new Map<Workspace, ShellHost>();
  // The names the terminal holds, and whose tab and which command set (`commandSetOf`) they
  // were built for, so the effect below can tell a switch from a format. Plain fields, not state.
  let registered: string[] = [];
  let current: Registration<Workspace> | null = null;

  function hostFor(term: BrowserTerminal, ws: Workspace): ShellHost {
    let host = hosts.get(ws);
    if (!host) {
      host = createStoreHost(ws, close, (prefix) => term.setPrompt(prefix));
      hosts.set(ws, host);
    }
    return host;
  }

  /**
   * Print `line` in the active pane in place of its prompt line, leaving the cursor on a fresh
   * line for the caller's `setPrompt` to redraw the prompt (and any typed input) on.
   * browser-terminal 0.3.0 has no call for a host line (`run()` hands its output back instead
   * of printing it), so this gives the pane manager the `paneOutput` event the engine itself
   * sends. An idle pane redraws its prompt only when the prefix changes, so the prefix is
   * cleared here and the caller's `setPrompt` sets it again. Guarded: if a later version
   * renames that internal, the banner is skipped and nothing else changes.
   */
  function printLine(term: BrowserTerminal, line: string) {
    type PaneOutput = { type: "paneOutput"; pane: number; data: string };
    const panes = (term as unknown as { paneManager?: { handleEvent?: (e: PaneOutput) => void } }).paneManager;
    const pane = term.snapshot?.active_pane;
    if (typeof panes?.handleEvent !== "function" || pane === undefined) return;
    panes.handleEvent({ type: "paneOutput", pane, data: `\r\x1b[2K${line}\r\n` });
    term.setPrompt("");
  }

  /**
   * Point the terminal at `ws`, whose command set is `set`: register that tab's commands over
   * its own `vfs` and its redirect handler when the tab or the set changed, print the banner
   * when the tab changed (not on the first registration), and always re-set the prompt from
   * the tab's cwd, which `VolumeStore`'s `onAdopt` puts back at /mnt after a format or a load.
   */
  function follow(term: BrowserTerminal, ws: Workspace, set: string) {
    const next = { ws, set };
    const change = registrationChange(current, next);
    const host = hostFor(term, ws);
    if (change.register) {
      registered = registerCommands(term, host, ws.vfs, registered);
      // `>`, `>>` and `<` resolve through the same VFS the commands do, so
      // `echo hi > /mnt/A.TXT` is the journaled write `echo hi | write /mnt/A.TXT` is.
      term.setRedirectHandler(createRedirectHandler(host, ws.vfs));
      current = next;
    }
    if (change.banner) printLine(term, tabBanner(ws.id, ws.volume.vol.fsType()));
    host.setPrompt(promptFor(ws.vfs.cwd));
  }

  /** Close the drawer and hand focus to the topbar button, since the element that had
   *  focus (the active pane's terminal input) is about to be hidden. `exit`, the Close
   *  button, and Escape on the bar all come through here. */
  function close() {
    terminal.close();
    document.getElementById("terminal-toggle")?.focus();
  }

  function focusShell() {
    bt?.focus();
  }

  /** The app's tokens as xterm settings. Read off the document each time, so the values
   *  are whatever `data-theme` on the root currently resolves them to. */
  function currentTheme() {
    return themeFromTokens(getComputedStyle(document.documentElement));
  }

  // The tokens swap when the switch sets `data-theme`, but xterm holds its theme in JS
  // rather than reading CSS, so the swap has to be pushed in. The store puts the attribute
  // on the root before this effect runs, so the computed tokens are already the new ones.
  $effect(() => {
    void theme.current;
    untrack(() => bt?.setTheme(currentTheme().theme));
  });

  /**
   * Create the terminal on first use. Called after the drawer is visible so the
   * library's ResizeObserver fits real dimensions on its first pass. One instance per
   * page is the library's rule; `bt` is the instance, `creating` the in-flight
   * promise, so a second open during the first load reuses it.
   */
  function ensureCreated(): Promise<void> {
    if (bt) return Promise.resolve();
    if (creating) return creating;
    const mount = mountEl;
    if (!mount) return Promise.resolve();
    terminal.ready = "loading";
    terminal.error = null;
    creating = (async () => {
      const { BrowserTerminal } = await import("@benjamin-small/browser-terminal");
      const { theme: xtermTheme, fontFamily } = currentTheme();
      // 12px matches the dump's `--dump-size` neighbourhood and keeps a usable number of
      // columns in a 220px drawer; the library's own default is 13.
      const term = await BrowserTerminal.create({ mount, terminal: { theme: xtermTheme, fontFamily, fontSize: 12 } });
      try {
        // Commands read live store fields through the host on every call; only what is fixed
        // at registration (the command set, its summaries and flag descriptions) follows the
        // tab and its family, through the effect below. Register the active tab's and seed
        // the prompt with its cwd; `cd`, `mkfs`, and the effect keep it in step from there.
        const ws = workspaces.active;
        follow(term, ws, commandSetOf(ws.volume.adapter));
      } catch (e) {
        // A half-registered instance would still hold the library's one-per-page slot, so
        // every later open would fail to create and sit behind a permanent error banner.
        // Dispose it and leave `bt` null: the next open starts over.
        term.dispose();
        forget();
        throw e;
      }
      bt = term;
      terminal.ready = "ready";
    })()
      .catch((e: unknown) => {
        terminal.ready = "error";
        terminal.error = e instanceof Error ? e.message : String(e);
      })
      .finally(() => {
        creating = null;
      });
    return creating;
  }

  // Opening (and every openAndFocus while open) creates on first use, then focuses the
  // shell. The library focuses the pane itself on creation; later opens need this.
  $effect(() => {
    void terminal.focusNonce;
    if (!terminal.open) return;
    tick()
      .then(ensureCreated)
      .then(() => requestAnimationFrame(focusShell));
  });

  // Follow the active tab. Every command reads the live adapter when it runs, but
  // browser-terminal keeps each spec as it was registered: the summaries and flag descriptions
  // that name the family's nouns (`df`'s "Cluster usage", the `b:65 (block)` address help,
  // `mkfs`'s types) and whether `crash` and `recover` exist at all are fixed then. So on a tab
  // switch, or when the tab's own set changes (ext3 formatted as ext2), swap the whole set over
  // that tab's `vfs`, with the banner on a switch. A format or a load replaces the volume and
  // `onAdopt` has put that tab's cwd back at /mnt, so the prompt is re-set every time. The
  // tracked reads are the active workspace, its adapter's command set, and its `vol`; the
  // terminal, the hosts, and the registration are plain fields, and the work is untracked.
  // Before the terminal exists there is nothing to follow: creation registers the tab active then.
  $effect(() => {
    const ws = workspaces.active;
    const set = commandSetOf(ws.volume.adapter);
    void ws.volume.vol;
    untrack(() => {
      if (bt) follow(bt, ws, set);
    });
  });

  /** Drop what belonged to a terminal instance: its hosts (their prompt callback holds it) and
   *  the registration. */
  function forget() {
    hosts.clear();
    registered = [];
    current = null;
  }

  function disposeTerminal() {
    bt?.dispose();
    bt = null;
    forget();
  }
  // HMR replaces this module: dispose first or the next create() throws "one instance
  // per page". The unmount cleanup covers the non-HMR teardown. dispose() is idempotent.
  if (import.meta.hot) import.meta.hot.dispose(disposeTerminal);
  onMount(() => disposeTerminal);

  // Drag the bar to resize. The store clamps each move and persists once on pointer up;
  // the library's ResizeObserver on the mount refits the terminal as the track height changes.
  let drag: { pointerId: number; startY: number; startH: number } | null = null;
  function onBarDown(e: PointerEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    if (terminal.placement === "side") return; // the bar resizes the bottom drawer only
    drag = { pointerId: e.pointerId, startY: e.clientY, startH: terminal.height };
    capture(e);
    e.preventDefault();
  }
  function onBarMove(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    terminal.setHeight(drag.startH + (drag.startY - e.clientY));
  }
  function onBarUp(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    drag = null;
    // Also on pointercancel: the drawer keeps the size it was dragged to, so that is the
    // height to remember. Committed before the capture is released, which can throw when
    // the browser already dropped the pointer (a cancelled touch, a window blur).
    terminal.commitHeight();
    release(e);
  }

  /** Pointer capture keeps a drag alive when the pointer leaves the handle. Best effort:
   *  a pointer the browser no longer tracks (or a synthetic event) throws, and the drag
   *  still works while the pointer stays over the handle. */
  function capture(e: PointerEvent) {
    try {
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    } catch {
      // No active pointer with that id: nothing to capture.
    }
  }
  function release(e: PointerEvent) {
    try {
      (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
    } catch {
      // Already released by the browser.
    }
  }

  // In the side column the left edge is the handle and dragging it left widens the column.
  // Same commit-on-pointer-up rule as the bar, into the column's own stored width.
  let gripDrag: { pointerId: number; startX: number; startW: number } | null = null;
  let gripping = $state(false);
  function onGripDown(e: PointerEvent) {
    gripDrag = { pointerId: e.pointerId, startX: e.clientX, startW: terminal.width };
    gripping = true;
    capture(e);
    e.preventDefault();
  }
  function onGripMove(e: PointerEvent) {
    if (!gripDrag || e.pointerId !== gripDrag.pointerId) return;
    terminal.setWidth(gripDrag.startW + (gripDrag.startX - e.clientX));
  }
  function onGripUp(e: PointerEvent) {
    if (!gripDrag || e.pointerId !== gripDrag.pointerId) return;
    gripDrag = null;
    gripping = false;
    terminal.commitWidth();
    release(e);
  }

  // Escape closes from the bar or the Close button. Inside the terminal xterm cancels
  // Escape before it bubbles, so this never fires while typing a command.
  function onKeydown(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    e.preventDefault();
    close();
  }
</script>

<!-- Re-clamp on viewport changes so the drawer never exceeds 60% of a smaller window. The
     store re-derives the rendered height from the user's choice and leaves storage alone, so
     widening the window again restores the height they picked. -->
<svelte:window onresize={() => terminal.syncViewport()} />

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
  id="terminal-drawer"
  class="terminal-drawer panel"
  class:side={terminal.placement === "side"}
  hidden={!terminal.open}
  aria-label="Terminal"
  onkeydown={onKeydown}
>
  {#if terminal.placement === "side"}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="term-grip"
      class:dragging={gripping}
      title="Drag to resize"
      onpointerdown={onGripDown}
      onpointermove={onGripMove}
      onpointerup={onGripUp}
      onpointercancel={onGripUp}
    ></div>
  {/if}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="term-bar" onpointerdown={onBarDown} onpointermove={onBarMove} onpointerup={onBarUp} onpointercancel={onBarUp}>
    <span class="term-title">Terminal <span class="muted">— /mnt is the volume, /dev/hda the raw disk. Type help.</span></span>
    <button type="button" onclick={close} aria-label="Close terminal">Close</button>
  </div>
  {#if terminal.ready === "loading"}<p class="muted term-note">Loading the shell…</p>{/if}
  {#if terminal.ready === "error"}<p class="term-note err">Could not start the terminal: {terminal.error}</p>{/if}
  <div class="terminal-mount" bind:this={mountEl}></div>
</section>
