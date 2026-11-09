// App state for the open vault and the open file, shared by all components.
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api, onVaultChanged, type VaultEntry, type VaultInfo } from "./api";
import { creatableFileTypes, type FileType } from "./filetypes";
import { ui } from "./ui.svelte";

const SAVE_DELAY_MS = 400;

class VaultStore {
  info = $state<VaultInfo | null>(null);
  entries = $state<VaultEntry[]>([]);
  openPath = $state<string | null>(null);
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
      if (change.path !== this.openPath) return;
      if (change.kind === "removed") void this.reloadOrClose();
      else if (!this.dirty) void this.reloadOpenFile();
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
      this.error = null;
      await this.refresh();
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
  }

  async refresh() {
    this.entries = await api.listEntries();
    this.revision++;
  }

  private scheduleRefresh() {
    clearTimeout(this.refreshTimer);
    this.refreshTimer = setTimeout(() => void this.refresh(), 150);
  }

  async openFile(path: string) {
    if (path === this.openPath) return;
    await this.flush();
    try {
      this.content = await api.readFile(path);
      this.openPath = path;
      this.dirty = false;
      this.error = null;
      ui.reveal(path);
    } catch (e) {
      this.error = String(e);
    }
  }

  /** Opens a link target: a vault path (`folder/note.md`) or a wikilink name (`note`). */
  async openLink(target: string) {
    const path = this.resolveLink(target);
    if (path) return this.openFile(path);
    // Like Obsidian: following a link to a missing note creates it.
    const name = target.endsWith(".md") ? target : `${target}.md`;
    await this.createFileAt(name, `# ${target.split("/").pop()}\n\n`);
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

  private async reloadOrClose() {
    if (!this.openPath) return;
    const fresh = await api.readFile(this.openPath).catch(() => null);
    if (fresh === null) {
      this.openPath = null;
      this.content = "";
      this.dirty = false;
    }
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
    await this.createFileAt(`${prefix}${name}.${ext}`, type.template?.(name) ?? "");
  }

  private async createFileAt(path: string, content: string) {
    try {
      await this.flush();
      await api.createFile(path, content);
      await this.refresh();
      await this.openFile(path);
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
      if (this.openPath === from) this.openPath = to;
      else if (this.openPath?.startsWith(`${from}/`)) this.openPath = to + this.openPath.slice(from.length);
      if (to.includes("/")) ui.reveal(to);
      await this.refresh();
    } catch (e) {
      this.error = String(e);
    }
  }

  async trash(path: string) {
    try {
      if (this.openPath === path || this.openPath?.startsWith(`${path}/`)) {
        clearTimeout(this.saveTimer);
        this.openPath = null;
        this.dirty = false;
      }
      await api.trashEntry(path);
      await this.refresh();
    } catch (e) {
      this.error = String(e);
    }
  }
}

export const vault = new VaultStore();
