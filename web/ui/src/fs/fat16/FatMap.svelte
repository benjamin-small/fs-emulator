<script lang="ts">
  import { attrAtOffset } from "../../core/attribution";
  import { cellAt, cellRect, dashCell, dotCell, drawChain, gridCols, gridRows, outlineCell, prepareCanvas } from "../../core/grid";
  import type { Interval } from "../../core/intervals";
  import { observeWidth } from "../../core/observeWidth";
  import { getWorkspace } from "../../state/workspace.svelte";
  import { clusterState } from "./fatchain";
  import { asFat16 } from "./index";

  const { volume, selection, layers } = getWorkspace();

  const CELL = 6, GAP = 1, MAX_HEIGHT = 260;

  let canvas = $state<HTMLCanvasElement>();
  // The panel's content width, kept current by `observeWidth` on the wrapper so `cols` follows
  // the sidebar's actual size (a resizable layout, a narrower viewport, …) rather than a value
  // baked in at mount.
  let width = $state(0);
  let hoverCluster = $state<number | null>(null);

  // The FAT view of the volume's adapter: `fat` (the table), `unitCount`, `unitByteRange`.
  // Its caches are plain fields refreshed per op, not reactive, so every `$derived` and
  // `$effect` below that reads them reads `volume.epoch` first (the rule in fs/adapter.ts).
  const fs = $derived(asFat16(volume.adapter));
  const clusterCount = $derived((volume.epoch, fs.unitCount));
  const cols = $derived(gridCols(width, CELL, GAP));
  const rows = $derived(gridRows(clusterCount, cols));
  const canvasWidth = $derived(Math.max(1, Math.floor(width)));
  const canvasHeight = $derived(Math.max(1, rows * (CELL + GAP)));

  $effect(() => {
    volume.epoch; layers.chain; layers.diff; layers.hover; selection.hoverOffset; canvasWidth; canvasHeight;
    paint();
  });

  /** Cluster `c`'s cell: the grid starts at cluster 2, the first data cluster. */
  function clusterCell(c: number): { x: number; y: number } {
    return cellRect(c - 2, cols, CELL, GAP);
  }

  function clusterAtPoint(px: number, py: number): number | null {
    const i = cellAt(px, py, cols, clusterCount, CELL, GAP);
    return i === null ? null : i + 2;
  }

  /** Whether cluster `c`'s bytes overlap any of `ivs` (the step's diff, the hovered range). */
  function overlapsAny(c: number, ivs: readonly Interval[]): boolean {
    if (!ivs.length) return false;
    const { start, end } = fs.unitByteRange(c);
    for (const iv of ivs) if (iv.start < end && iv.end > start) return true;
    return false;
  }

  function paint() {
    if (!canvas) return;
    const ctx = prepareCanvas(canvas, canvasWidth, canvasHeight);
    if (!ctx) return;

    // Read once per paint (not cached across paints) so a light/dark switch is
    // picked up on the very next repaint without any extra wiring.
    const style = getComputedStyle(canvas);
    const hairline = style.getPropertyValue("--hairline").trim();
    const diffInk = style.getPropertyValue("--diff-ink").trim();
    const diffColor = style.getPropertyValue("--diff").trim();
    const focus = style.getPropertyValue("--focus").trim();
    const ink = style.getPropertyValue("--ink").trim();
    const own = (i: number) => style.getPropertyValue(`--own-${i}`).trim();

    const fat = fs.fat;
    const attribution = volume.attribution;
    // What changed's hovered range or event, as a list for `overlapsAny`.
    const hover = layers.hover ? [layers.hover] : [];

    for (let c = 2; c <= clusterCount + 1; c++) {
      const entry = fat[c];
      if (!entry) continue;
      const { x, y } = clusterCell(c);
      const state = clusterState(entry);
      let fill = hairline;
      if (state === "bad") fill = diffInk;
      else {
        const idx = attribution.ownerByUnit[c];
        if (idx >= 0) fill = own(attribution.colorByUnit[c]);
      }
      ctx.fillStyle = fill;
      ctx.fillRect(x, y, CELL, CELL);

      if (state === "end") dotCell(ctx, x, y, CELL, ink);
      if (overlapsAny(c, layers.diff)) outlineCell(ctx, x, y, CELL, diffColor);
      // The hovered range's clusters, outlined in the focus colour over the diff's. A range in the
      // FATs or the root directory touches none.
      if (overlapsAny(c, hover)) outlineCell(ctx, x, y, CELL, focus);
    }

    if (layers.chain.length) drawChain(ctx, layers.chain.map(clusterCell), CELL, focus, 2);

    // Cross-link with the hex dump: whichever cluster the mouse is currently over
    // there gets a dashed outline here, so the map shows where in the whole disk
    // the hovered byte lives.
    if (selection.hoverOffset !== null) {
      const c = attrAtOffset(attribution, selection.hoverOffset).unit;
      if (c !== undefined) {
        const { x, y } = clusterCell(c);
        dashCell(ctx, x, y, CELL, focus);
      }
    }
  }

  function onMove(e: MouseEvent) {
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    hoverCluster = clusterAtPoint(e.clientX - rect.left, e.clientY - rect.top);
  }
  function onLeave() { hoverCluster = null; }
  function onClick() {
    if (hoverCluster === null) return;
    const idx = volume.attribution.ownerByUnit[hoverCluster];
    if (idx >= 0) selection.select(volume.attribution.owners[idx].path);
    selection.jumpTo(fs.unitByteRange(hoverCluster).start);
  }

  const caption = $derived.by(() => {
    if (hoverCluster === null) return "";
    volume.epoch;
    const entry = fs.fat[hoverCluster];
    if (!entry) return "";
    const state = clusterState(entry);
    const idx = volume.attribution.ownerByUnit[hoverCluster];
    const owner = idx >= 0 ? volume.attribution.owners[idx].path : "—";
    return `cluster ${hoverCluster} · ${state} · ${owner}`;
  });
</script>

<section class="panel fatmap">
  <h2>FAT map &middot; {clusterCount.toLocaleString()} clusters</h2>
  {#if !volume.atLatest}<p class="muted stale-note">Shows the latest state, not the step you are viewing.</p>{/if}
  <div class="fatmap-wrap" use:observeWidth={(w) => (width = w)} style:max-height="{MAX_HEIGHT}px">
    <canvas bind:this={canvas} onmousemove={onMove} onmouseleave={onLeave} onclick={onClick} aria-label="FAT cluster map"></canvas>
  </div>
  <p class="mono muted fatmap-caption">{caption || " "}</p>
</section>
