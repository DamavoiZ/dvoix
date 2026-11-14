// Layout state of the open vault: which side panel is shown, its width,
// expanded folders, open tabs. Saved in `.dvoix/workspace.json`, like
// Obsidian's `workspace.json`.
import { SvelteSet } from "svelte/reactivity";
import { readJsonRaw, writeJson } from "./config";

/** Id of a side panel. Panels come from plugins, e.g. "file-explorer" or "tasks". */
export type PanelId = string;

/** Open tabs: file paths ("" = empty tab, `dvoix://view/<id>` = a plugin view) and the active index. */
export interface SavedTabs {
  paths: string[];
  active: number;
  /** Recently opened files, newest first (for the quick switcher). */
  recent: string[];
}

interface Workspace {
  panel: PanelId;
  panelOpen: boolean;
  panelWidth: number;
  expanded: string[];
  tabs: SavedTabs;
}

const WORKSPACE_FILE = "workspace.json";
const DEFAULT_PANEL = "file-explorer";
/** Where earlier versions kept the layout of all vaults. Read once, then moved to `.dvoix/`. */
const LEGACY_KEY = "dvoix.layout";
/** Panel ids before they became plugins. */
const LEGACY_PANELS: Record<string, string> = { files: "file-explorer" };

export const PANEL_MIN = 180;
export const PANEL_MAX = 520;
export const PANEL_DEFAULT = 280;

class UiStore {
  panel = $state<PanelId>(DEFAULT_PANEL);
  panelOpen = $state(true);
  panelWidth = $state(PANEL_DEFAULT);
  /** Open dialog that takes the keyboard, e.g. "quick-switcher" or "settings". */
  modal = $state<string | null>(null);
  /** Path that is currently being renamed inline in the file tree. */
  renaming = $state<string | null>(null);
  /** Expanded folders of the current vault. */
  expanded = new SvelteSet<string>();

  private tabs: SavedTabs | undefined;
  private loaded = false;

  /** Loads the workspace of the vault that was just opened. */
  async load(vaultPath: string) {
    this.loaded = false;
    let saved = (await readJsonRaw(WORKSPACE_FILE)) as Partial<Workspace> | undefined;
    const migrated = !saved;
    if (!saved) saved = takeLegacy(vaultPath);
    this.panel = LEGACY_PANELS[saved.panel ?? ""] ?? saved.panel ?? DEFAULT_PANEL;
    this.panelOpen = saved.panelOpen ?? true;
    this.panelWidth = clampWidth(saved.panelWidth ?? PANEL_DEFAULT);
    this.expanded.clear();
    for (const p of saved.expanded ?? []) this.expanded.add(p);
    this.tabs = saved.tabs;
    this.modal = null;
    this.renaming = null;
    this.loaded = true;
    if (migrated) this.save();
  }

  unload() {
    this.loaded = false;
    this.tabs = undefined;
    this.expanded.clear();
    this.modal = null;
  }

  save() {
    if (!this.loaded) return;
    const ws: Workspace = {
      panel: this.panel,
      panelOpen: this.panelOpen,
      panelWidth: this.panelWidth,
      expanded: [...this.expanded],
      tabs: this.tabs ?? { paths: [], active: 0, recent: [] },
    };
    writeJson(WORKSPACE_FILE, ws);
  }

  savedTabs(): SavedTabs | undefined {
    return this.tabs;
  }

  saveTabs(tabs: SavedTabs) {
    this.tabs = tabs;
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

  /** Shows a panel (opens the side panel if it was collapsed). */
  showPanel(id: PanelId) {
    this.panel = id;
    this.panelOpen = true;
    this.save();
  }

  setWidth(px: number) {
    this.panelWidth = clampWidth(px);
  }

  toggleModal(id: string) {
    this.modal = this.modal === id ? null : id;
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

  /** Expands `folder` and all of its parent folders and shows the file tree. */
  revealFolder(folder: string) {
    if (!folder) return;
    const parts = folder.split("/");
    for (let i = 1; i <= parts.length; i++) this.expanded.add(parts.slice(0, i).join("/"));
    this.panel = DEFAULT_PANEL;
    this.panelOpen = true;
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

function clampWidth(px: number): number {
  return Math.round(Math.min(PANEL_MAX, Math.max(PANEL_MIN, px)));
}

interface LegacyLayout {
  panel?: string;
  panelOpen?: boolean;
  panelWidth?: number;
  expanded?: Record<string, string[]>;
  tabs?: Record<string, SavedTabs>;
}

/**
 * Takes this vault's layout out of localStorage (where versions before the
 * plugin system kept it). The key is removed once no vault is left in it.
 */
function takeLegacy(vaultPath: string): Partial<Workspace> {
  let old: LegacyLayout;
  try {
    old = JSON.parse(localStorage.getItem(LEGACY_KEY) ?? "null");
  } catch {
    old = {};
  }
  if (!old) return {};
  const ws: Partial<Workspace> = {
    panel: old.panel,
    panelOpen: old.panelOpen,
    panelWidth: old.panelWidth,
    expanded: old.expanded?.[vaultPath],
    tabs: old.tabs?.[vaultPath],
  };
  delete old.expanded?.[vaultPath];
  delete old.tabs?.[vaultPath];
  const left = Object.keys(old.expanded ?? {}).length + Object.keys(old.tabs ?? {}).length;
  if (left) localStorage.setItem(LEGACY_KEY, JSON.stringify(old));
  else localStorage.removeItem(LEGACY_KEY);
  return ws;
}

export const ui = new UiStore();
