import type { ClusterOwner, Volume } from "../lib/wasm";

export interface TreeNode { name: string; path: string; isDir: boolean; size: number; firstCluster: number; children: TreeNode[] }

/** Recursive listing; first clusters come from the owner map (0 for empty files). */
export function buildTree(vol: Volume, owners: ClusterOwner[]): TreeNode {
  const firstByPath = new Map<string, number>();
  for (const o of owners) if (!firstByPath.has(o.path)) firstByPath.set(o.path, o.firstCluster);
  const walk = (path: string): TreeNode[] =>
    vol.listDir(path).map((e) => {
      const childPath = path === "/" ? `/${e.name}` : `${path}/${e.name}`;
      return { name: e.name, path: childPath, isDir: e.isDir, size: e.size, firstCluster: firstByPath.get(childPath) ?? 0, children: e.isDir ? walk(childPath) : [] };
    });
  return { name: "/", path: "/", isDir: true, size: 0, firstCluster: 0, children: walk("/") };
}
