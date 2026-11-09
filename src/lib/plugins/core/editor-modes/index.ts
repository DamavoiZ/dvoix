// The three ways to show a note: reading view, live preview and the markdown
// source. The switch itself sits in the note view, this plugin adds the
// commands and the shortcuts for it.
import type { Plugin } from "../../types";
import { settings, type ViewMode } from "../../../settings.svelte";

function setMode(mode: ViewMode) {
  settings.app.viewMode = mode;
  settings.saveApp();
}

/** Ctrl+E switches between reading and editing, like in Obsidian. */
function toggleRead() {
  const current = settings.app.viewMode;
  setMode(current === "edit" ? "read" : "edit");
}

/** Ctrl+Shift+E switches between the source and the live preview. */
function toggleSource() {
  const current = settings.app.viewMode;
  setMode(current === "source" ? "edit" : "source");
}

export const editorModesPlugin: Plugin = {
  manifest: {
    id: "editor-modes",
    name: "Ansichten",
    description: "Read notes, edit them in the live preview or open them as source.",
    version: "1.0.0",
  },
  onload(ctx) {
    ctx.addCommand({ id: "read", name: "View: Read", run: () => setMode("read") });
    ctx.addCommand({ id: "edit", name: "View: Edit", run: () => setMode("edit") });
    ctx.addCommand({ id: "source", name: "View: Source", run: () => setMode("source") });
    ctx.addCommand({
      id: "toggle-read",
      name: "Toggle view (Read/Edit)",
      hotkeys: [{ mod: true, key: "e" }],
      run: toggleRead,
    });
    ctx.addCommand({
      id: "toggle-source",
      name: "Toggle view (Source/Edit)",
      hotkeys: [{ mod: true, shift: true, key: "e" }],
      run: toggleSource,
    });
  },
};
