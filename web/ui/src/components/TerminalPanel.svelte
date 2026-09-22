<script lang="ts">
  import { onMount, tick } from "svelte";
  // browser-terminal renders into our element with no shadow DOM and no stylesheet of
  // its own, so the app loads xterm's CSS. (@xterm/xterm has no `exports` map; the
  // deep path resolves.)
  import "@xterm/xterm/css/xterm.css";
  // Type-only: the runtime import is the lazy `import()` in ensureCreated, so the
  // library's wasm loads only when someone opens the drawer.
  import type { BrowserTerminal } from "@benjamin-small/browser-terminal";
  import { createCommands } from "../shell/commands";
  import { createStoreHost } from "../shell/storeHost.svelte";
  import { Vfs, promptFor } from "../shell/vfs";
  import { terminal } from "../state/terminal.svelte";

  let mountEl = $state<HTMLDivElement>();
  let bt: BrowserTerminal | null = null;
  let creating: Promise<void> | null = null;

  // One working directory per page (the commands' own rule), owned here so the prompt
  // can be seeded from it as soon as the commands are registered.
  const vfs = new Vfs();

  /** Close the drawer and hand focus to the topbar button, since the element that had
   *  focus (xterm's helper textarea) is about to be hidden. `exit`, the Close button,
   *  and Escape on the bar all come through here. */
  function close() {
    terminal.close();
    document.getElementById("terminal-toggle")?.focus();
  }

  function focusShell() {
    mountEl?.querySelector<HTMLTextAreaElement>(".xterm-helper-textarea")?.focus();
  }

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
      const term = await BrowserTerminal.create({ mount });
      try {
        // `setPrompt` lands in browser-terminal 0.3.0
        // (https://github.com/benjamin-small/browser-terminal/issues/12); the pinned 0.2.0
        // has no such method, so this optional call is a no-op until the pin moves and the
        // prompt starts showing the working directory before the `❯`.
        const applyPrompt = (prefix: string) => (term as { setPrompt?: (p: string) => void }).setPrompt?.(prefix);
        // Commands read live store fields through the host on every call, so registering
        // once is enough (same pattern as browser-terminal's Svelte demo).
        const host = createStoreHost(close, applyPrompt);
        for (const { spec, fn } of createCommands(host, vfs)) term.registerCommand(spec, fn);
        // Seed the prompt with the directory the shell starts in; `cd` and `mkfs` keep it
        // in step from there.
        host.setPrompt(promptFor(vfs.cwd));
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

  function disposeTerminal() {
    bt?.dispose();
    bt = null;
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
    drag = { pointerId: e.pointerId, startY: e.clientY, startH: terminal.height };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    e.preventDefault();
  }
  function onBarMove(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    terminal.setHeight(drag.startH + (drag.startY - e.clientY));
  }
  function onBarUp(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    drag = null;
    (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
    // Also on pointercancel: the drawer keeps the size it was dragged to, so that is the
    // height to remember.
    terminal.commitHeight();
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
  hidden={!terminal.open}
  aria-label="Terminal"
  onkeydown={onKeydown}
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="term-bar" onpointerdown={onBarDown} onpointermove={onBarMove} onpointerup={onBarUp} onpointercancel={onBarUp}>
    <span class="term-title">Terminal <span class="muted">— /mnt is the volume, /dev/hda the raw disk. Type help.</span></span>
    <button type="button" onclick={close} aria-label="Close terminal">Close</button>
  </div>
  {#if terminal.ready === "loading"}<p class="muted term-note">Loading the shell…</p>{/if}
  {#if terminal.ready === "error"}<p class="term-note err">Could not start the terminal: {terminal.error}</p>{/if}
  <div class="terminal-mount" bind:this={mountEl}></div>
</section>
