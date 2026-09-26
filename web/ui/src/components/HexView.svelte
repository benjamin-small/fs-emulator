<script lang="ts">
  import { untrack } from "svelte";
  import { getWorkspace } from "../state/workspace.svelte";
  import { attrAtOffset } from "../core/attribution";
  import { keepScroll } from "../core/keepScroll";
  import { BYTES_PER_ROW, buildSegments, offsetToRow, rowAt, rowToOffset, totalRows, type Segment } from "../core/segments";
  import { contains } from "../core/intervals";
  import { parseAddr } from "../shell/addr";
  import { unitIsSector } from "../fs/adapter";
  import HexRow from "./HexRow.svelte";

  const ws = getWorkspace();
  const { volume, selection, layers } = ws;

  const ROW_H = 17, OVERSCAN = 10, MIN_RUN = 8, MAX_SPACER = 10_000_000;
  let container = $state<HTMLDivElement>();
  let scrollTop = $state(0);
  let height = $state(600);

  const segments: Segment[] = $derived((volume.epoch, buildSegments(volume.zeros, { minRun: MIN_RUN, pinned: layers.pinnedSectors, sectorSize: volume.sectorSize })));
  const rows = $derived(totalRows(segments));
  const fullHeight = $derived(rows * ROW_H);
  const spacer = $derived(Math.min(fullHeight, MAX_SPACER));
  const scale = $derived(fullHeight > 0 ? spacer / fullHeight : 1);
  const firstRow = $derived(Math.max(0, Math.floor(scrollTop / scale / ROW_H) - OVERSCAN));
  const count = $derived(Math.ceil(height / ROW_H) + 2 * OVERSCAN);
  // Named `rowsInView`, not `window`: a local named `window` would shadow the global
  // `window` object that the "g" jump prompt below needs.
  //
  // `volume.image`, `volume.zeros`, and `volume.history` are `$state.raw` and get
  // patched in place (no reassignment), so merely reading them here does not by
  // itself cause this block to re-run after a mutation — Svelte only re-derives when
  // a tracked dependency's *value* changes, and `firstRow`/`rows`/`count` can easily
  // come out numerically unchanged across an edit. Reading `volume.epoch` (bumped on
  // every `run`/`seek`/`format`/`load`) forces a fresh pass so cell bytes, diff/sel
  // highlighting, and attribution never go stale between real scroll events.
  const rowsInView = $derived.by(() => {
    volume.epoch;
    const out = [];
    for (let r = firstRow; r < Math.min(rows, firstRow + count); r++) out.push(describeRow(r));
    return out;
  });

  $effect(() => { layers.visible = { start: rowToOffset(segments, volume.sectorSize, firstRow), end: rowToOffset(segments, volume.sectorSize, Math.min(rows - 1, firstRow + count)) + BYTES_PER_ROW }; });

  // Scroll only when a *new* jump is requested. `segments` changes on every cursor
  // click, selection, gap expansion, diff and epoch bump, so tracking it here would
  // re-scroll the dump to a stale target; the nonce gates that, and `untrack` keeps
  // everything the scroll math reads out of this effect's dependencies.
  let lastNonce = -1;
  $effect(() => {
    const t = selection.scrollTarget;
    if (!t || !container || t.nonce === lastNonce) return;
    lastNonce = t.nonce;
    untrack(() => {
      const row = offsetToRow(segments, volume.sectorSize, t.offset);
      container!.scrollTop = Math.max(0, (row - 3) * ROW_H * scale);
    });
  });

  function describeRow(r: number) {
    const { segment, rowInSegment } = rowAt(segments, r);
    if (segment.kind === "gap") {
      const bytes = segment.sectorCount * volume.sectorSize;
      return { kind: "gap" as const, key: r, segment, text: `· · · ${segment.sectorCount.toLocaleString()} empty ${volume.adapter.sector.plural} (${fmtBytes(bytes)}) · · ·` };
    }
    const offset = segment.startSector * volume.sectorSize + rowInSegment * BYTES_PER_ROW;
    const attr = attrAtOffset(volume.attribution, offset);
    const bytes = volume.image.subarray(offset, offset + BYTES_PER_ROW);
    const rec = volume.history[volume.cursor];
    const cells = Array.from(bytes, (b, i) => {
      const off = offset + i;
      let cls = "b";
      if (layers.str.length && contains(layers.str, off)) cls += " is-str";
      if (contains(layers.sel, off)) cls += " is-sel";
      if (contains(layers.diff, off)) cls += " is-diff";
      if (layers.remnant.length && contains(layers.remnant, off)) cls += " is-remnant";
      if (selection.cursorOffset === off) cls += " is-cur";
      let title = "";
      if (cls.includes("is-diff") && rec) { const before = beforeByte(rec, off); if (before !== null) title = `before 0x${before.toString(16).padStart(2, "0")} → after 0x${b.toString(16).padStart(2, "0")}`; }
      return { hex: b.toString(16).padStart(2, "0"), asc: b >= 0x20 && b <= 0x7e ? String.fromCharCode(b) : "·", cls, title };
    });
    const sectorStart = offset % volume.sectorSize === 0;
    // `unitStartsAt` is an adapter method (plain caches): `rowsInView`, the only caller of
    // `describeRow`, reads `volume.epoch` first.
    const { unit, sector } = volume.adapter;
    const unitStart = attr.unit !== undefined && sectorStart && volume.adapter.unitStartsAt(attr.sector);
    const label = unitStart ? `${unit.singular} ${attr.unit}${attr.ownerPath ? ` · ${attr.ownerPath}` : ""}` : sectorStart ? `${sector.singular} ${attr.sector}${attr.unit === undefined ? ` · ${attr.regionName}` : ""}` : "";
    return { kind: "row" as const, key: r, offset, attr, cells, sectorStart, unitStart, label };
  }

  function beforeByte(rec: { changes: { offset: number; before: Uint8Array }[] }, off: number): number | null {
    for (const c of rec.changes) if (off >= c.offset && off < c.offset + c.before.length) return c.before[off - c.offset];
    return null;
  }

  function fmtBytes(n: number) { return n >= 1 << 20 ? `${(n / (1 << 20)).toFixed(1)} MB` : n >= 1024 ? `${(n / 1024).toFixed(1)} KB` : `${n} B`; }

  function onKey(e: KeyboardEvent) {
    const cur = selection.cursorOffset ?? layers.visible.start;
    const max = volume.image.length - 1;
    const move = (d: number) => { selection.jumpTo(Math.max(0, Math.min(max, cur + d))); e.preventDefault(); };
    switch (e.key) {
      case "ArrowLeft": return move(-1); case "ArrowRight": return move(1);
      case "ArrowUp": return move(-BYTES_PER_ROW); case "ArrowDown": return move(BYTES_PER_ROW);
      case "PageUp": return move(-Math.floor(height / ROW_H) * BYTES_PER_ROW); case "PageDown": return move(Math.floor(height / ROW_H) * BYTES_PER_ROW);
      case "Home": return move(-cur); case "End": return move(max - cur);
      case "g": { const v = globalThis.prompt(`Jump to offset (${jumpForms()})`); if (v) jump(v); return; }
      case "s": selection.stringsOn = !selection.stringsOn; return;
    }
  }
  /** The `g` prompt's forms: FAT `0x…, decimal, s:sector, c:cluster`; where the unit is the
   *  sector (ext) the one noun: `0x…, decimal, b:block`. */
  function jumpForms(): string {
    const { sector, unit } = volume.adapter;
    const unitForm = unitIsSector(volume.adapter) ? "" : `, ${unit.letter}:${unit.singular}`;
    return `0x…, decimal, ${sector.letter}:${sector.singular}${unitForm}`;
  }

  function jump(v: string) {
    let off: number;
    try { off = parseAddr(v, volume.adapter); } catch { return; } // the prompt ignores bad input silently, as before
    if (off >= 0 && off < volume.image.length) selection.jumpTo(off);
  }
  function onOver(e: MouseEvent) { const t = (e.target as HTMLElement).closest<HTMLElement>("[data-off]"); selection.hoverOffset = t ? Number(t.dataset.off) : null; }
  function onClick(e: MouseEvent) { const t = (e.target as HTMLElement).closest<HTMLElement>("[data-off]"); if (t) selection.cursorOffset = Number(t.dataset.off); }
</script>

<!-- Hover reflects mouse position only; the grid itself is keyboard-operable via
     onkeydown (arrows/Home/End/PageUp/PageDown/g/s) and tabindex, so there is no
     keyboard equivalent for per-byte hover to pair with mouseover. -->
<!-- svelte-ignore a11y_mouse_events_have_key_events -->
<!-- `keepScroll`: hiding the tab zeroes the dump's scroll; showing it again puts it back. -->
<div class="hexview panel" use:keepScroll={ws.active} bind:this={container} bind:clientHeight={height} onscroll={(e) => (scrollTop = (e.currentTarget as HTMLDivElement).scrollTop)} onmouseover={onOver} onmouseleave={() => (selection.hoverOffset = null)} onclick={onClick} onkeydown={onKey} tabindex="0" role="grid" aria-label="Disk bytes">
  {#if volume.image.length === 0}
    <p class="muted">Format a disk or load an image to start.</p>
  {:else}
    <div class="spacer" style:height="{spacer}px">
      <div class="window" style:transform="translateY({firstRow * ROW_H * scale}px)">
        {#each rowsInView as r (r.key)}
          {#if r.kind === "gap"}
            <button class="gap mono" onclick={() => selection.expandGap(r.segment.startSector)}>{r.text}</button>
          {:else}
            <HexRow offset={r.offset} attr={r.attr} cells={r.cells} sectorStart={r.sectorStart} unitStart={r.unitStart} label={r.label} />
          {/if}
        {/each}
      </div>
    </div>
  {/if}
</div>
