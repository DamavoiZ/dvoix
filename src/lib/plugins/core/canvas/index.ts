import { LayoutDashboard } from "@lucide/svelte";
import type { FileType } from "../../../filetypes/types";
import type { Plugin } from "../../types";
import CanvasView from "./CanvasView.svelte";
import { serializeCanvas } from "./jsonCanvas";

export const canvasType: FileType = {
  id: "canvas",
  label: "Canvas",
  extensions: ["canvas"],
  icon: LayoutDashboard,
  view: CanvasView,
  template: () => serializeCanvas({ nodes: [], edges: [] }),
};

export const canvasPlugin: Plugin = {
  manifest: {
    id: "canvas",
    name: "Canvas",
    description: "Dateityp .canvas im offenen JSON-Canvas-Format: Karten frei auf einer Fläche anordnen.",
    version: "1.0.0",
  },
  icon: LayoutDashboard,
  onload(ctx) {
    ctx.registerFileType(canvasType);
  },
};
