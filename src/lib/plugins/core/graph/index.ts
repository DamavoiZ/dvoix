import { Waypoints } from "@lucide/svelte";
import type { Plugin } from "../../types";
import { vault } from "../../../vault.svelte";
import GraphOptions from "./GraphOptions.svelte";
import GraphView from "./GraphView.svelte";
import { graphSettings } from "./settings";

const VIEW = "graph";

export const graphPlugin: Plugin = {
  manifest: {
    id: "graph",
    name: "Graph",
    description: "Zeigt die Notizen als Netz, verbunden über Links, Ordner oder Tags.",
    version: "1.0.0",
  },
  icon: Waypoints,
  async onload(ctx) {
    await graphSettings.load(ctx);
    ctx.registerView({ id: VIEW, label: "Graph", icon: Waypoints, component: GraphView });
    ctx.addSettingTab({ component: GraphOptions });
    const open = () => vault.openViewTab(VIEW, { newTab: !!vault.openPath });
    ctx.addRibbonAction({ id: "open", label: "Graph öffnen", icon: Waypoints, action: () => void open(), order: 60 });
    ctx.addCommand({ id: "open", name: "Graph öffnen", hotkeys: [{ mod: true, key: "g" }], when: () => !!vault.info, run: open });
  },
};
