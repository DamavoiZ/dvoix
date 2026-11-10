// Settings of the attachments feature, in `.dvoix/plugins/attachments/data.json`.
import { pluginSettings } from "../../plugins/pluginSettings.svelte";

export type AttachmentLocation = "root" | "folder" | "current" | "subfolder";

export const attachmentSettings = pluginSettings({
  /** Where pasted and dropped files are saved, with the same choices as Obsidian. */
  location: "root" as AttachmentLocation,
  /** The folder for "folder" (vault-relative) and the subfolder name for "subfolder". */
  folder: "attachments",
});
