import { FolderTree } from "@lucide/svelte";
import type { Plugin } from "../../types";
import FileExplorer from "./FileExplorer.svelte";
import FileExplorerSettings from "./FileExplorerSettings.svelte";
import { explorerSettings } from "./settings";

export const fileExplorerPlugin: Plugin = {
  manifest: {
    id: "file-explorer",
    name: "Datei-Explorer",
    description: "Ordnerbaum des Vaults zum Öffnen, Anlegen, Umbenennen, Verschieben und Löschen.",
    version: "1.0.0",
  },
  icon: FolderTree,
  async onload(ctx) {
    await explorerSettings.load(ctx);
    ctx.addPanel({ id: "file-explorer", label: "Dateien", icon: FolderTree, component: FileExplorer, header: false, order: 10 });
    ctx.addSettingTab({ component: FileExplorerSettings });
  },
};
