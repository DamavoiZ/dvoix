import { ListTree, PanelRight } from "@lucide/svelte";
import type { Plugin } from "../../types";
import { vault } from "../../../vault.svelte";
import Outline from "./Outline.svelte";
import RightSidebar from "./RightSidebar.svelte";
import RightSidebarSettings from "./RightSidebarSettings.svelte";
import ToggleButton from "./ToggleButton.svelte";
import { OUTLINE_PANEL, rightSidebarSettings, toggleColumn } from "./settings";

export const rightSidebarPlugin: Plugin = {
  manifest: {
    id: "right-sidebar",
    name: "Right sidebar",
    description: "Shows the outline of the open note and other panels on the right edge, where notes can be docked.",
    version: "1.0.0",
  },
  icon: PanelRight,
  async onload(ctx) {
    await rightSidebarSettings.load(ctx);
    // The first time the plugin is on, the outline is the panel in the sidebar.
    if (!rightSidebarSettings.value.configured) {
      rightSidebarSettings.value.configured = true;
      rightSidebarSettings.value.items = [{ kind: "panel", id: OUTLINE_PANEL }];
      rightSidebarSettings.save();
    }

    ctx.addRightSidebar({ id: "right-sidebar", component: RightSidebar, toggle: ToggleButton });
    ctx.addPanel({ id: OUTLINE_PANEL, label: "Outline", icon: ListTree, component: Outline, side: "right" });
    ctx.addSettingTab({ component: RightSidebarSettings });
    ctx.addCommand({
      id: "toggle",
      name: "Toggle right sidebar",
      hotkeys: [{ mod: true, key: "]" }],
      when: () => !!vault.info,
      run: toggleColumn,
    });
  },
};
