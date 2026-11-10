// Entry point of the plugin system: registers the core features and all core
// plugins with the manager. Plugins from the store are loaded per vault by
// `store/store.svelte.ts`.
import { plugins } from "./manager.svelte";
import { coreFeatures } from "../features";
import { corePlugins } from "./core";

for (const f of coreFeatures) plugins.addFeature(f);
for (const p of corePlugins) plugins.add(p);

export { plugins, hotkeyLabel } from "./manager.svelte";
export { pluginStore } from "./store/store.svelte";
export type * from "./types";
