// Suggests notes while typing a `[[wikilink]]` in the editor.
import { Link } from "@lucide/svelte";
import type { Plugin } from "../../types";
import { linkSuggestExtensions } from "./editor";

export const linkSuggestPlugin: Plugin = {
  manifest: {
    id: "link-suggest",
    name: "Link suggestions",
    description: "Suggests matching notes and files when you type [[.",
    version: "1.0.0",
  },
  icon: Link,
  onload(ctx) {
    ctx.registerEditorExtension(linkSuggestExtensions());
  },
};
