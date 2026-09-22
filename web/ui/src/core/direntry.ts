import type { ClusterOwner, FatEntry, Geometry, RawEntry, Volume } from "../lib/wasm";
import { clusterByteRange } from "./attribution";
import { buildChain } from "./fatchain";

const ENTRY = 32;

/**
 * Walks raw directory entries, accumulating the preceding LFN text (if any) for
 * each non-`lfn` entry so callers don't have to reimplement VFAT's
 * last-entry-first-on-disk assembly. `cb` is called once per non-`lfn` entry with
 * its index, the entry itself, the accumulated long name ("" if the entry has no
 * LFN), and the index of the first slot of the group (the first LFN entry, or the
 * entry's own index when there is no LFN) — the same "first" used for
 * `slotOffset` ranges. Stops at a `free` entry (end of the directory), or early
 * when `cb` returns `true`.
 */
export function walkEntries(entries: RawEntry[], cb: (i: number, e: RawEntry, longName: string, firstIndex: number) => boolean | void): void {
  let lfnText = "", lfnStart = -1;
  for (let i = 0; i < entries.length; i++) {
    const e = entries[i];
    if (e.kind === "lfn") { if (e.isLast) { lfnText = ""; lfnStart = i; } lfnText = e.text + lfnText; continue; }
    const firstIndex = lfnStart >= 0 && lfnText ? lfnStart : i;
    const stop = cb(i, e, lfnText, firstIndex);
    if (e.kind === "free") break;
    lfnText = ""; lfnStart = -1;
    if (stop) break;
  }
}

export function findEntrySlots(vol: Volume, g: Geometry, fat: FatEntry[], owners: ClusterOwner[], path: string): { start: number; end: number } | null {
  if (path === "/") return null;
  const cut = path.lastIndexOf("/");
  const parent = cut === 0 ? "/" : path.slice(0, cut);
  const name = path.slice(cut + 1).toUpperCase();
  let entries: RawEntry[];
  try { entries = vol.rawDirEntries(parent); } catch { return null; }
  let result: { start: number; end: number } | null = null;
  walkEntries(entries, (i, e, longName, firstIndex) => {
    if (e.kind !== "short") return false;
    const matches = e.name.toUpperCase() === name || longName.toUpperCase() === name;
    if (!matches) return false;
    const start = slotOffset(g, fat, owners, parent, firstIndex);
    const last = slotOffset(g, fat, owners, parent, i);
    // A slot past the end of the parent's cluster chain has no byte range (a
    // truncated or corrupt directory); report "no range" rather than a bogus one.
    if (start < 0 || last < 0) return true;
    result = { start, end: last + ENTRY };
    return true;
  });
  return result;
}

/** Byte offset of directory slot `slot` in `dir`, or -1 when the slot falls past
 *  the end of that directory's cluster chain. */
export function slotOffset(g: Geometry, fat: FatEntry[], owners: ClusterOwner[], dir: string, slot: number): number {
  if (dir === "/") return g.firstRootDirSector * g.bytesPerSector + slot * ENTRY;
  const owner = owners.find((o) => o.path === dir);
  const chain = owner ? buildChain(fat, owner.firstCluster) : [];
  const perCluster = (g.bytesPerSector * g.sectorsPerCluster) / ENTRY;
  const cluster = chain[Math.floor(slot / perCluster)];
  if (cluster === undefined) return -1;
  return clusterByteRange(g, cluster).start + (slot % perCluster) * ENTRY;
}
