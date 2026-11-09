import type { FileType } from "../types";
import FallbackView from "./FallbackView.svelte";

export const fallbackType: FileType = {
  id: "fallback",
  label: "Datei",
  extensions: [],
  icon: "·",
  view: FallbackView,
};
