<script lang="ts">
  import { cellRect, dashCell, dotCell, drawChain, outlineCell, prepareCanvas } from "../../core/grid";
  import { observeWidth } from "../../core/observeWidth";
  import { layers } from "../../state/layers.svelte";
  import { selection } from "../../state/selection.svelte";
  import { volume } from "../../state/volume.svelte";
  import { CELL, FILL_FREE, GAP, HEADER, MAX_HEIGHT, bandHeader, blockAt, blockCaption, blockFills, blocksTouched, cellOf, clickPath, indirectBlocks, layoutBands, mapHeading, wrapHeader } from "./blockMap";
  import { asExt } from "./index";

  let canvas = $state<HTMLCanvasElement>();
  // The panel's content width, kept current by `observeWidth` on the wrapper so the bands wrap
  // to the sidebar's actual size.
  let width = $state(0);
  let hoverBlock = $state<number | null>(null);
  /** The lines the tallest band header needs at this width in the map's font; every band's
   *  header row is that tall. `paint` measures it, because measuring sets the canvas's font, a
   *  side effect no `$derived` may have. */
  let headerLines = $state(1);

  // The ext view of the volume's adapter: `geo` (the groups and their free counts) and
  // `journalBlocks`. Its caches are plain fields refreshed per op, not reactive, so every
  // `$derived` and `$effect` below that reads them reads `volume.epoch` first (the rule in
  // fs/adapter.ts).
  const fs = $derived(asExt(volume.adapter));
  const geo = $derived((volume.epoch, fs.geo));
  const headers = $derived(geo.groups.map(bandHeader));
  const map = $derived(layoutBands(geo, width, headerLines));
  const fills = $derived((volume.epoch, blockFills(volume.attribution, geo, fs.journalBlocks)));
  const heading = $derived(mapHeading(geo));

  $effect(() => {
    volume.epoch; layers.chain; layers.diff; selection.hoverOffset; headers; map; fills;
    paint();
  });

  function headerFont(el: HTMLCanvasElement): string {
    return `11px ${getComputedStyle(el).getPropertyValue("--font-mono").trim()}`;
  }

  function paint() {
    if (!canvas) return;
    // Drawn at the device's ratio, so the band headers stay sharp.
    const ctx = prepareCanvas(canvas, Math.max(1, Math.floor(width)), map.height);
    if (!ctx) return;

    // Each band's header in lines that fit the width in the map's font (a one-line header is
    // wider than the sidebar), measured on every paint so a late web font is picked up. When
    // the tallest needs a different number of lines the layout changes: record it and let the
    // effect paint again with the new layout.
    ctx.font = headerFont(canvas);
    const wrapped = width > 0 ? headers.map((t) => wrapHeader(t, Math.floor(width), (s) => ctx.measureText(s).width)) : headers.map((t) => [t]);
    const lines = Math.max(1, ...wrapped.map((l) => l.length));
    if (lines !== headerLines) {
      headerLines = lines;
      return;
    }

    // Read once per paint (not cached across paints) so a light/dark switch is
    // picked up on the very next repaint without any extra wiring.
    const style = getComputedStyle(canvas);
    const hairline = style.getPropertyValue("--hairline").trim();
    const diffColor = style.getPropertyValue("--diff").trim();
    const focus = style.getPropertyValue("--focus").trim();
    const ink = style.getPropertyValue("--ink").trim();
    const muted = style.getPropertyValue("--ink-muted").trim();
    const own = new Map<number, string>();
    const fillFor = (f: number) => {
      if (f === FILL_FREE) return hairline;
      let c = own.get(f);
      if (c === undefined) own.set(f, (c = style.getPropertyValue(`--own-${f}`).trim()));
      return c;
    };

    ctx.textBaseline = "middle";
    ctx.fillStyle = muted;
    map.bands.forEach((band, i) => wrapped[i].forEach((line, k) => ctx.fillText(line, 0, band.top + k * HEADER + HEADER / 2 - 1)));

    // Runs of one colour are the rule (a file's blocks, the journal, free space), so the fill
    // style changes only at a run's edge.
    let last = -1;
    for (const band of map.bands) {
      for (let i = 0; i < band.count; i++) {
        const f = fills[band.first + i];
        if (f !== last) { ctx.fillStyle = fillFor(f); last = f; }
        const c = cellRect(i, map.cols, CELL, GAP);
        ctx.fillRect(c.x, band.cellsTop + c.y, CELL, CELL);
      }
    }

    // A pointer block (a file's indirect block, or the journal's) gets a dot, like the FAT
    // map's end-of-chain mark.
    for (const b of indirectBlocks(volume.attribution)) {
      const c = cellOf(map, b);
      if (c) dotCell(ctx, c.x, c.y, CELL, ink);
    }

    for (const b of blocksTouched(layers.diff, geo.blockSize, geo.totalBlocks)) {
      const c = cellOf(map, b);
      if (c) outlineCell(ctx, c.x, c.y, CELL, diffColor);
    }

    const chain = layers.chain.map((b) => cellOf(map, b)).filter((c): c is { x: number; y: number } => c !== null);
    if (chain.length) drawChain(ctx, chain, CELL, focus, 1);

    // Cross-link with the hex dump: the block under the mouse there gets a dashed outline
    // here, so the map shows where in the whole disk the hovered byte lives.
    if (selection.hoverOffset !== null) {
      const c = cellOf(map, Math.floor(selection.hoverOffset / geo.blockSize));
      if (c) dashCell(ctx, c.x, c.y, CELL, focus);
    }
  }

  function onMove(e: MouseEvent) {
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    hoverBlock = blockAt(map, e.clientX - rect.left, e.clientY - rect.top);
  }
  function onLeave() { hoverBlock = null; }
  function onClick() {
    if (hoverBlock === null) return;
    const path = clickPath(volume.attribution, hoverBlock);
    if (path !== null) selection.select(path);
    selection.jumpTo(hoverBlock * geo.blockSize);
  }

  const caption = $derived.by(() => {
    if (hoverBlock === null) return "";
    volume.epoch;
    return blockCaption(volume.attribution, hoverBlock);
  });
</script>

<section class="panel bgmap">
  <h2>{heading}</h2>
  {#if !volume.atLatest}<p class="muted stale-note">Shows the latest state, not the step you are viewing.</p>{/if}
  <div class="bgmap-wrap" use:observeWidth={(w) => (width = w)} style:max-height="{MAX_HEIGHT}px">
    <canvas bind:this={canvas} onmousemove={onMove} onmouseleave={onLeave} onclick={onClick} aria-label="Block group map"></canvas>
  </div>
  <p class="mono muted bgmap-caption">{caption || " "}</p>
</section>
