import type { VaultEntry } from "../../api";

export interface TreeNode {
  entry: VaultEntry;
  children: TreeNode[];
}

/**
 * Compares the names of the files. One collator for all of them:
 * `localeCompare` with options builds a new one for every call.
 */
const byName = new Intl.Collator("en", { numeric: true });

/**
 * Adds extra entries (from other plugins) to the vault's, without duplicates and
 * sorted by path again, so every folder comes before what is inside it.
 */
export function mergeEntries(entries: VaultEntry[], extra: VaultEntry[]): VaultEntry[] {
  if (extra.length === 0) return entries;
  const seen = new Set(entries.map((e) => e.path));
  const merged = [...entries, ...extra.filter((e) => !seen.has(e.path))];
  // The key is built once per entry and not twice for every comparison.
  const keyed = merged.map((entry) => ({ entry, key: entry.path.toLowerCase() }));
  keyed.sort((a, b) => (a.key < b.key ? -1 : a.key > b.key ? 1 : 0));
  return keyed.map((k) => k.entry);
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
    nodes.sort((a, b) => Number(b.entry.isDir) - Number(a.entry.isDir) || byName.compare(a.entry.name, b.entry.name));
    nodes.forEach((n) => sort(n.children));
  };
  sort(root);
  return root;
}

/**
 * A folder with more than this many entries is drawn in steps: all of them
 * as DOM rows at once would freeze the tree for a moment on every open.
 */
export const FOLDER_PAGE = 500;

/**
 * The slice of a folder's children to draw. Below the page size the list
 * itself is returned, so its identity (and the tree's DOM) stays stable.
 */
export function visibleChildren(children: TreeNode[], limit: number): TreeNode[] {
  return children.length <= limit ? children : children.slice(0, limit);
}
