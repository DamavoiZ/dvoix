// App state for the open vault and the open file, shared by all components.
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api, onVaultChanged, type VaultChange, type VaultEntry, type VaultInfo } from "./api";
import { creatableFileTypes, type FileType } from "./filetypes";
import { flushConfig } from "./config";
import { plugins } from "./plugins/manager.svelte";
import { pluginStore } from "./plugins/store/store.svelte";
import { settings } from "./settings.svelte";
import { ui } from "./ui.svelte";

const SAVE_DELAY_MS = 400;
const RECENT_MAX = 50;

/**
 * An editor tab. `path` is null for an empty tab ("New tab") and for a tab
 * that shows a view of a plugin (e.g. the graph); then `view` is its id.
 */
export interface Tab {
  id: number;
  path: string | null;
  view?: string;
}

export interface OpenOptions {
  /** Open in a new tab instead of replacing the active tab's file. */
  newTab?: boolean;
}

/** How a view tab is stored in `workspace.json`; `//` never occurs in a vault path. */
const VIEW_PREFIX = "dvoix://view/";

/**
 * A group of changes in the vault. The number moves with every change of the
 * group, so a panel that reads it refreshes only when it has to.
 */
export interface ChangeGroup {
  /** Moves with every change of this group. */
  number: number;
  /** The path of the last change of this group. */
  path: string;
}

/** True for a note: only notes are indexed, so only they change the panels of notes. */
function isNote(path: string): boolean {
  return path.toLowerCase().endsWith(".md");
}

let nextTabId = 1;
const newTabObj = (path: string | null, view?: string): Tab => ({ id: nextTabId++, path, view });

class VaultStore {
  info = $state<VaultInfo | null>(null);
  entries = $state<VaultEntry[]>([]);
  /** Path of the file shown in the active tab (null for an empty tab). */
  openPath = $state<string | null>(null);
  /** Id of the view shown in the active tab instead of a file, if any. */
  openView = $state<string | null>(null);
  tabs = $state<Tab[]>([]);
  activeTabId = $state(0);
  /** Recently opened files, newest first. */
  recent = $state<string[]>([]);
  content = $state("");
  dirty = $state(false);
  error = $state<string | null>(null);
  /**
   * What changed in the vault. Every group has a counter that moves only for
   * the changes that can change what it shows, so a panel refreshes only when
   * it has to: the panels of notes read `notes`, the panels of the repository
   * read `files`.
   */
  changes = $state<{ notes: ChangeGroup; structure: ChangeGroup; files: ChangeGroup }>({
    /** A note changed: its text, its tags, its tasks and its links. */
    notes: { number: 0, path: "" },
    /** A file or folder was added, removed or moved: the tree of the vault changes. */
    structure: { number: 0, path: "" },
    /** Any file changed. */
    files: { number: 0, path: "" },
  });
  /** Moves when the list of the vault changes, so that caches know they are stale. */
  entriesVersion = $state(0);

  /** The files of the vault by their path, for the lookups of links. */
  private fileByPath = $derived.by(() => {
    const map = new Map<string, VaultEntry>();
    for (const entry of this.entries) if (!entry.isDir) map.set(entry.path.toLowerCase(), entry);
    return map;
  });

  /** The files of the vault by their name, for the lookups of links. */
  private fileByName = $derived.by(() => {
    const map = new Map<string, VaultEntry>();
    for (const entry of this.entries) if (!entry.isDir && !map.has(entry.name.toLowerCase())) map.set(entry.name.toLowerCase(), entry);
    return map;
  });

  private saveTimer: ReturnType<typeof setTimeout> | undefined;
  private refreshTimer: ReturnType<typeof setTimeout> | undefined;
  private unlisten: (() => void) | undefined;
  private unlistenViews: (() => void) | undefined;
  private tearingDown = false;

  async init() {
    this.unlisten?.();
    this.unlistenViews?.();
    this.unlistenViews = plugins.onViewRemoved((id) => {
      // When the whole vault closes, its tabs are dropped anyway (and saved before).
      if (!this.tearingDown) void this.closeTabsWhere((t) => t.view === id);
    });
    this.unlisten = await onVaultChanged((change) => {
      // Hidden files are not part of `entries` (only the hidden-files plugin lists them).
      if (!change.hidden) this.onChange(change);
      if (change.kind === "removed") void this.closeIfGone(change.path);
      else if (change.path === this.openPath && !this.dirty) void this.reloadOpenFile();
    });
    const last = await api.lastVault();
    if (last) await this.open(last).catch(() => undefined);
  }

  async pick() {
    const selected = await openDialog({ directory: true, title: "Choose vault folder" });
    if (typeof selected === "string") await this.open(selected);
  }

  async open(path: string) {
    try {
      await this.teardown();
      const info = await api.openVault(path);
      // Settings, workspace and plugins of this vault, all from `<vault>/.dvoix/`.
      await Promise.all([settings.load(), ui.load(info.path)]);
      this.openPath = null;
      this.content = "";
      this.dirty = false;
      this.error = null;
      await plugins.loadVault();
      await pluginStore.loadVault();
      this.info = info;
      await this.refresh();
      await this.restoreTabs();
    } catch (e) {
      this.error = String(e);
      throw e;
    }
  }

  async close() {
    await this.teardown();
    await api.closeVault();
  }

  /** Saves everything of the open vault and unloads its plugins. */
  private async teardown() {
    if (!this.info) return;
    await this.flush();
    ui.unload(); // stop saving the workspace: unloading plugins (e.g. tabs) must not change it
    await flushConfig();
    this.tearingDown = true;
    try {
      await plugins.unloadVault();
      pluginStore.reset();
    } finally {
      this.tearingDown = false;
    }
    settings.reset();
    this.info = null;
    this.entries = [];
    this.entriesVersion++;
    this.openPath = null;
    this.openView = null;
    this.tabs = [];
    this.recent = [];
  }

  async refresh(changed = "") {
    this.entries = await api.listEntries();
    this.entriesVersion++;
    // The list of the vault was replaced, so the tree of the vault changed.
    this.note(this.changes.structure, changed);
  }

  private scheduleRefresh(changed = "") {
    clearTimeout(this.refreshTimer);
    this.refreshTimer = setTimeout(() => void this.refresh(changed), 150);
  }

  /**
   * A file changed on disk. Only the content of a file changed in most cases,
   * and then its entry is corrected in place: the vault is not listed again
   * and the tree of the vault is not built again.
   */
  private onChange(change: VaultChange) {
    this.note(this.changes.files, change.path);
    // A folder can hold notes: when it comes, goes or is renamed, the panels
    // of notes change too, although the watcher names only the folder.
    if (isNote(change.path) || this.isFolder(change)) this.note(this.changes.notes, change.path);
    if (change.kind === "modified" && this.patchEntry(change)) return;
    // A file was added or removed: the vault has to be listed again.
    this.scheduleRefresh(change.path);
  }

  /**
   * Writes the new size and time of a changed file into its entry, so the
   * list of the vault stays right without listing the whole vault again.
   * False when the file is not in the list, then the vault has to be listed.
   */
  private patchEntry(change: VaultChange): boolean {
    const entry = change.entry;
    if (!entry) return false;
    const found = this.entries.find((e) => e.path === entry.path && !e.isDir);
    if (!found) return false;
    found.size = entry.size;
    found.modified = entry.modified;
    this.entriesVersion++;
    return true;
  }

  /** True when the change is about a folder, also one that is already gone. */
  private isFolder(change: VaultChange): boolean {
    if (change.entry) return change.entry.isDir;
    return this.entries.some((e) => e.isDir && e.path === change.path);
  }

  /** Notes a change in a group: the counter moves and the path is remembered. */
  private note(group: ChangeGroup, path: string) {
    group.number++;
    group.path = path;
  }

  // ---------------------------------------------------------------- tabs

  get activeTab(): Tab | undefined {
    return this.tabs.find((t) => t.id === this.activeTabId);
  }

  /**
   * Opens a file. If it is already open in a tab, that tab is activated.
   * Otherwise it replaces the active tab's file, or gets a new tab with `newTab`.
   */
  async openFile(path: string, opts: OpenOptions = {}) {
    const existing = this.tabs.find((t) => t.path === path);
    if (existing) return this.activateTab(existing.id);
    const content = await this.read(path);
    if (content === null) return;
    const active = this.activeTab;
    if (active && !opts.newTab) {
      active.path = path;
      active.view = undefined;
    } else {
      const tab = newTabObj(path);
      this.tabs.splice(active ? this.tabs.indexOf(active) + 1 : this.tabs.length, 0, tab);
      this.activeTabId = tab.id;
    }
    this.show(path, content);
  }

  /**
   * Opens a view of a plugin (see `PluginContext.registerView`) in a tab, like a file:
   * activates its tab if it has one, else replaces the active tab or opens a new one.
   */
  async openViewTab(id: string, opts: OpenOptions = {}) {
    if (!plugins.view(id)) return;
    const existing = this.tabs.find((t) => t.view === id);
    if (existing) return this.activateTab(existing.id);
    await this.flush();
    const active = this.activeTab;
    if (active && !opts.newTab) {
      active.path = null;
      active.view = id;
    } else {
      const tab = newTabObj(null, id);
      this.tabs.splice(active ? this.tabs.indexOf(active) + 1 : this.tabs.length, 0, tab);
      this.activeTabId = tab.id;
    }
    this.show(null, "", id);
  }

  async activateTab(id: number) {
    const tab = this.tabs.find((t) => t.id === id);
    if (!tab || (id === this.activeTabId && tab.path === this.openPath && (tab.view ?? null) === this.openView)) return;
    if (tab.view) {
      await this.flush();
      this.activeTabId = id;
      return this.show(null, "", tab.view);
    }
    const content = tab.path ? await this.read(tab.path) : "";
    if (content === null) tab.path = null; // file vanished: keep the tab, but empty
    this.activeTabId = id;
    this.show(tab.path, content ?? "");
  }

  /** Opens an empty tab next to the active one. */
  async newTab() {
    await this.flush();
    const active = this.activeTab;
    const tab = newTabObj(null);
    this.tabs.splice(active ? this.tabs.indexOf(active) + 1 : this.tabs.length, 0, tab);
    this.activeTabId = tab.id;
    this.show(null, "");
  }

  async closeTab(id: number) {
    const i = this.tabs.findIndex((t) => t.id === id);
    if (i < 0) return;
    if (id !== this.activeTabId) {
      this.tabs.splice(i, 1);
      this.saveTabs();
      return;
    }
    await this.flush();
    const next = this.tabs[i + 1] ?? this.tabs[i - 1];
    this.tabs.splice(i, 1);
    if (next) await this.activateTab(next.id);
    else await this.newTab();
  }

  async closeOtherTabs(id: number) {
    await this.activateTab(id);
    this.tabs = this.tabs.filter((t) => t.id === id);
    this.saveTabs();
  }

  /** Activates the tab `delta` steps to the right (negative: left), wrapping around. */
  async cycleTab(delta: number) {
    const n = this.tabs.length;
    if (n < 2) return;
    const i = this.tabs.findIndex((t) => t.id === this.activeTabId);
    await this.activateTab(this.tabs[(((i + delta) % n) + n) % n].id);
  }

  /** Drag & drop in the tab bar: moves a tab to position `to`. */
  moveTab(id: number, to: number) {
    const from = this.tabs.findIndex((t) => t.id === id);
    if (from < 0) return;
    const [tab] = this.tabs.splice(from, 1);
    this.tabs.splice(Math.max(0, Math.min(to, this.tabs.length)), 0, tab);
    this.saveTabs();
  }

  /** Flushes the current file and reads `path`; null (and an error message) if that fails. */
  private async read(path: string): Promise<string | null> {
    await this.flush();
    try {
      return await api.readFile(path);
    } catch (e) {
      this.error = String(e);
      return null;
    }
  }

  /** Makes `path` (or the view `view`) the content of the active tab. */
  private show(path: string | null, content: string, view: string | null = null) {
    clearTimeout(this.saveTimer);
    this.openPath = path;
    this.openView = view;
    this.content = content;
    this.dirty = false;
    if (path) {
      this.error = null;
      this.recent = [path, ...this.recent.filter((p) => p !== path)].slice(0, RECENT_MAX);
      ui.reveal(path);
    }
    this.saveTabs();
  }

  private saveTabs() {
    ui.saveTabs({
      paths: this.tabs.map((t) => (t.view ? VIEW_PREFIX + t.view : (t.path ?? ""))),
      active: Math.max(0, this.tabs.findIndex((t) => t.id === this.activeTabId)),
      recent: this.recent,
    });
  }

  /** Reopens the tabs the vault had when the app was last closed. */
  private async restoreTabs() {
    const saved = ui.savedTabs();
    const files = new Set(this.entries.filter((e) => !e.isDir).map((e) => e.path));
    const view = (p: string) => (p.startsWith(VIEW_PREFIX) ? p.slice(VIEW_PREFIX.length) : undefined);
    // Views only come back if their plugin is still on.
    const keep = (p: string) => !p || files.has(p) || !!plugins.view(view(p) ?? "");
    const paths = saved?.paths ?? [];
    this.recent = (saved?.recent ?? []).filter((p) => files.has(p));
    this.tabs = paths.filter(keep).map((p) => (view(p) ? newTabObj(null, view(p)) : newTabObj(p || null)));
    if (!this.tabs.length) this.tabs = [newTabObj(null)];
    // The previously active tab, or the closest one left of it if its file is gone.
    const before = paths.slice(0, (saved?.active ?? 0) + 1).filter(keep).length;
    const tab = this.tabs[Math.max(0, before - 1)];
    this.activeTabId = 0;
    await this.activateTab(tab.id);
  }

  /** Closes the tabs whose file matches `gone` (deleted, or inside a deleted folder). */
  private async closeTabs(gone: (path: string) => boolean) {
    this.recent = this.recent.filter((p) => !gone(p));
    await this.closeTabsWhere((t) => !!t.path && gone(t.path));
  }

  /** Closes the tabs matching `close`; the active one hands over to its neighbour. */
  private async closeTabsWhere(close: (tab: Tab) => boolean) {
    if (!this.tabs.some(close)) return this.saveTabs();
    const active = this.activeTab;
    const activeGone = !!active && close(active);
    if (activeGone) {
      clearTimeout(this.saveTimer);
      this.dirty = false;
      this.openPath = null;
      this.openView = null;
    }
    const i = active ? this.tabs.indexOf(active) : -1;
    const left = this.tabs.slice(0, i).reverse();
    const right = this.tabs.slice(i + 1);
    const next = [...right, ...left].find((t) => !close(t));
    this.tabs = this.tabs.filter((t) => !close(t));
    if (!activeGone) return this.saveTabs();
    if (next) await this.activateTab(next.id);
    else {
      this.tabs = [];
      await this.newTab();
    }
  }

  /** A file was removed on disk: close its tabs if it is really gone (not just rewritten). */
  private async closeIfGone(path: string) {
    if (!this.tabs.some((t) => t.path === path)) return;
    const exists = await api.readFile(path).then(
      () => true,
      () => false,
    );
    if (!exists) await this.closeTabs((p) => p === path);
  }

  // ---------------------------------------------------------------- files

  /** Opens a link target: a vault path (`folder/note.md`) or a wikilink name (`note`). */
  async openLink(target: string, opts: OpenOptions = {}) {
    const path = this.resolveLink(target);
    if (path) return this.openFile(path, opts);
    // Like Obsidian: following a link to a missing note creates it.
    const name = target.endsWith(".md") ? target : `${target}.md`;
    await this.createFileAt(name, `# ${target.split("/").pop()}\n\n`, opts);
  }

  resolveLink(target: string): string | undefined {
    // The path of the file, then its name. The maps are built once per change
    // of the list of the vault, so this is a lookup and not a walk.
    const t = target.toLowerCase();
    const exact = this.fileByPath.get(t) ?? this.fileByPath.get(`${t}.md`);
    if (exact) return exact.path;
    return (this.fileByName.get(t) ?? this.fileByName.get(`${t}.md`))?.path;
  }

  /** Called by file-type views on every edit; saves shortly after typing stops. */
  update(content: string) {
    if (content === this.content) return;
    this.content = content;
    this.dirty = true;
    clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => void this.flush(), SAVE_DELAY_MS);
  }

  async flush() {
    clearTimeout(this.saveTimer);
    if (!this.dirty || !this.openPath) return;
    const path = this.openPath;
    const content = this.content;
    try {
      await api.writeFile(path, content);
      if (this.openPath === path && this.content === content) this.dirty = false;
    } catch (e) {
      this.error = `Saving failed: ${e}`;
    }
  }

  private async reloadOpenFile() {
    if (!this.openPath) return;
    const fresh = await api.readFile(this.openPath).catch(() => null);
    if (fresh !== null && fresh !== this.content && !this.dirty) this.content = fresh;
  }

  creatable(): FileType[] {
    return creatableFileTypes();
  }
  /** Creates "Untitled.ext" (or "Untitled 2.ext", ...) of a file type in `folder`. */
  async createFile(type: FileType, folder = "") {
    const ext = type.extensions[0];
    const prefix = folder ? `${folder}/` : "";
    const taken = new Set(this.entries.map((e) => e.path.toLowerCase()));
    let name = "Untitled";
    for (let n = 2; taken.has(`${prefix}${name}.${ext}`.toLowerCase()); n++) name = `Untitled ${n}`;
    // New files get their own tab, unless the active tab is empty anyway.
    await this.createFileAt(`${prefix}${name}.${ext}`, type.template?.(name) ?? "", { newTab: !!this.openPath });
  }

  private async createFileAt(path: string, content: string, opts: OpenOptions = {}) {
    try {
      await this.flush();
      await api.createFile(path, content);
      await this.refresh(path);
      await this.openFile(path, opts);
    } catch (e) {
      this.error = String(e);
    }
  }

  async createFolder(parent = "") {
    const prefix = parent ? `${parent}/` : "";
    const taken = new Set(this.entries.map((e) => e.path.toLowerCase()));
    let name = "New folder";
    for (let n = 2; taken.has(`${prefix}${name}`.toLowerCase()); n++) name = `New folder ${n}`;
    try {
      await api.createFolder(`${prefix}${name}`);
      if (parent) ui.reveal(`${prefix}${name}`);
      await this.refresh(`${prefix}${name}`);
      return `${prefix}${name}`;
    } catch (e) {
      this.error = String(e);
    }
  }

  async rename(from: string, newName: string) {
    const parent = from.includes("/") ? from.slice(0, from.lastIndexOf("/") + 1) : "";
    if (!newName.trim()) return;
    await this.moveTo(from, `${parent}${newName.trim()}`);
  }

  /** Moves a file or folder into another folder ("" = vault root), e.g. by drag & drop. */
  async move(from: string, folder: string) {
    const name = from.split("/").pop()!;
    if (folder === from || folder.startsWith(`${from}/`)) return; // not into itself
    await this.moveTo(from, folder ? `${folder}/${name}` : name);
  }

  private async moveTo(from: string, to: string) {
    if (to === from) return;
    try {
      await this.flush();
      await api.renameEntry(from, to);
      ui.moved(from, to);
      const remap = (p: string) => (p === from ? to : p.startsWith(`${from}/`) ? to + p.slice(from.length) : p);
      if (this.openPath) this.openPath = remap(this.openPath);
      for (const t of this.tabs) if (t.path) t.path = remap(t.path);
      this.recent = this.recent.map(remap);
      this.saveTabs();
      if (to.includes("/")) ui.reveal(to);
      await this.refresh(to);
    } catch (e) {
      this.error = String(e);
    }
  }

  async trash(path: string) {
    try {
      const p = this.openPath;
      if (p === path || p?.startsWith(`${path}/`)) {
        // Don't let a pending save recreate the file after it was trashed.
        clearTimeout(this.saveTimer);
        this.dirty = false;
      }
      await api.trashEntry(path);
      await this.closeTabs((p) => p === path || p.startsWith(`${path}/`));
      await this.refresh(path);
    } catch (e) {
      this.error = String(e);
    }
  }
}

export const vault = new VaultStore();
