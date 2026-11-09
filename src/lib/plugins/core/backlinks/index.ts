import { Link } from "@lucide/svelte";
import type { Plugin } from "../../types";
import Backlinks from "./Backlinks.svelte";

export const backlinksPlugin: Plugin = {
  manifest: {
    id: "backlinks",
    name: "Backlinks",
    description: "Zeigt unter einer Notiz, welche anderen Notizen auf sie verlinken.",
    version: "1.0.0",
  },
  icon: Link,
  onload(ctx) {
    ctx.addFileFooter({ id: "backlinks", component: Backlinks });
  },
};
