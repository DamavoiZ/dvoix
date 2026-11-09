import { FolderPlus, Pencil, Trash2 } from "@lucide/svelte";
import type { VaultEntry } from "../api";
import { creatableFileTypes } from "../filetypes";
import { ui } from "../ui.svelte";
import { vault } from "../vault.svelte";
import type { MenuEntry } from "./contextMenu.svelte";

/** "Neue Notiz", "Neues Canvas", ..., "Neuer Ordner" inside `folder` ("" = vault root). */
export function createItems(folder: string): MenuEntry[] {
  return [
    ...creatableFileTypes().map((type) => ({
      label: `${type.label} erstellen`,
      icon: type.icon,
      action: () => void vault.createFile(type, folder),
    })),
    { label: "Ordner erstellen", icon: FolderPlus, action: () => void newFolder(folder) },
  ];
}

export async function newFolder(parent: string) {
  const path = await vault.createFolder(parent);
  if (path) ui.renaming = path;
}

export function entryMenu(entry: VaultEntry): MenuEntry[] {
  return [
    ...(entry.isDir ? [...createItems(entry.path), "separator" as const] : []),
    { label: "Umbenennen", icon: Pencil, action: () => (ui.renaming = entry.path) },
    { label: "Löschen", icon: Trash2, danger: true, action: () => void vault.trash(entry.path) },
  ];
}

export const DRAG_TYPE = "application/x-dvoix-path";
