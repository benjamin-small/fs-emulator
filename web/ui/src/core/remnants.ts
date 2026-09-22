import type { ClusterOwner, FatEntry, Geometry, Volume } from "../lib/wasm";
import { clusterByteRange } from "./attribution";
import { slotOffset } from "./direntry";
import { normalize, type Interval } from "./intervals";

const ENTRY = 32;

/** Byte ranges of deleted directory slots (root + every reachable subdirectory) and
 *  free clusters that still hold non-zero bytes (data left behind by a delete). */
export function findRemnants(vol: Volume, g: Geometry, fat: FatEntry[], owners: ClusterOwner[], zeros: Uint8Array): Interval[] {
  const out: Interval[] = [];

  const walk = (dir: string) => {
    let entries;
    try { entries = vol.rawDirEntries(dir); } catch { return; }
    for (let i = 0; i < entries.length; i++) {
      const e = entries[i];
      if (e.kind === "deleted") {
        const start = slotOffset(g, fat, owners, dir, i);
        out.push({ start, end: start + ENTRY });
      } else if (e.kind === "short" && e.isDir && e.name !== "." && e.name !== "..") {
        walk(dir === "/" ? `/${e.name}` : `${dir}/${e.name}`);
      }
    }
  };
  walk("/");

  const sectorsPerCluster = g.sectorsPerCluster;
  for (let c = 2; c <= g.clusterCount + 1; c++) {
    if (fat[c]?.kind !== "free") continue;
    const firstSector = clusterByteRange(g, c).start / g.bytesPerSector;
    let dirty = false;
    for (let s = firstSector; s < firstSector + sectorsPerCluster; s++) {
      if (zeros[s] === 0) { dirty = true; break; }
    }
    if (dirty) out.push(clusterByteRange(g, c));
  }

  return normalize(out);
}
