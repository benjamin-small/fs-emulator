export interface StringHit { offset: number; length: number; text: string }

const isPrintable = (b: number) => b >= 0x20 && b <= 0x7e;

/** Printable ASCII runs of at least `minRun` bytes within [start, end). */
export function findStrings(buf: Uint8Array, start: number, end: number, minRun = 4, limit = 2000): StringHit[] {
  const hits: StringHit[] = [];
  const stop = Math.min(end, buf.length);
  let runStart = -1;
  const flush = (at: number) => {
    if (runStart >= 0 && at - runStart >= minRun) {
      hits.push({ offset: runStart, length: at - runStart, text: String.fromCharCode(...buf.subarray(runStart, at)) });
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
