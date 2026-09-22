import type { ClusterOwner, FatEntry, Geometry, RawEntry, Volume } from "../lib/wasm";
import { clusterByteRange } from "./attribution";
import { buildChain } from "./fatchain";

const ENTRY = 32;

export function findEntrySlots(vol: Volume, g: Geometry, fat: FatEntry[], owners: ClusterOwner[], path: string): { start: number; end: number } | null {
  if (path === "/") return null;
  const cut = path.lastIndexOf("/");
  const parent = cut === 0 ? "/" : path.slice(0, cut);
  const name = path.slice(cut + 1).toUpperCase();
  let entries: RawEntry[];
  try { entries = vol.rawDirEntries(parent); } catch { return null; }
  let lfnText = "", lfnStart = -1;
  for (let i = 0; i < entries.length; i++) {
    const e = entries[i];
    if (e.kind === "lfn") { if (e.isLast) { lfnText = ""; lfnStart = i; } lfnText = e.text + lfnText; continue; }
    if (e.kind === "short") {
      const matches = e.name.toUpperCase() === name || lfnText.toUpperCase() === name;
      if (matches) {
        const first = lfnStart >= 0 && lfnText ? lfnStart : i;
        return { start: slotOffset(g, fat, owners, parent, first), end: slotOffset(g, fat, owners, parent, i) + ENTRY };
      }
    }
    if (e.kind === "free") break;
    lfnText = ""; lfnStart = -1;
  }
  return null;
}

function slotOffset(g: Geometry, fat: FatEntry[], owners: ClusterOwner[], dir: string, slot: number): number {
  if (dir === "/") return g.firstRootDirSector * g.bytesPerSector + slot * ENTRY;
  const owner = owners.find((o) => o.path === dir);
  const chain = owner ? buildChain(fat, owner.firstCluster) : [];
  const perCluster = (g.bytesPerSector * g.sectorsPerCluster) / ENTRY;
  const cluster = chain[Math.floor(slot / perCluster)];
  return clusterByteRange(g, cluster).start + (slot % perCluster) * ENTRY;
}
