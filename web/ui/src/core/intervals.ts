export interface Interval { start: number; end: number }

export function normalize(list: Interval[]): Interval[] {
  const sorted = list.filter((i) => i.end > i.start).sort((a, b) => a.start - b.start);
  const out: Interval[] = [];
  for (const i of sorted) {
    const last = out[out.length - 1];
    if (last && i.start <= last.end) last.end = Math.max(last.end, i.end);
    else out.push({ ...i });
  }
  return out;
}

export function contains(list: Interval[], offset: number): boolean {
  let lo = 0, hi = list.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    const i = list[mid];
    if (offset < i.start) hi = mid - 1; else if (offset >= i.end) lo = mid + 1; else return true;
  }
  return false;
}

export function intervalsToSectors(list: Interval[], sectorSize: number): Set<number> {
  const s = new Set<number>();
  for (const i of list) for (let x = Math.floor(i.start / sectorSize); x <= Math.floor((i.end - 1) / sectorSize); x++) s.add(x);
  return s;
}
