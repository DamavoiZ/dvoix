import type { FileType } from "../types";
import CanvasView from "./CanvasView.svelte";
import { serializeCanvas } from "./jsonCanvas";

export const canvasType: FileType = {
  id: "canvas",
  label: "Canvas",
  extensions: ["canvas"],
  icon: "▦",
  view: CanvasView,
  template: () => serializeCanvas({ nodes: [], edges: [] }),
};
