import type { Plugin } from "../../plugins/types";
import { editorToolbarExtensions } from "./toolbar";

export const editorToolbarFeature: Plugin = {
  manifest: {
    id: "editor-toolbar",
    name: "Formatting toolbar",
    description: "Floating formatting bar on text selection with Bold and Italic shortcuts.",
    version: "1.0.0",
  },
  onload(ctx) {
    ctx.registerEditorExtension(editorToolbarExtensions());
  },
};
