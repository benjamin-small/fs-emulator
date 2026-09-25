<script lang="ts">
  import { untrack } from "svelte";
  import { attrAtSector, type Attr } from "../core/attribution";
  import { freeSpaceLabel } from "../core/freeSpace";
  import { colorIndexForPath } from "../core/palette";
  import { buildTree, type TreeNode } from "../core/tree";
  import type { Region } from "../lib/wasm";
  import { layers } from "../state/layers.svelte";
  import { selection } from "../state/selection.svelte";
  import { volume } from "../state/volume.svelte";

  const HEIGHT = 28;
  const FLASH_MS = 300;

  let wrap = $state<HTMLDivElement>();
  let canvas = $state<HTMLCanvasElement>();
  let width = $state(0);
  let hoverCol = $state<number | null>(null);
  let dragging = $state(false);

  // Non-reactive animation/cache state: read and written only from paint()/the flash
  // loop, never from a template, so they don't need to be `$state`.
  let flashStart: number | null = null;
  let flashCols: Set<number> | null = null;
  let rafId = 0;
  let colorCache: { idx: number; free: boolean }[] = [];
  let colorCacheKey = "";

  // Track the ribbon's own on-screen width so `cols` follows a resize (breakpoint
  // change, window resize) rather than a value baked in at mount — same pattern as
  // FatMap.
  $effect(() => {
    if (!wrap) return;
    width = wrap.getBoundingClientRect().width;
    const ro = new ResizeObserver((entries) => { width = entries[0].contentRect.width; });
    ro.observe(wrap);
    return () => ro.disconnect();
  });

  const cols = $derived(Math.max(1, Math.floor(width)));
  const sectorsPerCol = $derived(Math.max(1, Math.ceil(volume.totalSectors / cols)));

  $effect(() => {
    volume.epoch; layers.visible; layers.diff; cols;
    paint();
  });

  // The flash is tied to `volume.cursor` changing (a timeline step being selected or
  // replay moving), not to every epoch bump — reading only `volume.cursor` as a
  // dependency (via `untrack` around the body) keeps that precise.
  $effect(() => {
    volume.cursor;
    untrack(() => startFlash());
    // Stop the fade loop when this effect re-runs or the component is destroyed,
    // so no rAF callback survives into a painted-over or unmounted canvas.
    return () => { if (rafId) { cancelAnimationFrame(rafId); rafId = 0; } };
  });

  function reducedMotion(): boolean {
    return typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  function computeFlashCols(): Set<number> {
    const out = new Set<number>();
    for (const iv of layers.diff) {
      const s0 = Math.floor(iv.start / volume.sectorSize);
      const s1 = Math.max(s0, Math.ceil(iv.end / volume.sectorSize) - 1);
      const c0 = Math.max(0, Math.min(cols - 1, Math.floor(s0 / sectorsPerCol)));
      const c1 = Math.max(0, Math.min(cols - 1, Math.floor(s1 / sectorsPerCol)));
      for (let c = c0; c <= c1; c++) out.add(c);
    }
    return out;
  }

  function startFlash() {
    if (reducedMotion() || !layers.diff.length) return;
    flashCols = computeFlashCols();
    flashStart = performance.now();
    if (rafId) cancelAnimationFrame(rafId);
    const step = () => {
      paint();
      if (flashStart !== null && performance.now() - flashStart < FLASH_MS) {
        rafId = requestAnimationFrame(step);
      } else {
        flashStart = null;
        flashCols = null;
        rafId = 0;
        paint();
      }
    };
    rafId = requestAnimationFrame(step);
  }

  /** First sector owned by a file wins (ownership, not content); else the first
   *  metadata (non-data) sector's region color; else free. */
  function columnColor(startSector: number, endSector: number): { idx: number; free: boolean } {
    let metaIdx: number | null = null;
    for (let s = startSector; s < endSector; s++) {
      const attr = attrAtSector(volume.attribution, s);
      if (attr.ownerPath !== undefined) return { idx: attr.colorIndex, free: false };
      if (!attr.free && metaIdx === null) metaIdx = attr.colorIndex;
    }
    return metaIdx !== null ? { idx: metaIdx, free: false } : { idx: -1, free: true };
  }

  function ensureColors(): { idx: number; free: boolean }[] {
    const key = `${volume.epoch}:${cols}:${sectorsPerCol}`;
    if (key === colorCacheKey) return colorCache;
    const total = volume.totalSectors;
    const out = new Array<{ idx: number; free: boolean }>(cols);
    for (let i = 0; i < cols; i++) {
      const startSector = i * sectorsPerCol;
      out[i] = startSector >= total ? { idx: -1, free: true } : columnColor(startSector, Math.min(total, startSector + sectorsPerCol));
    }
    colorCache = out;
    colorCacheKey = key;
    return out;
  }

  function paint() {
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    const cssWidth = cols;
    canvas.width = Math.max(1, Math.round(cssWidth * dpr));
    canvas.height = Math.max(1, Math.round(HEIGHT * dpr));
    canvas.style.width = `${cssWidth}px`;
    canvas.style.height = `${HEIGHT}px`;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, cssWidth, HEIGHT);

    // Read tokens once per paint (not cached across paints) so a light/dark switch is
    // picked up on the next repaint with no extra wiring.
    const style = getComputedStyle(canvas);
    const hairline = style.getPropertyValue("--hairline").trim();
    const inkMuted = style.getPropertyValue("--ink-muted").trim();
    const focus = style.getPropertyValue("--focus").trim();
    const diff = style.getPropertyValue("--diff").trim();
    const own = (i: number) => style.getPropertyValue(`--own-${i}`).trim();

    const colors = ensureColors();
    for (let i = 0; i < cssWidth; i++) {
      const c = colors[i];
      ctx.fillStyle = c.free ? hairline : own(c.idx);
      ctx.fillRect(i, 0, 1, HEIGHT);
    }

    // Region boundary ticks.
    ctx.fillStyle = inkMuted;
    for (const r of volume.layout) {
      const col = Math.min(cssWidth - 1, Math.floor(r.sectors.start / sectorsPerCol));
      ctx.fillRect(col, 0, 1, HEIGHT);
    }

    // Viewport bracket: top and bottom lines spanning the visible columns.
    const v = layers.visible;
    if (v.end > v.start) {
      const s0 = Math.floor(v.start / volume.sectorSize);
      const s1 = Math.max(s0, Math.ceil(v.end / volume.sectorSize) - 1);
      const c0 = Math.max(0, Math.min(cssWidth - 1, Math.floor(s0 / sectorsPerCol)));
      const c1 = Math.max(c0, Math.min(cssWidth - 1, Math.floor(s1 / sectorsPerCol)));
      ctx.fillStyle = focus;
      ctx.fillRect(c0, 0, c1 - c0 + 1, 2);
      ctx.fillRect(c0, HEIGHT - 2, c1 - c0 + 1, 2);
    }

    // Diff flash: fades out over FLASH_MS, driven by the rAF loop in startFlash().
    if (flashStart !== null && flashCols) {
      const elapsed = performance.now() - flashStart;
      if (elapsed < FLASH_MS) {
        ctx.globalAlpha = 1 - elapsed / FLASH_MS;
        ctx.fillStyle = diff;
        for (const col of flashCols) ctx.fillRect(col, 0, 1, HEIGHT);
        ctx.globalAlpha = 1;
      }
    }
  }

  function colAtX(x: number): number {
    return Math.max(0, Math.min(cols - 1, Math.floor(x)));
  }

  function jumpToColumn(col: number) {
    const total = volume.totalSectors;
    const startSector = Math.min(col * sectorsPerCol, Math.max(0, total - 1));
    selection.jumpTo(startSector * volume.sectorSize);
  }

  function pointFromEvent(e: MouseEvent): number | null {
    if (!canvas) return null;
    const rect = canvas.getBoundingClientRect();
    return colAtX(e.clientX - rect.left);
  }

  function onDown(e: MouseEvent) {
    dragging = true;
    const col = pointFromEvent(e);
    if (col !== null) jumpToColumn(col);
  }
  function onMove(e: MouseEvent) {
    const col = pointFromEvent(e);
    hoverCol = col;
    if (dragging && col !== null) jumpToColumn(col);
  }
  function onUp() { dragging = false; }
  function onLeave() { dragging = false; hoverCol = null; }

  /** Keyboard equivalent of click-and-drag seeking: one column per arrow press,
   *  Home/End for the ends of the disk. */
  function onKey(e: KeyboardEvent) {
    const sector = Math.floor((selection.cursorOffset ?? 0) / volume.sectorSize);
    const cur = Math.max(0, Math.min(cols - 1, Math.floor(sector / sectorsPerCol)));
    let col: number;
    switch (e.key) {
      case "ArrowLeft": col = cur - 1; break;
      case "ArrowRight": col = cur + 1; break;
      case "Home": col = 0; break;
      case "End": col = cols - 1; break;
      default: return;
    }
    e.preventDefault();
    jumpToColumn(Math.max(0, Math.min(cols - 1, col)));
  }

  const caption = $derived.by(() => {
    if (hoverCol === null) return "";
    volume.epoch;
    const total = volume.totalSectors;
    const sector = Math.min(hoverCol * sectorsPerCol, Math.max(0, total - 1));
    const attr: Attr = attrAtSector(volume.attribution, sector);
    return `${volume.adapter.sector.singular} ${sector} · ${attr.ownerPath ?? attr.regionName}`;
  });

  function metaLabel(r: Region): string {
    if (r.kind === "boot") return "boot";
    if (r.kind === "directory") return "root";
    return r.name;
  }

  const metaRegions = $derived.by(() => {
    volume.epoch;
    return volume.layout
      .filter((r) => r.kind !== "data")
      .map((r) => ({ name: r.name, label: metaLabel(r), start: r.sectors.start, colorIndex: attrAtSector(volume.attribution, r.sectors.start).colorIndex }));
  });

  const files = $derived.by(() => {
    volume.epoch;
    const out: { path: string; name: string; colorIndex: number }[] = [];
    const walk = (n: TreeNode) => {
      for (const c of n.children) {
        if (c.isDir) walk(c);
        else out.push({ path: c.path, name: c.name, colorIndex: colorIndexForPath(c.path) });
      }
    };
    walk(buildTree(volume.vol, volume.adapter.owners));
    return out;
  });

  // The family's free count: on FAT the units no file owns, as always; on ext the superblock's,
  // because the inode tables, the bitmaps, and the journal have no owner and are not free.
  const freeLabel = $derived.by(() => { volume.epoch; return freeSpaceLabel(volume.adapter); });
</script>

<svelte:window onmouseup={onUp} />

<section class="ribbon">
  <div class="ribbon-canvas-wrap" bind:this={wrap}>
    <!-- svelte-ignore a11y_mouse_events_have_key_events -->
    <canvas
      bind:this={canvas}
      onmousedown={onDown}
      onmousemove={onMove}
      onmouseleave={onLeave}
      onkeydown={onKey}
      aria-label="Disk ribbon: whole-disk overview, click or drag to seek, arrow keys to move one column"
      tabindex="0"
    ></canvas>
  </div>
  <div class="ribbon-legend">
    {#each metaRegions as r (r.name)}
      <button class="ribbon-swatch" style:--sw="var(--own-{r.colorIndex})" onclick={() => selection.jumpTo(r.start * volume.sectorSize)}>
        <span class="dot"></span>{r.label}
      </button>
    {/each}
    {#each files as f (f.path)}
      <button class="ribbon-swatch" style:--sw="var(--own-{f.colorIndex})" onclick={() => selection.select(f.path)}>
        <span class="dot"></span>{f.name}
      </button>
    {/each}
    <span class="ribbon-free muted">{freeLabel}</span>
    <span class="ribbon-caption mono muted">{caption || " "}</span>
  </div>
</section>
