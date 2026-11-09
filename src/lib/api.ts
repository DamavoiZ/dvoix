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

/** One file with changes (see `git::ChangedFile` in `crates/dvoix-core/src/git.rs`). */
export interface GitChangedFile {
  /** Path relative to the vault root. */
  path: string;
  /** Letter of the change: "M", "A", "D", "R" or "?" for files git does not know yet. */
  status: string;
}

/** The repository the vault is in (see `git::RepoInfo`). */
export interface GitRepoInfo {
  root: string;
  branch: string;
  upstream: string | null;
  remote: string | null;
  ahead: number;
  behind: number;
  vaultIsRoot: boolean;
}

/** The state of the repository (see `git::GitStatus`). */
export interface GitStatus {
  /** `null` when the vault is not inside a repository. */
  repo: GitRepoInfo | null;
  staged: GitChangedFile[];
  unstaged: GitChangedFile[];
  untracked: GitChangedFile[];
}

/** One commit of the history (see `git::Commit`). */
export interface GitCommit {
  id: string;
  shortId: string;
  author: string;
  date: string;
  message: string;
}

/** One branch (see `git::Branch`). */
export interface GitBranch {
  name: string;
  remote: boolean;
  current: boolean;
}

/** The two versions of a file that a diff compares (see `git::FileDiff`). */
export interface GitFileDiff {
  old: string;
  new: string;
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

  gitStatus: () => invoke<GitStatus>("git_status"),
  gitStage: (paths: string[]) => invoke<void>("git_stage", { paths }),
  gitUnstage: (paths: string[]) => invoke<void>("git_unstage", { paths }),
  gitStageAll: () => invoke<void>("git_stage_all"),
  gitDiscard: (path: string, untracked: boolean) => invoke<void>("git_discard", { path, untracked }),
  gitCommit: (message: string, stageAll: boolean) => invoke<string>("git_commit", { message, stageAll }),
  gitLog: (limit: number) => invoke<GitCommit[]>("git_log", { limit }),
  gitCommitFiles: (id: string) => invoke<string[]>("git_commit_files", { id }),
  gitBranches: () => invoke<GitBranch[]>("git_branches"),
  gitCheckout: (name: string) => invoke<void>("git_checkout", { name }),
  gitCreateBranch: (name: string) => invoke<void>("git_create_branch", { name }),
  gitPush: () => invoke<string>("git_push"),
  gitPull: () => invoke<string>("git_pull"),
  gitInit: () => invoke<void>("git_init"),
  gitDiff: (path: string, staged: boolean, commit: string | null) =>
    invoke<GitFileDiff>("git_diff", { path, staged, commit }),
};

export function onVaultChanged(handler: (change: VaultChange) => void): Promise<UnlistenFn> {
  return listen<VaultChange>("vault://changed", (e) => handler(e.payload));
}
