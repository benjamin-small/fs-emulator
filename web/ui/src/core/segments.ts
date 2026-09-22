export const BYTES_PER_ROW = 16;

export interface Segment {
  kind: "rows" | "gap";
  startSector: number;
  sectorCount: number;
  firstRow: number;
  rowCount: number;
}

export interface RowRef { segment: Segment; rowInSegment: number }

export function rowsPerSector(sectorSize: number): number { return sectorSize / BYTES_PER_ROW; }

/** Collapse runs of >= minRun consecutive all-zero sectors into one gap row each.
 *  Pinned sectors are always rendered as rows and split any run they fall in. */
export function buildSegments(bits: Uint8Array, opts: { minRun: number; pinned: ReadonlySet<number>; sectorSize?: number }): Segment[] {
  const rps = rowsPerSector(opts.sectorSize ?? 512);
  const n = bits.length;
  const segs: Segment[] = [];
  const runAt = (k: number) => { let j = k; while (j < n && bits[j] === 1 && !opts.pinned.has(j)) j++; return j - k; };
  const collapsibleAt = (k: number) => bits[k] === 1 && !opts.pinned.has(k) && runAt(k) >= opts.minRun;
  let i = 0, row = 0;
  while (i < n) {
    if (collapsibleAt(i)) {
      const len = runAt(i);
      segs.push({ kind: "gap", startSector: i, sectorCount: len, firstRow: row, rowCount: 1 });
      row += 1; i += len;
    } else {
      let k = i + 1;
      while (k < n && !collapsibleAt(k)) k++;
      segs.push({ kind: "rows", startSector: i, sectorCount: k - i, firstRow: row, rowCount: (k - i) * rps });
      row += (k - i) * rps; i = k;
    }
  }
  return segs;
}

export function totalRows(segments: Segment[]): number {
  const last = segments[segments.length - 1];
  return last ? last.firstRow + last.rowCount : 0;
}

/** Binary search the segment containing `row`. */
export function rowAt(segments: Segment[], row: number): RowRef {
  let lo = 0, hi = segments.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (segments[mid].firstRow <= row) lo = mid; else hi = mid - 1;
  }
  const segment = segments[lo];
  return { segment, rowInSegment: Math.min(row - segment.firstRow, segment.rowCount - 1) };
}

export function rowToOffset(segments: Segment[], sectorSize: number, row: number): number {
  const { segment, rowInSegment } = rowAt(segments, row);
  if (segment.kind === "gap") return segment.startSector * sectorSize;
  return segment.startSector * sectorSize + rowInSegment * BYTES_PER_ROW;
}

export function offsetToRow(segments: Segment[], sectorSize: number, offset: number): number {
  const sector = Math.floor(offset / sectorSize);
  let lo = 0, hi = segments.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (segments[mid].startSector <= sector) lo = mid; else hi = mid - 1;
  }
  const seg = segments[lo];
  if (seg.kind === "gap") return seg.firstRow;
  return seg.firstRow + Math.floor((offset - seg.startSector * sectorSize) / BYTES_PER_ROW);
}
