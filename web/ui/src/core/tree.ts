import type { ClusterOwner, Volume } from "../lib/wasm";
import { isCorrupt } from "./corrupt";

export interface TreeNode { name: string; path: string; isDir: boolean; size: number; firstCluster: number; children: TreeNode[] }

/** Recursive listing; first clusters come from the owner map (0 for empty files).
 *  `listDir` is gated: while the volume is corrupt (a raw write left the boot sector
 *  unparsable) it throws `CorruptImage`, and this returns an empty root instead of
 *  throwing, so a `$derived` reading it doesn't freeze on the last good tree. */
export function buildTree(vol: Volume, owners: ClusterOwner[]): TreeNode {
  const firstByPath = new Map<string, number>();
  for (const o of owners) if (!firstByPath.has(o.path)) firstByPath.set(o.path, o.firstCluster);
  const walk = (path: string): TreeNode[] =>
    vol.listDir(path).map((e) => {
      const childPath = path === "/" ? `/${e.name}` : `${path}/${e.name}`;
      return { name: e.name, path: childPath, isDir: e.isDir, size: e.size, firstCluster: firstByPath.get(childPath) ?? 0, children: e.isDir ? walk(childPath) : [] };
    });
  let children: TreeNode[];
  try { children = walk("/"); } catch (e) { if (isCorrupt(e)) children = []; else throw e; }
  return { name: "/", path: "/", isDir: true, size: 0, firstCluster: 0, children };
}
