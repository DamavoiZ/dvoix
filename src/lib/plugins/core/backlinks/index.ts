import { Link } from "@lucide/svelte";
import type { Plugin } from "../../types";
import Backlinks from "./Backlinks.svelte";

export const backlinksPlugin: Plugin = {
  manifest: {
    id: "backlinks",
    name: "Backlinks",
    description: "Shows below a note which other notes link to it.",
    version: "1.0.0",
  },
  icon: Link,
  onload(ctx) {
    ctx.addFileFooter({ id: "backlinks", component: Backlinks });
  },
};
