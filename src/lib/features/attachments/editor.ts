// Pasting or dropping files into the markdown editor: each file is saved in the
// vault (where the settings say) and `![[name]]` is inserted where it landed,
// the way Obsidian does it.
import type { Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { api } from "../../api";
import { cleanFolder, folderOf } from "../../attachments";
import { vault } from "../../vault.svelte";
import { attachmentSettings } from "./settings";

/** The folder new attachments of the open note go to. */
function targetFolder(): string {
  const { location, folder } = attachmentSettings.value;
  const here = folderOf(vault.openPath);
  const sub = cleanFolder(folder);
  switch (location) {
    case "folder":
      return sub;
    case "current":
      return here;
    case "subfolder":
      return [here, sub || "attachments"].filter(Boolean).join("/");
    default:
      return "";
  }
}

/** A pasted screenshot has no real name, so it gets one with the time, like in Obsidian. */
function fileName(file: File): string {
  const name = file.name.replace(/[\\/:*?"<>|]/g, "-").trim();
  if (name && name !== "image.png") return name;
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  const stamp = `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`;
  const ext = file.type.split("/")[1]?.replace("jpeg", "jpg").replace("svg+xml", "svg") || "png";
  return `Pasted image ${stamp}.${ext}`;
}

/** `photo.png`, or `photo 1.png`, `photo 2.png`, ... if the name is taken in `folder`. */
function freePath(folder: string, name: string): string {
  const taken = new Set(vault.entries.map((e) => e.path.toLowerCase()));
  const dot = name.lastIndexOf(".");
  const base = dot > 0 ? name.slice(0, dot) : name;
  const ext = dot > 0 ? name.slice(dot) : "";
  const prefix = folder ? `${folder}/` : "";
  let path = `${prefix}${name}`;
  for (let n = 1; taken.has(path.toLowerCase()); n++) path = `${prefix}${base} ${n}${ext}`;
  return path;
}

/** Saves the files and inserts an embed for each at `pos`. */
async function insertFiles(view: EditorView, files: File[], pos: number): Promise<void> {
  const links: string[] = [];
  for (const file of files) {
    const path = freePath(targetFolder(), fileName(file));
    try {
      await api.createBinary(path, new Uint8Array(await file.arrayBuffer()));
      links.push(path);
    } catch (e) {
      vault.error = `Could not save ${file.name || "the file"}: ${e}`;
    }
  }
  if (!links.length) return;
  await vault.refresh();
  // The short name is enough while it is unique in the vault; else the full path.
  const text = links
    .map((path) => {
      const name = path.split("/").pop()!;
      const unique = vault.entries.filter((e) => !e.isDir && e.name.toLowerCase() === name.toLowerCase()).length === 1;
      return `![[${unique ? name : path}]]`;
    })
    .join("\n");
  const at = Math.min(pos, view.state.doc.length);
  view.dispatch({
    changes: { from: at, insert: text },
    selection: { anchor: at + text.length },
    userEvent: "input.paste",
  });
}

export function attachmentExtensions(): Extension {
  return EditorView.domEventHandlers({
    paste(event, view) {
      const files = [...(event.clipboardData?.files ?? [])];
      if (!files.length) return false;
      event.preventDefault();
      const { from, to } = view.state.selection.main;
      if (from !== to) view.dispatch({ changes: { from, to }, selection: { anchor: from } });
      void insertFiles(view, files, from);
      return true;
    },
    drop(event, view) {
      const files = [...(event.dataTransfer?.files ?? [])];
      if (!files.length) return false;
      event.preventDefault();
      const pos = view.posAtCoords({ x: event.clientX, y: event.clientY }) ?? view.state.selection.main.head;
      void insertFiles(view, files, pos);
      return true;
    },
  });
}
