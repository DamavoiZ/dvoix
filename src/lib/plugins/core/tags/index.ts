import { Hash } from "@lucide/svelte";
import type { Plugin } from "../../types";
import TagsPanel from "./TagsPanel.svelte";

export const tagsPlugin: Plugin = {
  manifest: {
    id: "tags",
    name: "Tags",
    description: "Übersicht aller #tags im Vault mit den Notizen dazu.",
    version: "1.0.0",
  },
  icon: Hash,
  onload(ctx) {
    ctx.addPanel({ id: "tags", label: "Tags", icon: Hash, component: TagsPanel, order: 30 });
  },
};
