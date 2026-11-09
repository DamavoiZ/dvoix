// Built-in file types. To add a new one (e.g. ".board" for Kanban):
// create a folder next to these with a view + `index.ts` exporting a FileType,
// then register it here. Nothing else in the app needs to change.
import { registerFileType, setFallbackFileType } from "./registry";
import { markdownType } from "./markdown";
import { canvasType } from "./canvas";
import { fallbackType } from "./fallback";

registerFileType(markdownType);
registerFileType(canvasType);
setFallbackFileType(fallbackType);

export * from "./registry";
export type * from "./types";
