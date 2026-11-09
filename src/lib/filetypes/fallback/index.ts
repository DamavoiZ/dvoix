import type { FileType } from "../types";
import { File } from "@lucide/svelte";
import FallbackView from "./FallbackView.svelte";

export const fallbackType: FileType = {
  id: "fallback",
  label: "File",
  extensions: [],
  icon: File,
  view: FallbackView,
};
