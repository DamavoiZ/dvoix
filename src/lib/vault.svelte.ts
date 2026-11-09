// App state for the open vault and the open file, shared by all components.
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api, onVaultChanged, type VaultEntry, type VaultInfo } from "./api";
import { creatableFileTypes, type FileType } from "./filetypes";
import { ui } from "./ui.svelte";

const SAVE_DELAY_MS = 400;
const RECENT_MAX = 50;

/** An editor tab. `path` is null for an empty tab ("Neuer Tab"). */
export interface Tab {
  id: number;
  path: string | null;
}

export interface OpenOptions {
  /** Open in a new tab instead of replacing the active tab's file. */
  newTab?: boolean;
}

let nextTabId = 1;
const newTabObj = (path: string | null): Tab => ({ id: nextTabId++, path });

class VaultStore {
  info = $state<VaultInfo | null>(null);
  entries = $state<VaultEntry[]>([]);
  /** Path of the file shown in the active tab (null for an empty tab). */
  openPath = $state<string | null>(null);
  tabs = $state<Tab[]>([]);
  activeTabId = $state(0);
  /** Recently opened files, newest first. */
  recent = $state<string[]>([]);
  content = $state("");
  dirty = $state(false);
  error = $state<string | null>(null);
  /** Bumped on every change on disk so panels (backlinks, tags) can refresh. */
  revision = $state(0);

  private saveTimer: ReturnType<typeof setTimeout> | undefined;
  private refreshTimer: ReturnType<typeof setTimeout> | undefined;
  private unlisten: (() => void) | undefined;

  async init() {
    this.unlisten?.();
    this.unlisten = await onVaultChanged((change) => {
      this.scheduleRefresh();
      if (change.kind === "removed") void this.closeIfGone(change.path);
      else if (change.path === this.openPath && !this.dirty) void this.reloadOpenFile();
    });
    const last = await api.lastVault();
    if (last) await this.open(last).catch(() => undefined);
  }

  async pick() {
    const selected = await openDialog({ directory: true, title: "Vault-Ordner wählen" });
    if (typeof selected === "string") await this.open(selected);
  }

  async open(path: string) {
    try {
      await this.flush();
      this.info = await api.openVault(path);
      ui.useVault(this.info.path);
      this.openPath = null;
      this.content = "";
      this.dirty = false;
      this.error = null;
      await this.refresh();
      await this.restoreTabs();
    } catch (e) {
      this.error = String(e);
      throw e;
    }
  }

  async close() {
    await this.flush();
    await api.closeVault();
    this.info = null;
    this.entries = [];
    this.openPath = null;
    this.tabs = [];
    this.recent = [];
  }

  async refresh() {
    this.entries = await api.listEntries();
    this.revision++;
  }

  private scheduleRefresh() {
    clearTimeout(this.refreshTimer);
    this.refreshTimer = setTimeout(() => void this.refresh(), 150);
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
    } else {
      const tab = newTabObj(path);
      this.tabs.splice(active ? this.tabs.indexOf(active) + 1 : this.tabs.length, 0, tab);
      this.activeTabId = tab.id;
    }
    this.show(path, content);
  }

  async activateTab(id: number) {
    const tab = this.tabs.find((t) => t.id === id);
    if (!tab || (id === this.activeTabId && tab.path === this.openPath)) return;
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

  /** Makes `path` the file of the active tab. */
  private show(path: string | null, content: string) {
    clearTimeout(this.saveTimer);
    this.openPath = path;
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
      paths: this.tabs.map((t) => t.path ?? ""),
      active: Math.max(0, this.tabs.findIndex((t) => t.id === this.activeTabId)),
      recent: this.recent,
    });
  }

  /** Reopens the tabs the vault had when the app was last closed. */
  private async restoreTabs() {
    const saved = ui.savedTabs();
    const files = new Set(this.entries.filter((e) => !e.isDir).map((e) => e.path));
    const keep = (p: string) => !p || files.has(p);
    const paths = saved?.paths ?? [];
    this.recent = (saved?.recent ?? []).filter((p) => files.has(p));
    this.tabs = paths.filter(keep).map((p) => newTabObj(p || null));
    if (!this.tabs.length) this.tabs = [newTabObj(null)];
    // The previously active tab, or the closest one left of it if its file is gone.
    const before = paths.slice(0, (saved?.active ?? 0) + 1).filter(keep).length;
    const tab = this.tabs[Math.max(0, before - 1)];
    this.activeTabId = 0;
    await this.activateTab(tab.id);
  }

  /** Closes the tabs whose file matches `gone` (deleted, or inside a deleted folder). */
  private async closeTabs(gone: (path: string) => boolean) {
    const active = this.activeTab;
    const activeGone = !!active?.path && gone(active.path);
    if (activeGone) {
      clearTimeout(this.saveTimer);
      this.dirty = false;
      this.openPath = null;
    }
    const i = active ? this.tabs.indexOf(active) : -1;
    const left = this.tabs.slice(0, i).reverse();
    const right = this.tabs.slice(i + 1);
    const next = [...right, ...left].find((t) => !t.path || !gone(t.path));
    this.tabs = this.tabs.filter((t) => !t.path || !gone(t.path));
    this.recent = this.recent.filter((p) => !gone(p));
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
    const files = this.entries.filter((e) => !e.isDir);
    const t = target.toLowerCase();
    const exact = files.find((f) => f.path.toLowerCase() === t || f.path.toLowerCase() === `${t}.md`);
    if (exact) return exact.path;
    const byName = files.find((f) => {
      const name = f.name.toLowerCase();
      return name === t || name === `${t}.md`;
    });
    return byName?.path;
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
      this.error = `Speichern fehlgeschlagen: ${e}`;
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
  /** Creates "Unbenannt.ext" (or "Unbenannt 2.ext", ...) of a file type in `folder`. */
  async createFile(type: FileType, folder = "") {
    const ext = type.extensions[0];
    const prefix = folder ? `${folder}/` : "";
    const taken = new Set(this.entries.map((e) => e.path.toLowerCase()));
    let name = "Unbenannt";
    for (let n = 2; taken.has(`${prefix}${name}.${ext}`.toLowerCase()); n++) name = `Unbenannt ${n}`;
    // New files get their own tab, unless the active tab is empty anyway.
    await this.createFileAt(`${prefix}${name}.${ext}`, type.template?.(name) ?? "", { newTab: !!this.openPath });
  }

  private async createFileAt(path: string, content: string, opts: OpenOptions = {}) {
    try {
      await this.flush();
      await api.createFile(path, content);
      await this.refresh();
      await this.openFile(path, opts);
    } catch (e) {
      this.error = String(e);
    }
  }

  async createFolder(parent = "") {
    const prefix = parent ? `${parent}/` : "";
    const taken = new Set(this.entries.map((e) => e.path.toLowerCase()));
    let name = "Neuer Ordner";
    for (let n = 2; taken.has(`${prefix}${name}`.toLowerCase()); n++) name = `Neuer Ordner ${n}`;
    try {
      await api.createFolder(`${prefix}${name}`);
      if (parent) ui.reveal(`${prefix}${name}`);
      await this.refresh();
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
      await this.refresh();
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
      await this.refresh();
    } catch (e) {
      this.error = String(e);
    }
  }
}

export const vault = new VaultStore();
