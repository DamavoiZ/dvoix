import type { FileType } from "../types";
import { LayoutDashboard } from "@lucide/svelte";
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
