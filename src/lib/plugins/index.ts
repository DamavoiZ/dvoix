// Entry point of the plugin system: registers all core plugins with the manager.
import { plugins } from "./manager.svelte";
import { corePlugins } from "./core";

for (const p of corePlugins) plugins.add(p);

export { plugins, hotkeyLabel } from "./manager.svelte";
export type * from "./types";
