<script lang="ts">
  import { findStrings, scanChunked, type StringHit } from "../core/strings";
  import { normalize } from "../core/intervals";
  import { attrAtOffset } from "../core/attribution";
  import { getWorkspace } from "../state/workspace.svelte";

  const { volume, selection, layers } = getWorkspace();

  const CHUNK = 1 << 20; // 1 MiB
  const VISIBLE_LIMIT = 500;
  const SCAN_LIMIT = 2000;

  let scope = $state<"visible" | "whole">("visible");
  let minRun = $state(4);
  let scanHits = $state.raw<StringHit[]>([]);
  let scanPct = $state<number | null>(null);

  const clampedMinRun = $derived(Math.max(2, minRun || 2));

  /** Visible-scope hits: printable runs in the currently rendered byte range. */
  const visibleHits: StringHit[] = $derived.by(() => {
    volume.epoch;
    if (!selection.stringsOn || scope !== "visible") return [];
    return findStrings(volume.image, layers.visible.start, layers.visible.end, clampedMinRun, VISIBLE_LIMIT);
  });

  const hits: StringHit[] = $derived(scope === "whole" ? scanHits : visibleHits);

  // Chunked whole-disk scan, one 1 MiB chunk of `scanChunked` per animation frame,
  // skipping chunks whose sectors are all zero. `scanChunked` (src/core/strings.ts)
  // carries a printable run that straddles a chunk boundary into the next chunk
  // instead of splitting it into two hits at the seam. Restarts whenever the toggle,
  // scope, min length, or the volume itself (epoch) changes; deliberately does NOT
  // depend on hover/cursor/selection so it never re-runs on every mouse move. The
  // returned cleanup cancels the pending frame both when the effect re-runs and when
  // the component unmounts.
  $effect(() => {
    const on = selection.stringsOn;
    const wantsWhole = scope === "whole";
    const mr = clampedMinRun;
    volume.epoch;

    if (!on || !wantsWhole) {
      scanHits = [];
      scanPct = null;
      return;
    }

    const image = volume.image;
    const zeros = volume.zeros;
    const sectorSize = volume.sectorSize;
    const isSkippable = (chunkStart: number, chunkEnd: number) => {
      const s0 = Math.floor(chunkStart / sectorSize);
      const s1 = Math.ceil(chunkEnd / sectorSize);
      for (let s = s0; s < s1; s++) if (!zeros[s]) return false;
      return true;
    };
    const gen = scanChunked(image, CHUNK, mr, SCAN_LIMIT, isSkippable);
    let cancelled = false;
    let rafId = 0;

    scanHits = [];
    scanPct = image.length > 0 ? 0 : null;

    function step() {
      if (cancelled) return;
      const r = gen.next();
      if (r.done) {
        scanHits = r.value;
        scanPct = null;
        return;
      }
      scanHits = r.value.hits;
      scanPct = r.value.pct;
      rafId = requestAnimationFrame(step);
    }
    rafId = requestAnimationFrame(step);

    return () => { cancelled = true; if (rafId) cancelAnimationFrame(rafId); };
  });

  // Publish the highlight overlay for HexView. Only ever writes `layers.str`.
  $effect(() => {
    layers.str = selection.stringsOn ? normalize(hits.map((h) => ({ start: h.offset, end: h.offset + h.length }))) : [];
  });

  function ownerOrRegion(offset: number): string {
    const a = attrAtOffset(volume.attribution, offset);
    return a.ownerPath ?? a.regionName;
  }
  function truncate(text: string): string {
    return text.length > 40 ? text.slice(0, 40) + "…" : text;
  }
  function hex(n: number): string { return "0x" + n.toString(16); }
</script>

<section class="panel strings">
  <h2>Strings</h2>
  <label class="toggle">
    <input type="checkbox" bind:checked={selection.stringsOn} />
    Highlight strings
  </label>
  <div class="strings-controls">
    <label class="field">
      Scope
      <select bind:value={scope}>
        <option value="visible">Visible bytes</option>
        <option value="whole">Whole disk</option>
      </select>
    </label>
    <label class="field">
      Min length
      <input class="mono" type="number" min="2" bind:value={minRun} />
    </label>
  </div>
  {#if scanPct !== null}
    <p class="muted">Scanning… {scanPct}%</p>
  {/if}
  {#if hits.length === 0}
    <p class="muted">No printable runs in this range.</p>
  {:else}
    <ul class="hit-list">
      {#each hits as h (h.offset)}
        <li>
          <button class="link" onclick={() => selection.jumpTo(h.offset)}>
            <span class="mono">{hex(h.offset)}</span> · {ownerOrRegion(h.offset)} · <span class="mono">"{truncate(h.text)}"</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</section>
