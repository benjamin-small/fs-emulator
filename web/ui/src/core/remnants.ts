import type { ClusterOwner, FatEntry, Geometry, Volume } from "../lib/wasm";
import { clusterByteRange } from "./attribution";
import { slotOffset, walkEntries } from "./direntry";
import { normalize, type Interval } from "./intervals";

const ENTRY = 32;
const MAX_DEPTH = 64;

/** Byte ranges of deleted directory slots (root + every reachable subdirectory) and
 *  free clusters that still hold non-zero bytes (data left behind by a delete). */
export function findRemnants(vol: Volume, g: Geometry, fat: FatEntry[], owners: ClusterOwner[], zeros: Uint8Array): Interval[] {
  const out: Interval[] = [];

  // Guards against a corrupt/crafted image where a subdirectory entry's first
  // cluster points back at an ancestor (or itself): track visited first-clusters
  // so such a cycle is skipped rather than recursed into forever, with a depth
  // cap as a belt-and-braces backstop in case cluster resolution is ever fooled.
  const visited = new Set<number>();

  const walk = (dir: string, depth: number) => {
    if (depth > MAX_DEPTH) return;
    let entries;
    // Deliberately broad, as in core/direntry.ts: this walk runs inside a `$derived`, and a
    // directory that has gone away (NotFound after a delete) or become unreadable
    // (CorruptImage after a raw write) must be skipped, not thrown from.
    // See tests/corruption.test.ts.
    try { entries = vol.rawDirEntries(dir); } catch { return; }
    walkEntries(entries, (i, e, longName) => {
      if (e.kind === "deleted") {
        const start = slotOffset(g, fat, owners, dir, i);
        if (start >= 0) out.push({ start, end: start + ENTRY }); // -1: slot past the chain's end
        return false;
      }
      if (e.kind === "short" && e.isDir && e.name !== "." && e.name !== "..") {
        const childName = longName || e.name;
        const childPath = dir === "/" ? `/${childName}` : `${dir}/${childName}`;
        const owner = owners.find((o) => o.path === childPath);
        const firstCluster = owner ? owner.firstCluster : e.firstCluster;
        if (firstCluster >= 2 && !visited.has(firstCluster)) {
          visited.add(firstCluster);
          walk(childPath, depth + 1);
        }
      }
      return false;
    });
  };
  walk("/", 0);

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
