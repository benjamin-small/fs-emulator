<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  // browser-terminal renders into our element with no shadow DOM and no stylesheet of
  // its own, so the app loads xterm's CSS. (@xterm/xterm has no `exports` map; the
  // deep path resolves.)
  import "@xterm/xterm/css/xterm.css";
  // Type-only: the runtime import is the lazy `import()` in ensureCreated, so the
  // library's wasm loads only when someone opens the drawer.
  import type { BrowserTerminal } from "@benjamin-small/browser-terminal";
  import type { Volume } from "../lib/wasm";
  import { themeFromTokens } from "../core/terminalTheme";
  import { commandSetOf, registerCommands } from "../shell/register";
  import type { ShellHost } from "../shell/host";
  import { createRedirectHandler } from "../shell/redirect";
  import { createStoreHost } from "../shell/storeHost.svelte";
  import { MOUNT, Vfs, promptFor } from "../shell/vfs";
  import { terminal } from "../state/terminal.svelte";
  import { theme } from "../state/theme.svelte";
  import { workspaces } from "../state/workspace.svelte";

  let mountEl = $state<HTMLDivElement>();
  let bt: BrowserTerminal | null = null;
  let creating: Promise<void> | null = null;

  // One working directory per page (the commands' own rule), owned here so the prompt
  // can be seeded from it as soon as the commands are registered.
  const vfs = new Vfs();
  // The registered commands' host, kept so the volume watcher below can reach `setPrompt`.
  let host: ShellHost | null = null;
  // The names the terminal holds and the command set they were built for (`commandSetOf`),
  // so the family watcher below can swap them all. Plain fields, not state.
  let registered: string[] = [];
  let registeredSet: string | null = null;

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
        // family, through the re-registration effect below. The host is the active tab's
        // workspace's (the page opens one workspace for now).
        const created = createStoreHost(workspaces.active, close, (prefix) => term.setPrompt(prefix));
        registered = registerCommands(term, created, vfs);
        registeredSet = commandSetOf(workspaces.active.volume.adapter);
        // `>`, `>>` and `<` resolve through the same VFS the commands do, so
        // `echo hi > /mnt/A.TXT` is the journaled write `echo hi | write /mnt/A.TXT` is.
        term.setRedirectHandler(createRedirectHandler(created, vfs));
        // Seed the prompt with the directory the shell starts in; `cd`, `mkfs`, and the
        // volume watcher below keep it in step from there.
        created.setPrompt(promptFor(vfs.cwd));
        host = created;
      } catch (e) {
        // A half-registered instance would still hold the library's one-per-page slot, so
        // every later open would fail to create and sit behind a permanent error banner.
        // Dispose it and leave `bt` null: the next open starts over.
        term.dispose();
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

  // A format or a load replaces the volume: VolumeStore.adopt swaps `vol` for a brand new
  // wasm Volume, and the directory the shell was sitting in no longer exists. `mkfs` resets
  // the cwd itself; the Actions panel's Format, a scenario step's `step.format`, and Load
  // image do not, so follow the volume here and put the shell back at /mnt with a matching
  // prompt. The active workspace's `volume.vol` is the only tracked read: `seenVol` and
  // `vfs.cwd` are plain fields, and the prompt call is untracked so this effect can never
  // depend on what it writes.
  let seenVol: Volume | null = null;
  $effect(() => {
    const vol = workspaces.active.volume.vol;
    if (vol === seenVol) return;
    const first = seenVol === null;
    seenVol = vol;
    if (first) return; // the disk the page started on; nothing to reset
    vfs.cwd = MOUNT;
    untrack(() => host?.setPrompt(promptFor(vfs.cwd)));
  });

  // Re-register when the mounted family (or its journal) changes. Every command reads the
  // live adapter when it runs, but browser-terminal keeps each spec as it was registered: the
  // summaries and flag descriptions that name the family's nouns (`df`'s "Cluster usage",
  // the `b:65 (block)` address help) and whether `crash` and `recover` exist at all are fixed
  // then. So swap the whole set: unregister every name, register `createCommands` again over
  // the same `vfs` (the working directory survives), and re-set the prompt. The active
  // workspace's `volume.adapter` is the only tracked read; the terminal and the names are
  // plain fields, and the work is untracked. Before the terminal exists there is nothing to
  // swap: creation registers the set of the family mounted then.
  $effect(() => {
    const set = commandSetOf(workspaces.active.volume.adapter);
    untrack(() => {
      if (!bt || !host || set === registeredSet) return;
      registered = registerCommands(bt, host, vfs, registered);
      registeredSet = set;
      host.setPrompt(promptFor(vfs.cwd));
    });
  });

  function disposeTerminal() {
    bt?.dispose();
    bt = null;
    host = null;
    registered = [];
    registeredSet = null;
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
