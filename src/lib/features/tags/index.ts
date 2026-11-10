import { Hash } from "@lucide/svelte";
import type { Plugin } from "../../plugins/types";
import TagsPanel from "./TagsPanel.svelte";

export const tagsFeature: Plugin = {
  manifest: {
    id: "tags",
    name: "Tags",
    description: "Overview of all #tags in the vault with their notes.",
    version: "1.0.0",
  },
  icon: Hash,
  onload(ctx) {
    ctx.addPanel({ id: "tags", label: "Tags", icon: Hash, component: TagsPanel, order: 30 });
  },
};
