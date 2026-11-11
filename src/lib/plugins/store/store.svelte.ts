// The plugin store: which plugins are installed from it, which of them are on,
// installing, updating and removing them, and loading their code.
//
// A store plugin lives in `.dvoix/plugins/<id>/` (manifest.json, main.js,
// styles.css); what is on and where it came from is in `.dvoix/store.json`:
//
//   { "allowCommunity": false,
//     "plugins": { "kanban": { "enabled": true, "repo": "dvoi-dev/dvoix-kanban", "channel": "official" } } }
//
// Official plugins can be installed right away. Community plugins only once
// `allowCommunity` is switched on; switching it off again unloads them.
import { getVersion } from "@tauri-apps/api/app";
import { api, type InstalledPlugin, type RegistryEntry, type StoreManifest } from "../../api";
import { readJsonRaw, writeJson } from "../../config";
import { plugins } from "../manager.svelte";
import type { Plugin } from "../types";
import { createApi } from "./api";
import { iconComponent } from "./component";

const STORE_FILE = "store.json";

export type Channel = RegistryEntry["channel"];

interface StoreRecord {
  enabled: boolean;
  repo?: string;
  channel: Channel;
}

interface StoreFile {
  allowCommunity: boolean;
  plugins: Record<string, StoreRecord>;
}

/** `a` is newer than `b` (both `major.minor.patch`). */
export function isNewer(a: string, b: string): boolean {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    if ((pa[i] ?? 0) !== (pb[i] ?? 0)) return (pa[i] ?? 0) > (pb[i] ?? 0);
  }
  return false;
}

/** What a store plugin's `main.js` must export by default. */
interface PluginModule {
  onload(api: ReturnType<typeof createApi>): void | Promise<void>;
  onunload?(): void | Promise<void>;
}

/** Runs `main.js` as an ES module. A new URL each time, so turning a plugin
 *  off and on again (or updating it) starts with fresh module state. */
async function importModule(id: string, code: string): Promise<unknown> {
  const url = URL.createObjectURL(
    new Blob([`${code}\n//# sourceURL=dvoix-plugin://${id}/main.js`], { type: "text/javascript" }),
  );
  try {
    return await import(/* @vite-ignore */ url);
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** The `Plugin` the manager loads for an installed store plugin. */
function wrap(installed: InstalledPlugin): Plugin {
  const m = installed.manifest;
  let instance: PluginModule | undefined;
  return {
    manifest: { id: m.id, name: m.name, description: m.description, version: m.version, author: m.author },
    icon: iconComponent(undefined),
    async onload(ctx) {
      // The code and the styles do not depend on each other: read both at once.
      const [code, css] = await Promise.all([
        api.readConfig(`plugins/${m.id}/main.js`),
        installed.hasStyles ? api.readConfig(`plugins/${m.id}/styles.css`) : Promise.resolve(null),
      ]);
      if (code === null) throw new Error("main.js is missing");
      if (css) {
        const style = document.createElement("style");
        style.dataset.dvoixPlugin = m.id;
        style.textContent = css;
        document.head.append(style);
        ctx.register(() => style.remove());
      }
      const mod = (await importModule(m.id, code)) as { default?: PluginModule };
      const def = mod.default;
      if (!def || typeof def.onload !== "function") {
        throw new Error("main.js must export an object with onload() as its default export");
      }
      instance = def;
      await def.onload(createApi(ctx, m));
    },
    async onunload() {
      const i = instance;
      instance = undefined;
      await i?.onunload?.();
    },
  };
}

class PluginStore {
  /** Store plugins found in `.dvoix/plugins/`. */
  installed = $state<InstalledPlugin[]>([]);
  file = $state<StoreFile>({ allowCommunity: false, plugins: {} });
  /** The plugin list, once loaded. */
  registry = $state<RegistryEntry[] | null>(null);
  registryError = $state<string | null>(null);
  loadingRegistry = $state(false);
  /** Newest release manifest per plugin id, from `checkUpdates` or the details. */
  latest = $state<Record<string, StoreManifest>>({});
  /** What is running for a plugin right now, e.g. "Installing …". */
  busy = $state<Record<string, string>>({});
  /** Last error of an install, update or uninstall, by plugin id. */
  errors = $state<Record<string, string>>({});
  appVersion = $state("0.0.0");

  /** Reads `store.json` and loads every store plugin that is on. */
  async loadVault() {
    // Independent reads run at once instead of one after another.
    const [raw, version] = await Promise.all([readJsonRaw(STORE_FILE), getVersion().catch(() => "0.0.0")]);
    const f = (raw && typeof raw === "object" ? raw : {}) as Partial<StoreFile>;
    this.file = {
      allowCommunity: f.allowCommunity === true,
      plugins: f.plugins && typeof f.plugins === "object" ? f.plugins : {},
    };
    this.appVersion = version;
    await this.refreshInstalled();
    // Activations run one after another: the manager queues them anyway and
    // a plugin may rely on the ones before it being loaded.
    for (const p of this.installed) if (this.isEnabled(p.manifest.id)) await this.activate(p);
  }

  /** Vault closed; the manager has unloaded the plugins already. */
  reset() {
    this.installed = [];
    this.file = { allowCommunity: false, plugins: {} };
    this.latest = {};
    this.busy = {};
    this.errors = {};
  }

  /** Official if installed from the store as official; anything else (also
   *  plugins copied into the folder by hand) counts as community. */
  channelOf(id: string): Channel {
    return this.file.plugins[id]?.channel ?? "community";
  }

  /** Community plugins are blocked while they are not allowed. */
  isBlocked(id: string): boolean {
    return this.channelOf(id) === "community" && !this.file.allowCommunity;
  }

  isEnabled(id: string): boolean {
    return (this.file.plugins[id]?.enabled ?? false) && !this.isBlocked(id) && !this.tooNew(id);
  }

  isInstalled(id: string): boolean {
    return this.installed.some((p) => p.manifest.id === id);
  }

  installedManifest(id: string): StoreManifest | undefined {
    return this.installed.find((p) => p.manifest.id === id)?.manifest;
  }

  /** Needs a newer dvoix than this one. */
  needsNewerApp(m: StoreManifest | undefined): boolean {
    return !!m?.minAppVersion && isNewer(m.minAppVersion, this.appVersion);
  }

  private tooNew(id: string): boolean {
    return this.needsNewerApp(this.installedManifest(id));
  }

  hasUpdate(id: string): boolean {
    const now = this.installedManifest(id);
    const next = this.latest[id];
    return !!now && !!next && isNewer(next.version, now.version);
  }

  /** The id is taken by a core feature or a core plugin. */
  clashesWithCore(id: string): boolean {
    return plugins.isBuiltIn(id);
  }

  async setEnabled(id: string, on: boolean) {
    const rec = this.file.plugins[id] ?? { enabled: false, channel: "community" as Channel };
    this.file.plugins[id] = { ...rec, enabled: on };
    this.save();
    if (on && this.isEnabled(id)) {
      const p = this.installed.find((p) => p.manifest.id === id);
      if (p) await this.activate(p);
    } else {
      await plugins.unloadExtra(id);
    }
  }

  /** The switch for community plugins. Off unloads them right away. */
  async setAllowCommunity(on: boolean) {
    this.file.allowCommunity = on;
    this.save();
    for (const p of this.installed) {
      const id = p.manifest.id;
      if (this.channelOf(id) !== "community") continue;
      if (on && this.isEnabled(id)) await this.activate(p);
      else if (!on) await plugins.unloadExtra(id);
    }
  }

  async fetchRegistry() {
    this.loadingRegistry = true;
    this.registryError = null;
    try {
      this.registry = await api.storeRegistry();
    } catch (e) {
      this.registryError = String(e);
    } finally {
      this.loadingRegistry = false;
    }
  }

  /** The newest release of a plugin, for the details and to see updates. */
  async fetchLatest(entry: RegistryEntry): Promise<StoreManifest | undefined> {
    try {
      const m = await api.storeLatestManifest(entry.repo);
      this.latest = { ...this.latest, [entry.id]: m };
      return m;
    } catch (e) {
      this.errors = { ...this.errors, [entry.id]: String(e) };
    }
  }

  /** Looks for newer releases of all installed plugins that came from the store. */
  async checkUpdates() {
    if (!this.registry) await this.fetchRegistry();
    await Promise.all(
      this.installed.map((p) => {
        const entry = this.entryFor(p.manifest.id);
        return entry ? this.fetchLatest(entry) : undefined;
      }),
    );
  }

  /** The registry entry of an installed plugin, by the repository it came from. */
  entryFor(id: string): RegistryEntry | undefined {
    const repo = this.file.plugins[id]?.repo;
    return this.registry?.find((e) => e.id === id && (!repo || e.repo === repo));
  }

  async install(entry: RegistryEntry) {
    if (entry.channel === "community" && !this.file.allowCommunity) return;
    await this.run(entry.id, "Installing …", async () => {
      await api.storeInstall(entry.repo, entry.id);
      this.file.plugins[entry.id] = { enabled: true, repo: entry.repo, channel: entry.channel };
      this.save();
      await this.refreshInstalled();
      const p = this.installed.find((p) => p.manifest.id === entry.id);
      if (p) this.latest = { ...this.latest, [entry.id]: p.manifest };
      if (p && this.isEnabled(entry.id)) await this.activate(p);
    });
  }

  async update(id: string) {
    const entry = this.entryFor(id);
    if (!entry) return;
    await this.run(id, "Updating …", async () => {
      await plugins.unloadExtra(id);
      try {
        await api.storeInstall(entry.repo, id);
        // The registry may have changed the channel since the install.
        const rec = this.file.plugins[id];
        this.file.plugins[id] = { enabled: rec?.enabled ?? true, repo: entry.repo, channel: entry.channel };
        this.save();
      } finally {
        await this.refreshInstalled();
        const p = this.installed.find((p) => p.manifest.id === id);
        if (p && this.isEnabled(id)) await this.activate(p);
      }
    });
  }

  async uninstall(id: string) {
    await this.run(id, "Removing …", async () => {
      await plugins.unloadExtra(id);
      await api.storeUninstall(id);
      const { [id]: _, ...rest } = this.file.plugins;
      this.file.plugins = rest;
      this.save();
      await this.refreshInstalled();
    });
  }

  // ------------------------------------------------------------------ internals

  private async run(id: string, label: string, fn: () => Promise<void>) {
    if (this.busy[id]) return;
    this.busy = { ...this.busy, [id]: label };
    const { [id]: _, ...rest } = this.errors;
    this.errors = rest;
    try {
      await fn();
    } catch (e) {
      this.errors = { ...this.errors, [id]: String(e) };
    } finally {
      const { [id]: _b, ...others } = this.busy;
      this.busy = others;
    }
  }

  private async refreshInstalled() {
    this.installed = await api.storeInstalled().catch(() => []);
  }

  private async activate(p: InstalledPlugin) {
    const id = p.manifest.id;
    if (plugins.isLoaded(id) || plugins.extra.some((x) => x.manifest.id === id)) return;
    if (this.clashesWithCore(id)) {
      this.errors = { ...this.errors, [id]: `The ID "${id}" already belongs to a core plugin` };
      return;
    }
    await plugins.loadExtra(wrap(p)).catch((e) => {
      this.errors = { ...this.errors, [id]: String(e) };
    });
  }

  private save() {
    writeJson(STORE_FILE, $state.snapshot(this.file));
  }
}

export const pluginStore = new PluginStore();
