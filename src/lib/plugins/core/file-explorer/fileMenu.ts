import { FolderPlus, PanelTop, Pencil, Trash2 } from "@lucide/svelte";
import type { VaultEntry } from "../../../api";
import { creatableFileTypes } from "../../../filetypes";
import { ui } from "../../../ui.svelte";
import { vault } from "../../../vault.svelte";
import type { MenuEntry } from "../../../components/contextMenuState.svelte";

/** "New note", "New canvas", ..., "New folder" inside `folder` ("" = vault root). */
export function createItems(folder: string): MenuEntry[] {
  return [
    ...creatableFileTypes().map((type) => ({
      label: `Create ${type.label}`,
      icon: type.icon,
      action: () => void vault.createFile(type, folder),
    })),
    { label: "Create folder", icon: FolderPlus, action: () => void newFolder(folder) },
  ];
}

export async function newFolder(parent: string) {
  const path = await vault.createFolder(parent);
  if (path) ui.renaming = path;
}

export function entryMenu(entry: VaultEntry): MenuEntry[] {
  return [
    ...(entry.isDir
      ? [...createItems(entry.path), "separator" as const]
      : [
          ...(vault.tabsEnabled
            ? [{ label: "Open in new tab", icon: PanelTop, action: () => void vault.openFile(entry.path, { newTab: true }) }]
            : []),
          "separator" as const,
        ]),
    { label: "Rename", icon: Pencil, action: () => (ui.renaming = entry.path) },
    { label: "Delete", icon: Trash2, danger: true, action: () => void vault.trash(entry.path) },
  ];
}

export const DRAG_TYPE = "application/x-dvoix-path";
