// Loads and unloads plugins and keeps everything they registered (panels,
// commands, file types, ...) in one place for the rest of the app to render.
import type { Extension } from "@codemirror/state";
import { readJsonRaw, readJson, writeJson } from "../config";
import { registerFileType } from "../filetypes";
import type {
  CommandSpec,
  Dispose,
  ExplorerEntriesSpec,
  FileFooterSpec,
  Hotkey,
  OverlaySpec,
  PaneHeaderSpec,
  PanelSpec,
  Plugin,
  PluginContext,
  PluginManifest,
  RibbonActionSpec,
  SettingTabSpec,
  ViewSpec,
} from "./types";

/** Which core plugins are on, by id. Like Obsidian's `core-plugins.json`. */
const ENABLED_FILE = "core-plugins.json";

export interface Command extends CommandSpec {
  /** `<plugin-id>:<command-id>` */
  fullId: string;
  pluginId: string;
}

interface Owned {
  pluginId: string;
}

type WithOrder = { order?: number };
const byOrder = <T extends WithOrder>(list: T[]) => [...list].sort((a, b) => (a.order ?? 100) - (b.order ?? 100));

class PluginManager {
  /** All core plugins, in load order. */
  readonly all: Plugin[] = [];
  /**
   * Plugins installed from the store that are loaded (or failed to load) in
   * this vault. Their on/off state lives in the store (`store/store.svelte.ts`),
   * not in `core-plugins.json`.
   */
  extra = $state.raw<Plugin[]>([]);
  /** Turned on in this vault (saved in `.dvoix/core-plugins.json`). */
  enabled = $state<Record<string, boolean>>({});
  // Registries are `$state.raw`: entries hold components and are compared by identity on removal.
  /** Plugins that are loaded right now. */
  loaded = $state<string[]>([]);
  /** Error message of plugins whose `onload` failed. */
  errors = $state<Record<string, string>>({});
  /** Bumped when editor extensions change, so open editors are rebuilt. */
  editorRevision = $state(0);

  private panelList = $state.raw<(PanelSpec & Owned)[]>([]);
  private ribbonList = $state.raw<(RibbonActionSpec & Owned)[]>([]);
  private commandList = $state.raw<Command[]>([]);
  private footerList = $state.raw<(FileFooterSpec & Owned)[]>([]);
  private headerList = $state.raw<(PaneHeaderSpec & Owned)[]>([]);
  private overlayList = $state.raw<(OverlaySpec & Owned)[]>([]);
  private settingList = $state.raw<(SettingTabSpec & Owned)[]>([]);
  private viewList = $state.raw<(ViewSpec & Owned)[]>([]);
  private explorerEntryList = $state.raw<(ExplorerEntriesSpec & Owned)[]>([]);
  private viewRemovedListeners = new Set<(id: string) => void>();
  private extensionList: { pluginId: string; extension: Extension }[] = [];
  private disposers = new Map<string, Dispose[]>();
  private queue: Promise<void> = Promise.resolve();

  panels = $derived(byOrder(this.panelList));
  ribbonActions = $derived(byOrder(this.ribbonList));
  commands = $derived(this.commandList);
  fileFooters = $derived(byOrder(this.footerList));
  paneHeaders = $derived(byOrder(this.headerList));
  overlays = $derived(this.overlayList);
  settingTabs = $derived(this.settingList);
  views = $derived(this.viewList);
  /** Extra file explorer entries of all loaded plugins (see `addExplorerEntries`). */
  explorerEntries = $derived(this.explorerEntryList.flatMap((s) => s.entries()));

  /** Makes a plugin known to the app. Call before a vault is opened. */
  add(plugin: Plugin) {
    if (this.all.some((p) => p.manifest.id === plugin.manifest.id)) {
      throw new Error(`Plugin "${plugin.manifest.id}" is already registered`);
    }
    this.all.push(plugin);
  }

  get(id: string): Plugin | undefined {
    return this.all.find((p) => p.manifest.id === id) ?? this.extra.find((p) => p.manifest.id === id);
  }

  /** Whether a core plugin is on (store plugins: see `pluginStore.isEnabled`). */
  isEnabled(id: string): boolean {
    const p = this.all.find((p) => p.manifest.id === id);
    if (!p) return false;
    return this.enabled[id] ?? p.manifest.enabledByDefault ?? true;
  }

  /** A registered view (see `PluginContext.registerView`), if its plugin is on. */
  view(id: string): (ViewSpec & Owned) | undefined {
    return this.viewList.find((v) => v.id === id);
  }

  /** Called when a view goes away (its plugin was turned off), so its tabs can close. */
  onViewRemoved(fn: (id: string) => void): Dispose {
    this.viewRemovedListeners.add(fn);
    return () => this.viewRemovedListeners.delete(fn);
  }

  isLoaded(id: string): boolean {
    return this.loaded.includes(id);
  }

  /** Markdown editor extensions of all loaded plugins. */
  editorExtensions(): Extension[] {
    return this.extensionList.map((e) => e.extension);
  }

  /** Reads `core-plugins.json` and loads every plugin that is on. */
  async loadVault() {
    await this.serial(async () => {
      const saved = await readJsonRaw(ENABLED_FILE);
      this.enabled = saved && typeof saved === "object" ? (saved as Record<string, boolean>) : {};
      this.errors = {};
      for (const p of this.all) if (this.isEnabled(p.manifest.id)) await this.load(p);
      // Write the file once so it shows what is on, even before anything was changed.
      if (!saved) this.saveEnabled();
    });
  }

  /** Unloads everything (vault closed). */
  async unloadVault() {
    await this.serial(async () => {
      for (const id of [...this.loaded].reverse()) await this.unload(id);
      this.extra = [];
      this.errors = {};
    });
  }

  /** Loads a plugin from the store. Its id must not be taken by another plugin. */
  async loadExtra(plugin: Plugin) {
    await this.serial(async () => {
      const id = plugin.manifest.id;
      if (this.get(id)) throw new Error(`Ein Plugin mit der ID „${id}“ gibt es schon`);
      this.extra = [...this.extra, plugin];
      await this.load(plugin);
    });
  }

  /** Unloads a store plugin and forgets it (turned off or uninstalled). */
  async unloadExtra(id: string) {
    await this.serial(async () => {
      if (!this.extra.some((p) => p.manifest.id === id)) return;
      if (this.isLoaded(id)) await this.unload(id);
      this.extra = this.extra.filter((p) => p.manifest.id !== id);
      const { [id]: _, ...rest } = this.errors;
      this.errors = rest;
    });
  }

  /** Settings toggle: turns a plugin on or off and remembers it for this vault. */
  async setEnabled(id: string, on: boolean) {
    await this.serial(async () => {
      const p = this.all.find((p) => p.manifest.id === id);
      if (!p) return;
      this.enabled = { ...this.enabled, [id]: on };
      this.saveEnabled();
      if (on && !this.isLoaded(id)) await this.load(p);
      else if (!on && this.isLoaded(id)) await this.unload(id);
    });
  }

  /** Runs a command by its full id; false if it does not exist or may not run now. */
  run(fullId: string): boolean {
    const cmd = this.commandList.find((c) => c.fullId === fullId);
    if (!cmd || (cmd.when && !cmd.when())) return false;
    void cmd.run();
    return true;
  }

  has(fullId: string): boolean {
    return this.commandList.some((c) => c.fullId === fullId);
  }

  /** Finds the command bound to a key press, if any. */
  commandFor(e: KeyboardEvent, inModal: boolean): Command | undefined {
    return this.commandList.find(
      (c) => (!inModal || c.inModal) && c.hotkeys?.some((h) => matches(h, e)) && (!c.when || c.when()),
    );
  }

  // ------------------------------------------------------------------ internals

  /** Plugin changes run one after another so quick toggling can't interleave. */
  private serial(fn: () => Promise<void>): Promise<void> {
    const next = this.queue.then(fn, fn);
    this.queue = next.catch(() => undefined);
    return next;
  }

  private saveEnabled() {
    const out: Record<string, boolean> = { ...this.enabled };
    for (const p of this.all) out[p.manifest.id] = this.isEnabled(p.manifest.id);
    writeJson(ENABLED_FILE, out);
  }

  private async load(plugin: Plugin) {
    const id = plugin.manifest.id;
    this.disposers.set(id, []);
    const { [id]: _, ...rest } = this.errors;
    this.errors = rest;
    try {
      await plugin.onload(this.context(plugin.manifest));
      this.loaded = [...this.loaded, id];
    } catch (e) {
      console.error(`Plugin ${id} failed to load`, e);
      this.errors = { ...this.errors, [id]: String(e) };
      this.dispose(id);
    }
  }

  private async unload(id: string) {
    try {
      await this.get(id)?.onunload?.();
    } catch (e) {
      console.error(`Plugin ${id} failed to unload`, e);
    }
    this.dispose(id);
    this.loaded = this.loaded.filter((p) => p !== id);
  }

  private dispose(id: string) {
    for (const fn of (this.disposers.get(id) ?? []).reverse()) {
      try {
        fn();
      } catch (e) {
        console.error(e);
      }
    }
    this.disposers.delete(id);
  }

  private context(manifest: PluginManifest): PluginContext {
    const pluginId = manifest.id;
    const track = (fn: Dispose): Dispose => {
      let done = false;
      const once = () => {
        if (done) return;
        done = true;
        fn();
      };
      this.disposers.get(pluginId)?.push(once);
      return once;
    };
    /** Adds `item` to one of the lists and returns how to take it out again. */
    const addTo = <T extends Owned>(get: () => T[], set: (v: T[]) => void, item: T, key?: (t: T) => string) => {
      if (key && get().some((x) => key(x) === key(item))) throw new Error(`${pluginId}: "${key(item)}" is already registered`);
      set([...get(), item]);
      return track(() => set(get().filter((x) => x !== item)));
    };
    const dataFile = `plugins/${pluginId}/data.json`;

    return {
      manifest,
      addPanel: (panel) =>
        addTo(() => this.panelList, (v) => (this.panelList = v), { ...panel, pluginId }, (p) => p.id),
      addRibbonAction: (action) =>
        addTo(() => this.ribbonList, (v) => (this.ribbonList = v), { ...action, pluginId }, (a) => a.id),
      addCommand: (command) =>
        addTo(
          () => this.commandList,
          (v) => (this.commandList = v),
          { ...command, pluginId, fullId: `${pluginId}:${command.id}` },
          (c) => c.fullId,
        ),
      registerFileType: (type) => track(registerFileType(type)),
      registerEditorExtension: (extension) => {
        const entry = { pluginId, extension };
        this.extensionList = [...this.extensionList, entry];
        this.editorRevision++;
        return track(() => {
          this.extensionList = this.extensionList.filter((e) => e !== entry);
          this.editorRevision++;
        });
      },
      addFileFooter: (footer) =>
        addTo(() => this.footerList, (v) => (this.footerList = v), { ...footer, pluginId }, (f) => f.id),
      addPaneHeader: (header) =>
        addTo(() => this.headerList, (v) => (this.headerList = v), { ...header, pluginId }, (h) => h.id),
      addOverlay: (overlay) =>
        addTo(() => this.overlayList, (v) => (this.overlayList = v), { ...overlay, pluginId }, (o) => o.id),
      addSettingTab: (tab) =>
        addTo(() => this.settingList, (v) => (this.settingList = v), { ...tab, pluginId }, (t) => t.pluginId),
      registerView: (view) => {
        const remove = addTo(() => this.viewList, (v) => (this.viewList = v), { ...view, pluginId }, (v) => v.id);
        return track(() => {
          remove();
          for (const fn of this.viewRemovedListeners) fn(view.id);
        });
      },
      addExplorerEntries: (source) =>
        addTo(() => this.explorerEntryList, (v) => (this.explorerEntryList = v), { ...source, pluginId }, (s) => s.id),
      register: (fn) => void track(fn),
      loadData: (defaults) => readJson(dataFile, defaults),
      saveData: (data) => writeJson(dataFile, data),
    };
  }
}

function matches(h: Hotkey, e: KeyboardEvent): boolean {
  return (
    !!h.mod === (e.ctrlKey || e.metaKey) &&
    !!h.shift === e.shiftKey &&
    !!h.alt === e.altKey &&
    h.key.toLowerCase() === e.key.toLowerCase()
  );
}

/** Text for a hotkey, e.g. "Strg+Umschalt+Tab". */
export function hotkeyLabel(h: Hotkey): string {
  const mac = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform);
  const parts = [h.mod && (mac ? "Cmd" : "Strg"), h.shift && "Umschalt", h.alt && "Alt"].filter(Boolean) as string[];
  const names: Record<string, string> = { pagedown: "Bild ab", pageup: "Bild auf", tab: "Tab", ",": "," };
  parts.push(names[h.key.toLowerCase()] ?? h.key.toUpperCase());
  return parts.join("+");
}

export const plugins = new PluginManager();
