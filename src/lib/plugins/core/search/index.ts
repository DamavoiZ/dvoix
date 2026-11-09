import { Search } from "@lucide/svelte";
import type { Plugin } from "../../types";
import { ui } from "../../../ui.svelte";
import SearchPanel from "./SearchPanel.svelte";

export const searchPlugin: Plugin = {
  manifest: {
    id: "search",
    name: "Search",
    description: "Full-text search across all notes in the vault.",
    version: "1.0.0",
  },
  icon: Search,
  onload(ctx) {
    ctx.addPanel({ id: "search", label: "Search", icon: Search, component: SearchPanel, order: 20 });
    ctx.addCommand({
      id: "open",
      name: "Open search",
      hotkeys: [{ mod: true, shift: true, key: "f" }],
      run: () => ui.showPanel("search"),
    });
  },
};
