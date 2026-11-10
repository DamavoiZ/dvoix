// Attachments: paste or drop images and other files into a note. They are saved
// in the vault and embedded with `![[name]]`, as in Obsidian. A basic note
// function, so a core feature; showing the embeds is part of the markdown views.
import { Paperclip } from "@lucide/svelte";
import type { Plugin } from "../../plugins/types";
import AttachmentSettings from "./AttachmentSettings.svelte";
import { attachmentExtensions } from "./editor";
import { attachmentSettings } from "./settings";

export const attachmentsFeature: Plugin = {
  manifest: {
    id: "attachments",
    name: "Attachments",
    description: "Saves images and files pasted or dropped into a note and embeds them.",
    version: "1.0.0",
  },
  icon: Paperclip,
  async onload(ctx) {
    await attachmentSettings.load(ctx);
    ctx.addSettingTab({ component: AttachmentSettings });
    ctx.registerEditorExtension(attachmentExtensions());
  },
};
