export interface ByteChangeLike { offset: number; before: Uint8Array; after: Uint8Array }

/** Apply an operation's byte changes to a cached image. Forward writes `after` in
 *  order; reverse writes `before` in reverse order, so reverse∘forward is the identity
 *  even when writes overlap. */
export function applyChanges(buf: Uint8Array, changes: ByteChangeLike[], direction: "forward" | "reverse"): void {
  if (direction === "forward") {
    for (const c of changes) buf.set(c.after, c.offset);
  } else {
    for (let i = changes.length - 1; i >= 0; i--) buf.set(changes[i].before, changes[i].offset);
  }
}

/** Sorted, de-duplicated sector numbers touched by the changes. An empty change
 *  (the core allows a zero-length raw write, even at the very end of the disk)
 *  touches no sector. */
export function changedSectors(changes: ByteChangeLike[], sectorSize: number): number[] {
  const set = new Set<number>();
  for (const c of changes) {
    if (c.after.length === 0) continue;
    const first = Math.floor(c.offset / sectorSize);
    const last = Math.floor((c.offset + c.after.length - 1) / sectorSize);
    for (let s = first; s <= last; s++) set.add(s);
  }
  return [...set].sort((a, b) => a - b);
}

/** Bytes the FAT core re-parses as the boot sector after a raw write. */
export const BOOT_SECTOR_LEN = 512;

/** True when any change starts inside the boot sector. The core may then have adopted
 *  a new geometry, so the store must re-read `geometry` and `layout`. */
export function touchesBootSector(changes: ByteChangeLike[]): boolean {
  return changes.some((c) => c.offset < BOOT_SECTOR_LEN);
}
