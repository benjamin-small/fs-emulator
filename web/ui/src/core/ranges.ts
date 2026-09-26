/** A run of consecutive sectors (blocks on ext) as the What changed panel labels it. */
export interface SectorRange { label: string; first: number; last: number }

/**
 * Fold sorted, de-duplicated sector numbers (`changedSectors`'s output) into runs of consecutive
 * numbers: `[1, 2, 3, 4, 5, 6, 69, 82, …, 91]` becomes `1–6`, `69`, `82–91`, with an en dash.
 * An ext3 create touches 18 blocks in four runs, so four buttons stand in for eighteen.
 */
export function formatRanges(sorted: readonly number[]): SectorRange[] {
  const runs: { first: number; last: number }[] = [];
  for (const n of sorted) {
    const run = runs[runs.length - 1];
    if (run && n <= run.last + 1) run.last = Math.max(run.last, n);
    else runs.push({ first: n, last: n });
  }
  return runs.map(({ first, last }) => ({ label: first === last ? `${first}` : `${first}–${last}`, first, last }));
}
