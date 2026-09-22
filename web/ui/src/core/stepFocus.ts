/**
 * Where a timeline step's changes begin: the offset the dump scrolls to when someone
 * navigates to that step. The first change is the one the operation started with (the
 * directory entry, then the FAT, then the data), so it is the most explanatory byte to
 * land on. Null means "nothing to focus": no record at all, or a step that changed no
 * bytes, and the caller leaves the dump where it is.
 */
export function stepFocusOffset(rec: { changes: { offset: number }[] } | null | undefined): number | null {
  return rec?.changes.length ? rec.changes[0].offset : null;
}
