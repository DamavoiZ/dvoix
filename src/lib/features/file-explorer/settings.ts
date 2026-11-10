import { pluginSettings } from "../../plugins/pluginSettings.svelte";

export const explorerSettings = pluginSettings({
  /** Where the toolbar buttons create new files: next to the open file, in the vault root or in `newFileFolder`. */
  newFileLocation: "current" as "current" | "root" | "folder",
  /** Vault-relative folder for `newFileLocation: "folder"`; created when needed. */
  newFileFolder: "",
});
