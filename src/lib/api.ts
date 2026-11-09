// Typed wrappers around the Rust commands in `src-tauri/src/commands.rs`.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface VaultInfo {
  name: string;
  path: string;
}

export interface VaultEntry {
  path: string;
  name: string;
  extension: string;
  isDir: boolean;
  size: number;
  modified: number;
}

export interface SearchHit {
  path: string;
  title: string;
  snippet: string;
}

export interface NoteSummary {
  path: string;
  title: string;
  frontmatter: Record<string, unknown>;
}

export interface TagCount {
  tag: string;
  count: number;
}

/** Everything the graph view needs (see `Index::graph` in `crates/dvoix-core/src/index.rs`). */
export interface GraphData {
  notes: { path: string; title: string; tags: string[] }[];
  /** `target` is the link text as written (lower-cased), not yet resolved to a file. */
  links: { source: string; target: string }[];
}

export type Priority = "high" | "medium" | "low";

/** A markdown checkbox somewhere in the vault (see `crates/dvoix-core/src/tasks.rs`). */
export interface TaskItem {
  path: string;
  noteTitle: string;
  /** 0-based line in the file. */
  line: number;
  done: boolean;
  text: string;
  due: string | null;
  priority: Priority | null;
  tags: string[];
  indent: number;
}

export type ProjectStatus = "active" | "waiting" | "idea" | "done" | "cancelled";

/** A note with `type: projekt` in its frontmatter (see `crates/dvoix-core/src/project.rs`). */
export interface ProjectSummary {
  path: string;
  title: string;
  status: ProjectStatus;
  due: string | null;
  start: string | null;
  priority: Priority | null;
  area: string | null;
  openTasks: number;
  doneTasks: number;
  nextDue: string | null;
}

export interface VaultChange {
  kind: "created" | "modified" | "removed";
  path: string;
}

/** A note rendered for the reading view (see `dvoix_core::markdown::render`). */
export interface RenderedNote {
  /** The body as HTML, without the frontmatter. */
  html: string;
  /** Frontmatter as JSON, shown as properties. */
  frontmatter: Record<string, unknown>;
}

export const api = {
  lastVault: () => invoke<string | null>("last_vault"),
  openVault: (path: string) => invoke<VaultInfo>("open_vault", { path }),
  closeVault: () => invoke<void>("close_vault"),
  listEntries: () => invoke<VaultEntry[]>("list_entries"),
  readFile: (path: string) => invoke<string>("read_file", { path }),
  writeFile: (path: string, content: string) => invoke<VaultEntry>("write_file", { path, content }),
  renderMarkdown: (content: string) => invoke<RenderedNote>("render_markdown", { content }),
  createFile: (path: string, content: string) => invoke<VaultEntry>("create_file", { path, content }),
  createFolder: (path: string) => invoke<VaultEntry>("create_folder", { path }),
  renameEntry: (from: string, to: string) => invoke<VaultEntry>("rename_entry", { from, to }),
  trashEntry: (path: string) => invoke<void>("trash_entry", { path }),
  search: (query: string) => invoke<SearchHit[]>("search", { query }),
  tags: () => invoke<TagCount[]>("tags"),
  notesWithTag: (tag: string) => invoke<NoteSummary[]>("notes_with_tag", { tag }),
  notesWhere: (key: string, value: string) => invoke<NoteSummary[]>("notes_where", { key, value }),
  backlinks: (path: string) => invoke<NoteSummary[]>("backlinks", { path }),
  graph: () => invoke<GraphData>("graph"),
  tasks: () => invoke<TaskItem[]>("tasks"),
  projects: () => invoke<ProjectSummary[]>("projects"),
  setTaskDone: (path: string, line: number, text: string, done: boolean) =>
    invoke<VaultEntry>("set_task_done", { path, line, text, done }),
  readConfig: (path: string) => invoke<string | null>("read_config", { path }),
  writeConfig: (path: string, content: string) => invoke<void>("write_config", { path, content }),
  rebuildIndex: () => invoke<number>("rebuild_index"),
};

export function onVaultChanged(handler: (change: VaultChange) => void): Promise<UnlistenFn> {
  return listen<VaultChange>("vault://changed", (e) => handler(e.payload));
}
