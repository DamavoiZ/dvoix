import { FolderTree } from "@lucide/svelte";
import type { Plugin } from "../../types";
import FileExplorer from "./FileExplorer.svelte";
import FileExplorerActions from "./FileExplorerActions.svelte";
import FileExplorerSettings from "./FileExplorerSettings.svelte";
import { explorerSettings } from "./settings";

export const fileExplorerPlugin: Plugin = {
  manifest: {
    id: "file-explorer",
    name: "File explorer",
    description: "Folder tree of the vault for opening, creating, renaming, moving and deleting.",
    version: "1.0.0",
  },
  icon: FolderTree,
  async onload(ctx) {
    await explorerSettings.load(ctx);
    ctx.addPanel({ id: "file-explorer", label: "Files", icon: FolderTree, component: FileExplorer, actions: FileExplorerActions, header: false, order: 10 });
    ctx.addSettingTab({ component: FileExplorerSettings });
  },
};
