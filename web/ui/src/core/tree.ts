import type { Volume } from "../lib/wasm";
import type { UnitOwner } from "../fs/adapter";
import { isCorrupt } from "./corrupt";

export interface TreeNode { name: string; path: string; isDir: boolean; size: number; firstUnit: number | null; children: TreeNode[] }

/** Recursive listing; first units come from the owner map (`null` for the root and for an
 *  empty file, which own no unit). `listDir` is gated: while the volume is corrupt (a raw
 *  write left the on-disk metadata unparsable) it throws `CorruptImage`, and this returns an
 *  empty root instead of throwing, so a `$derived` reading it doesn't freeze on the last good tree. */
export function buildTree(vol: Volume, owners: readonly UnitOwner[]): TreeNode {
  const firstByPath = new Map<string, number>();
  for (const o of owners) if (!firstByPath.has(o.path)) firstByPath.set(o.path, o.firstUnit);
  const walk = (path: string): TreeNode[] =>
    vol.listDir(path).map((e) => {
      const childPath = path === "/" ? `/${e.name}` : `${path}/${e.name}`;
      return { name: e.name, path: childPath, isDir: e.isDir, size: e.size, firstUnit: firstByPath.get(childPath) ?? null, children: e.isDir ? walk(childPath) : [] };
    });
  let children: TreeNode[];
  try { children = walk("/"); } catch (e) { if (isCorrupt(e)) children = []; else throw e; }
  return { name: "/", path: "/", isDir: true, size: 0, firstUnit: null, children };
}
