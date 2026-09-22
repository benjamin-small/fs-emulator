<script lang="ts">
  import { attrAtOffset, clusterByteRange } from "../core/attribution";
  import { clusterState } from "../core/fatchain";
  import { layers } from "../state/layers.svelte";
  import { selection } from "../state/selection.svelte";
  import { volume } from "../state/volume.svelte";

  const CELL = 6, GAP = 1, MAX_HEIGHT = 260;

  let wrap = $state<HTMLDivElement>();
  let canvas = $state<HTMLCanvasElement>();
  let width = $state(0);
  let hoverCluster = $state<number | null>(null);

  const clusterCount = $derived(volume.geometry.clusterCount);
  const cols = $derived(Math.max(1, Math.floor(width / (CELL + GAP))));
  const rows = $derived(Math.max(1, Math.ceil(clusterCount / cols)));
  const canvasWidth = $derived(Math.max(1, Math.floor(width)));
  const canvasHeight = $derived(Math.max(1, rows * (CELL + GAP)));

  // Track the panel's content width so `cols` follows the sidebar's actual size
  // (a resizable layout, a narrower viewport, …) rather than a value baked in at mount.
  $effect(() => {
    if (!wrap) return;
    width = wrap.getBoundingClientRect().width;
    const ro = new ResizeObserver((entries) => { width = entries[0].contentRect.width; });
    ro.observe(wrap);
    return () => ro.disconnect();
  });

  $effect(() => {
    volume.epoch; layers.chain; layers.diff; selection.hoverOffset; canvasWidth; canvasHeight;
    paint();
  });

  function cellRect(c: number): { x: number; y: number } {
    const i = c - 2, col = i % cols, row = Math.floor(i / cols);
    return { x: col * (CELL + GAP), y: row * (CELL + GAP) };
  }

  function clusterAtPoint(px: number, py: number): number | null {
    const col = Math.floor(px / (CELL + GAP));
    const row = Math.floor(py / (CELL + GAP));
    if (col < 0 || col >= cols || row < 0) return null;
    const c = row * cols + col + 2;
    return c >= 2 && c <= clusterCount + 1 ? c : null;
  }

  function overlapsDiff(c: number): boolean {
    if (!layers.diff.length) return false;
    const { start, end } = clusterByteRange(volume.geometry, c);
    for (const iv of layers.diff) if (iv.start < end && iv.end > start) return true;
    return false;
  }

  function paint() {
    if (!canvas) return;
    canvas.width = canvasWidth;
    canvas.height = canvasHeight;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    // Read once per paint (not cached across paints) so a light/dark switch is
    // picked up on the very next repaint without any extra wiring.
    const style = getComputedStyle(canvas);
    const hairline = style.getPropertyValue("--hairline").trim();
    const diffInk = style.getPropertyValue("--diff-ink").trim();
    const diffColor = style.getPropertyValue("--diff").trim();
    const focus = style.getPropertyValue("--focus").trim();
    const ink = style.getPropertyValue("--ink").trim();
    const own = (i: number) => style.getPropertyValue(`--own-${i}`).trim();

    const fat = volume.fat;
    const attribution = volume.attribution;

    for (let c = 2; c <= clusterCount + 1; c++) {
      const entry = fat[c];
      if (!entry) continue;
      const { x, y } = cellRect(c);
      const state = clusterState(entry);
      let fill = hairline;
      if (state === "bad") fill = diffInk;
      else {
        const idx = attribution.ownerByCluster[c];
        if (idx >= 0) fill = own(attribution.colorByCluster[c]);
      }
      ctx.fillStyle = fill;
      ctx.fillRect(x, y, CELL, CELL);

      if (state === "end") {
        ctx.fillStyle = ink;
        ctx.fillRect(x + 2, y + 2, 2, 2);
      }

      if (overlapsDiff(c)) {
        ctx.strokeStyle = diffColor;
        ctx.lineWidth = 1;
        ctx.strokeRect(x + 0.5, y + 0.5, CELL - 1, CELL - 1);
      }
    }

    const chain = layers.chain;
    if (chain.length) {
      ctx.strokeStyle = focus;
      ctx.lineWidth = 1;
      for (const c of chain) {
        const { x, y } = cellRect(c);
        ctx.strokeRect(x + 0.5, y + 0.5, CELL - 1, CELL - 1);
      }
      ctx.lineWidth = 2;
      ctx.beginPath();
      chain.forEach((c, i) => {
        const { x, y } = cellRect(c);
        const cx = x + CELL / 2, cy = y + CELL / 2;
        if (i === 0) ctx.moveTo(cx, cy); else ctx.lineTo(cx, cy);
      });
      ctx.stroke();
    }

    // Cross-link with the hex dump: whichever cluster the mouse is currently over
    // there gets a dashed outline here, so the map shows where in the whole disk
    // the hovered byte lives.
    if (selection.hoverOffset !== null) {
      const c = attrAtOffset(attribution, selection.hoverOffset).cluster;
      if (c !== undefined) {
        const { x, y } = cellRect(c);
        ctx.save();
        ctx.strokeStyle = focus;
        ctx.setLineDash([1, 1]);
        ctx.lineWidth = 1;
        ctx.strokeRect(x + 0.5, y + 0.5, CELL - 1, CELL - 1);
        ctx.restore();
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
    const idx = volume.attribution.ownerByCluster[hoverCluster];
    if (idx >= 0) selection.select(volume.attribution.owners[idx].path);
    selection.jumpTo(clusterByteRange(volume.geometry, hoverCluster).start);
  }

  const caption = $derived.by(() => {
    if (hoverCluster === null) return "";
    const entry = volume.fat[hoverCluster];
    if (!entry) return "";
    const state = clusterState(entry);
    const idx = volume.attribution.ownerByCluster[hoverCluster];
    const owner = idx >= 0 ? volume.attribution.owners[idx].path : "—";
    return `cluster ${hoverCluster} · ${state} · ${owner}`;
  });
</script>

<section class="panel fatmap">
  <h2>FAT map &middot; {clusterCount.toLocaleString()} clusters</h2>
  {#if !volume.atLatest}<p class="muted stale-note">Shows the latest state, not the step you are viewing.</p>{/if}
  <div class="fatmap-wrap" bind:this={wrap} style:max-height="{MAX_HEIGHT}px">
    <canvas bind:this={canvas} onmousemove={onMove} onmouseleave={onLeave} onclick={onClick} aria-label="FAT cluster map"></canvas>
  </div>
  <p class="mono muted fatmap-caption">{caption || " "}</p>
</section>
