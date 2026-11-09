import { pluginSettings } from "../../pluginSettings.svelte";

export const explorerSettings = pluginSettings({
  /** Where the toolbar buttons create new files: next to the open file, or in the vault root. */
  newFileLocation: "current" as "current" | "root",
});
