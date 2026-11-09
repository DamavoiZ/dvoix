import type { VaultEntry } from "../../../api";

export interface TreeNode {
  entry: VaultEntry;
  children: TreeNode[];
}

/** Builds a folder tree from the flat entry list; folders first, then by name. */
export function buildTree(entries: VaultEntry[]): TreeNode[] {
  const root: TreeNode[] = [];
  const folders = new Map<string, TreeNode>();
  for (const entry of entries) {
    const node: TreeNode = { entry, children: [] };
    if (entry.isDir) folders.set(entry.path, node);
    const parent = entry.path.includes("/") ? entry.path.slice(0, entry.path.lastIndexOf("/")) : "";
    (parent ? folders.get(parent)?.children : root)?.push(node);
  }
  const sort = (nodes: TreeNode[]) => {
    nodes.sort((a, b) => Number(b.entry.isDir) - Number(a.entry.isDir) || a.entry.name.localeCompare(b.entry.name, "de", { numeric: true }));
    nodes.forEach((n) => sort(n.children));
  };
  sort(root);
  return root;
}
