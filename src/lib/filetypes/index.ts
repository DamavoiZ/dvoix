// File types. Markdown (and the fallback for unknown files) are built in;
// everything else comes from plugins via `ctx.registerFileType(...)`, see
// `src/lib/plugins/core/canvas` for an example.
import { registerFileType, setFallbackFileType } from "./registry";
import { markdownType } from "./markdown";
import { fallbackType } from "./fallback";

registerFileType(markdownType);
setFallbackFileType(fallbackType);

export * from "./registry";
export type * from "./types";
