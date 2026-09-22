export interface StringHit { offset: number; length: number; text: string }

const isPrintable = (b: number) => b >= 0x20 && b <= 0x7e;
const latin1 = new TextDecoder("latin1");

/** Printable ASCII runs of at least `minRun` bytes within [start, end). */
export function findStrings(buf: Uint8Array, start: number, end: number, minRun = 4, limit = 2000): StringHit[] {
  const hits: StringHit[] = [];
  const stop = Math.min(end, buf.length);
  let runStart = -1;
  const flush = (at: number) => {
    if (runStart >= 0 && at - runStart >= minRun) {
      hits.push({ offset: runStart, length: at - runStart, text: latin1.decode(buf.subarray(runStart, at)) });
    }
    runStart = -1;
  };
  for (let i = Math.max(0, start); i < stop; i++) {
    if (isPrintable(buf[i])) { if (runStart < 0) runStart = i; }
    else { flush(i); if (hits.length >= limit) return hits; }
  }
  flush(stop);
  return hits.slice(0, limit);
}

export interface ScanStep { hits: StringHit[]; pct: number }

/** How far back `scanChunked` will walk to find where a run touching a chunk's
 * right edge began, so it can be carried whole into the next chunk instead of
 * being split at the boundary. A run longer than this (or a chunk that is
 * printable all the way back to its own start) is left split at the seam
 * rather than searched backward without bound. */
const MAX_CARRY = 65536;

/**
 * Finds the start of the printable run touching `boundary - 1`, walking back
 * at most `cap` bytes from `boundary` and never past `from`. Returns `null`
 * when the walk cannot establish an exact start within that window — either
 * the run is longer than `cap`, or it runs printable all the way back to
 * `from` (the whole chunk so far) — in which case the caller should give up
 * carrying it over and accept a split at `boundary`.
 */
function tailRunStart(buf: Uint8Array, from: number, boundary: number, cap: number): number | null {
  const stopAt = Math.max(from, boundary - cap);
  let back = boundary;
  while (back > stopAt && isPrintable(buf[back - 1])) back--;
  return back > stopAt ? back : null;
}

/**
 * Chunked equivalent of `findStrings(buf, 0, buf.length, minRun, limit)`:
 * scans the whole buffer `chunkSize` bytes at a time (skipping chunks
 * `isSkippable` marks as empty, e.g. all-zero disk sectors) and yields
 * progress after each chunk so a caller — typically an animation-frame loop —
 * can pace the scan across frames instead of blocking on the whole buffer at
 * once.
 *
 * Scanning each chunk independently with `findStrings` would split (or, at
 * `minRun`, lose) any printable run that straddles a chunk boundary, since
 * neither side's call sees past its own slice. Instead, after processing a
 * chunk, if the bytes touching its right edge are still part of a run that
 * might continue, that chunk is re-cut short at `tailRunStart` — the run is
 * left out of this chunk's `findStrings` call entirely — so the next chunk
 * starts exactly at the run's beginning and finds it whole, merged with
 * whatever follows.
 *
 * Draining the generator to completion (iterating until `done`, or via the
 * returned value) yields hits identical to a single `findStrings` call over
 * the whole buffer.
 */
export function* scanChunked(
  buf: Uint8Array,
  chunkSize: number,
  minRun = 4,
  limit = 2000,
  isSkippable?: (chunkStart: number, chunkEnd: number) => boolean,
): Generator<ScanStep, StringHit[], void> {
  const hits: StringHit[] = [];
  const total = buf.length;
  let pos = 0;
  while (pos < total && hits.length < limit) {
    const chunkEnd = Math.min(total, pos + chunkSize);
    if (isSkippable?.(pos, chunkEnd)) {
      pos = chunkEnd;
      yield { hits: hits.slice(), pct: Math.floor((chunkEnd / total) * 100) };
      continue;
    }
    let scanEnd = chunkEnd;
    if (chunkEnd < total) {
      const runStart = tailRunStart(buf, pos, chunkEnd, MAX_CARRY);
      if (runStart !== null) scanEnd = runStart;
    }
    hits.push(...findStrings(buf, pos, scanEnd, minRun, limit - hits.length));
    pos = scanEnd;
    yield { hits: hits.slice(), pct: Math.floor((chunkEnd / total) * 100) };
  }
  return hits;
}
