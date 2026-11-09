import { Search } from "@lucide/svelte";
import type { Plugin } from "../../types";
import { ui } from "../../../ui.svelte";
import SearchPanel from "./SearchPanel.svelte";

export const searchPlugin: Plugin = {
  manifest: {
    id: "search",
    name: "Suche",
    description: "Volltextsuche über alle Notizen des Vaults.",
    version: "1.0.0",
  },
  icon: Search,
  onload(ctx) {
    ctx.addPanel({ id: "search", label: "Suche", icon: Search, component: SearchPanel, order: 20 });
    ctx.addCommand({
      id: "open",
      name: "Suche öffnen",
      hotkeys: [{ mod: true, shift: true, key: "f" }],
      run: () => ui.showPanel("search"),
    });
  },
};
