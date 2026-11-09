// Settings of the hidden-files plugin, in `.dvoix/plugins/hidden-files/data.json`.
import { pluginSettings } from "../../pluginSettings.svelte";

export const hiddenSettings = pluginSettings({
  /** Shown right now; flipped by the command "Show/hide hidden files". */
  visible: true,
  /** Also dot-files like `.gitignore`, not only dot-folders. */
  includeFiles: true,
  /** Also dvoix's own `.dvoix/` folder (settings, index). */
  includeAppDir: false,
  /** Names that stay hidden anyway. `.git` holds thousands of internal files. */
  exclude: [".git"] as string[],
});
