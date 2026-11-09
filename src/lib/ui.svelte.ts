// Layout state (which side panel is open, its width, expanded folders).
// Kept in localStorage: it is about this computer's window, not about the vault's content.
import { SvelteSet } from "svelte/reactivity";

export type PanelId = "files" | "search" | "tags";

const KEY = "dvoix.layout";

interface Saved {
  panel: PanelId;
  panelOpen: boolean;
  panelWidth: number;
  expanded: Record<string, string[]>;
  tabs: Record<string, SavedTabs>;
}

/** Open tabs of one vault: file paths ("" = empty tab) and the active index. */
export interface SavedTabs {
  paths: string[];
  active: number;
  /** Recently opened files, newest first (for the quick switcher). */
  recent: string[];
}

function load(): Partial<Saved> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}");
  } catch {
    return {};
  }
}

export const PANEL_MIN = 180;
export const PANEL_MAX = 520;

class UiStore {
  panel = $state<PanelId>("files");
  panelOpen = $state(true);
  panelWidth = $state(280);
  /** Quick switcher (Strg+O) is open. */
  switcherOpen = $state(false);
  /** Path that is currently being renamed inline in the file tree. */
  renaming = $state<string | null>(null);
  /** Expanded folders of the current vault. */
  expanded = new SvelteSet<string>();

  private vaultPath = "";
  private expandedByVault: Record<string, string[]> = {};
  private tabsByVault: Record<string, SavedTabs> = {};

  constructor() {
    const s = load();
    this.panel = s.panel ?? "files";
    this.panelOpen = s.panelOpen ?? true;
    this.panelWidth = s.panelWidth ?? 280;
    this.expandedByVault = s.expanded ?? {};
    this.tabsByVault = s.tabs ?? {};
  }

  save() {
    if (this.vaultPath) this.expandedByVault[this.vaultPath] = [...this.expanded];
    const s: Saved = {
      panel: this.panel,
      panelOpen: this.panelOpen,
      panelWidth: this.panelWidth,
      expanded: this.expandedByVault,
      tabs: this.tabsByVault,
    };
    localStorage.setItem(KEY, JSON.stringify(s));
  }

  /** Restores the expanded folders of a vault when it is opened. */
  useVault(path: string) {
    this.vaultPath = path;
    this.expanded.clear();
    for (const p of this.expandedByVault[path] ?? []) this.expanded.add(p);
  }

  savedTabs(): SavedTabs | undefined {
    return this.tabsByVault[this.vaultPath];
  }

  saveTabs(tabs: SavedTabs) {
    if (!this.vaultPath) return;
    this.tabsByVault[this.vaultPath] = tabs;
    this.save();
  }

  /** Ribbon click: switch panel, or collapse it when the active one is clicked again. */
  togglePanel(id: PanelId) {
    if (this.panel === id && this.panelOpen) this.panelOpen = false;
    else {
      this.panel = id;
      this.panelOpen = true;
    }
    this.save();
  }

  setWidth(px: number) {
    this.panelWidth = Math.round(Math.min(PANEL_MAX, Math.max(PANEL_MIN, px)));
  }

  toggleFolder(path: string) {
    if (this.expanded.has(path)) this.expanded.delete(path);
    else this.expanded.add(path);
    this.save();
  }

  /** Expands all parent folders of a path so it becomes visible in the tree. */
  reveal(path: string) {
    const parts = path.split("/");
    for (let i = 1; i < parts.length; i++) this.expanded.add(parts.slice(0, i).join("/"));
    this.save();
  }

  collapseAll() {
    this.expanded.clear();
    this.save();
  }

  /** Keeps expanded state when a folder is renamed or moved. */
  moved(from: string, to: string) {
    for (const p of [...this.expanded]) {
      if (p === from || p.startsWith(`${from}/`)) {
        this.expanded.delete(p);
        this.expanded.add(to + p.slice(from.length));
      }
    }
    this.save();
  }
}

export const ui = new UiStore();
