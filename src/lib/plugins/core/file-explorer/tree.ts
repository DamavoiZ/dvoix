import type { VaultEntry } from "../../../api";

export interface TreeNode {
  entry: VaultEntry;
  children: TreeNode[];
}

/**
 * Adds extra entries (from other plugins) to the vault's, without duplicates and
 * sorted by path again, so every folder comes before what is inside it.
 */
export function mergeEntries(entries: VaultEntry[], extra: VaultEntry[]): VaultEntry[] {
  if (extra.length === 0) return entries;
  const seen = new Set(entries.map((e) => e.path));
  const merged = [...entries, ...extra.filter((e) => !seen.has(e.path))];
  return merged.sort((a, b) => (a.path.toLowerCase() < b.path.toLowerCase() ? -1 : a.path.toLowerCase() > b.path.toLowerCase() ? 1 : 0));
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
