// Turns the notes, tags and links of the vault into the nodes and lines of the
// graph view, according to the settings. Pure functions without UI, so the
// rules (which lines exist, what is filtered out) stay readable on their own.
import type { GraphData, VaultEntry } from "../../../api";
import type { ColorBy, GraphSettings } from "./settings";

/** The settings that change which nodes and lines exist (the rest only changes looks). */
export type ModelSettings = Pick<
  GraphSettings,
  "links" | "folders" | "tags" | "tagMode" | "filter" | "orphans" | "attachments" | "unresolved"
>;

export type NodeKind = "note" | "attachment" | "unresolved" | "folder" | "tag";
export type EdgeKind = "link" | "folder" | "tag";

export interface GraphNode {
  /** Unique: the path for files, `folder:<path>`, `tag:<tag>` or `unresolved:<target>` otherwise. */
  id: string;
  kind: NodeKind;
  label: string;
  /** Vault path for notes and attachments; folder path for folders; link text for unresolved notes. */
  path: string;
  /** First tag of a note; the tag itself for a tag node. */
  tag: string | null;
  /** Number of lines at this node. */
  degree: number;
}

export interface GraphEdge {
  source: string;
  target: string;
  kind: EdgeKind;
}

export interface GraphModel {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

/**
 * Above this many notes a tag is drawn as a node even in "shared" mode:
 * lines between every pair would be n² lines and only make a hairball.
 */
export const SHARED_TAG_MAX = 40;

interface Item {
  path: string;
  title: string;
  tags: string[];
}

export function buildGraph(data: GraphData, entries: VaultEntry[], s: ModelSettings, vaultName: string): GraphModel {
  const matches = filterFn(s.filter);
  const resolve = resolver(entries);
  const noteSet = new Set(data.notes.map((n) => n.path));

  const nodes = new Map<string, GraphNode>();
  const items = new Map<string, Item>();
  const edges: GraphEdge[] = [];
  const edgeKeys = new Set<string>();

  const addNode = (node: Omit<GraphNode, "degree" | "tag">, item?: Item) => {
    if (!nodes.has(node.id)) nodes.set(node.id, { ...node, tag: item?.tags[0] ?? (node.kind === "tag" ? node.path : null), degree: 0 });
    if (item) items.set(node.id, item);
    return node.id;
  };
  const addEdge = (source: string, target: string, kind: EdgeKind, undirected = false) => {
    if (source === target) return;
    const key = undirected && source > target ? `${target}\0${source}` : `${source}\0${target}`;
    if (edgeKeys.has(key)) return;
    edgeKeys.add(key);
    edges.push({ source, target, kind });
  };

  for (const n of data.notes) {
    const item = { path: n.path, title: n.title, tags: n.tags };
    if (matches(item)) addNode({ id: n.path, kind: "note", label: n.title, path: n.path }, item);
  }

  // Files that are not notes: with "attachments" on, all of them, not only linked ones.
  const attachment = (path: string): string | undefined => {
    if (!s.attachments) return undefined;
    const item = { path, title: fileName(path), tags: [] };
    return matches(item) ? addNode({ id: path, kind: "attachment", label: item.title, path }, item) : undefined;
  };
  if (s.attachments) for (const e of entries) if (!e.isDir && !noteSet.has(e.path)) attachment(e.path);

  if (s.links) {
    for (const link of data.links) {
      if (!nodes.has(link.source)) continue;
      const path = resolve(link.target);
      let target: string | undefined;
      if (path && noteSet.has(path)) target = nodes.has(path) ? path : undefined;
      else if (path) target = attachment(path);
      else if (s.unresolved && matches({ path: link.target, title: link.target, tags: [] })) {
        target = addNode({ id: `unresolved:${link.target}`, kind: "unresolved", label: link.target, path: link.target });
      }
      if (target) addEdge(link.source, target, "link");
    }
  }

  // Taken before folders and tags add their own nodes.
  const files = [...nodes.values()].filter((n) => n.kind === "note" || n.kind === "attachment");

  if (s.folders) {
    const folderNode = (path: string): string => {
      const id = `folder:${path}`;
      if (nodes.has(id)) return id;
      addNode({ id, kind: "folder", label: path ? fileName(path) : vaultName, path });
      if (path) addEdge(id, folderNode(parentOf(path)), "folder");
      return id;
    };
    for (const f of files) addEdge(f.id, folderNode(parentOf(f.path)), "folder");
  }

  if (s.tags) {
    const byTag = new Map<string, string[]>();
    for (const f of files) {
      for (const tag of items.get(f.id)?.tags ?? []) {
        const list = byTag.get(tag);
        if (list) list.push(f.id);
        else byTag.set(tag, [f.id]);
      }
    }
    for (const [tag, ids] of byTag) {
      if (s.tagMode === "shared" && ids.length <= SHARED_TAG_MAX) {
        for (let i = 0; i < ids.length; i++) for (let j = i + 1; j < ids.length; j++) addEdge(ids[i], ids[j], "tag", true);
      } else {
        const id = addNode({ id: `tag:${tag}`, kind: "tag", label: `#${tag}`, path: tag });
        for (const f of ids) addEdge(f, id, "tag");
      }
    }
  }

  for (const e of edges) {
    nodes.get(e.source)!.degree++;
    nodes.get(e.target)!.degree++;
  }
  let list = [...nodes.values()];
  if (!s.orphans) list = list.filter((n) => n.degree > 0);
  return { nodes: list, edges };
}

/**
 * Color groups: the top folder or the first tag of each node, numbered by how
 * often they occur (0 = most frequent), so the palette goes to the big groups.
 */
export function colorGroups(nodes: GraphNode[], by: ColorBy): Map<string, number> {
  const groupOf = new Map<string, string>();
  const counts = new Map<string, number>();
  if (by === "none") return new Map();
  for (const n of nodes) {
    const g = by === "tag" ? n.tag : folderGroup(n);
    if (g === null) continue;
    groupOf.set(n.id, g);
    counts.set(g, (counts.get(g) ?? 0) + 1);
  }
  const order = [...counts].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).map(([g]) => g);
  const index = new Map(order.map((g, i) => [g, i]));
  return new Map([...groupOf].map(([id, g]) => [id, index.get(g)!]));
}

function folderGroup(n: GraphNode): string | null {
  if (n.kind === "tag" || n.kind === "unresolved") return null;
  const top = n.path.split("/")[0];
  // A file in the root has no folder; a top-level folder node is its own group.
  if (n.kind === "folder") return n.path ? top : null;
  return n.path.includes("/") ? top : null;
}

/**
 * Resolves link text the way `vault.resolveLink` does (exact path, path
 * without `.md`, then file name), but with lookup tables, because the graph
 * resolves every link of the vault at once.
 */
function resolver(entries: VaultEntry[]): (target: string) => string | undefined {
  const byPath = new Map<string, string>();
  const byName = new Map<string, string>();
  for (const e of entries) {
    if (e.isDir) continue;
    byPath.set(e.path.toLowerCase(), e.path);
    const name = e.name.toLowerCase();
    if (!byName.has(name)) byName.set(name, e.path);
  }
  return (target) => {
    const t = target.toLowerCase();
    return byPath.get(t) ?? byPath.get(`${t}.md`) ?? byName.get(t) ?? byName.get(`${t}.md`);
  };
}

/**
 * The filter field: every term must occur in the path, the title or as `#tag`;
 * a term starting with `-` hides what it matches. Case-insensitive.
 */
export function filterFn(filter: string): (item: Item) => boolean {
  const terms = filter.toLowerCase().split(/\s+/).filter((t) => t && t !== "-");
  if (!terms.length) return () => true;
  return (item) => {
    const hay = `${item.path}\n${item.title}\n${item.tags.map((t) => `#${t}`).join(" ")}`.toLowerCase();
    return terms.every((t) => (t.startsWith("-") ? !hay.includes(t.slice(1)) : hay.includes(t)));
  };
}

function fileName(path: string): string {
  return path.split("/").pop() ?? path;
}

function parentOf(path: string): string {
  const i = path.lastIndexOf("/");
  return i < 0 ? "" : path.slice(0, i);
}
