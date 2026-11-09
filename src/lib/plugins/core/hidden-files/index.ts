// Shows dot-folders like `.obsidian` or `.trash` (and dot-files) in the file
// explorer. They stay out of the index, search, graph and quick switcher.
import { EyeOff } from "@lucide/svelte";
import type { Plugin } from "../../types";
import HiddenFilesSettings from "./HiddenFilesSettings.svelte";
import { hiddenSettings } from "./settings";
import { hidden, refresh, watch } from "./state.svelte";

export const hiddenFilesPlugin: Plugin = {
  manifest: {
    id: "hidden-files",
    name: "Hidden files",
    description: "Shows folders and files whose name starts with a dot (e.g. .obsidian) in the file explorer.",
    version: "1.0.0",
    enabledByDefault: false,
  },
  icon: EyeOff,
  async onload(ctx) {
    await hiddenSettings.load(ctx);
    ctx.addExplorerEntries({ id: "hidden-files", entries: () => hidden.entries });
    ctx.addSettingTab({ component: HiddenFilesSettings });
    ctx.addCommand({
      id: "toggle",
      name: "Show/hide hidden files",
      run: () => {
        hiddenSettings.value.visible = !hiddenSettings.value.visible;
        hiddenSettings.save();
      },
    });
    ctx.register(hiddenSettings.onChange(() => void refresh()));
    ctx.register(await watch());
    await refresh();
  },
};
