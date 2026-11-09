import { PanelTop } from "@lucide/svelte";
import type { Plugin } from "../../types";
import { vault } from "../../../vault.svelte";
import TabBar from "./TabBar.svelte";

export const tabsPlugin: Plugin = {
  manifest: {
    id: "tabs",
    name: "Tabs",
    description: "Keep several files open in tabs. When turned off, only one file is open at a time.",
    version: "1.0.0",
  },
  icon: PanelTop,
  onload(ctx) {
    vault.setTabsEnabled(true);
    ctx.register(() => vault.setTabsEnabled(false));
    ctx.addPaneHeader({ id: "tab-bar", component: TabBar });
    ctx.addCommand({ id: "new", name: "New tab", hotkeys: [{ mod: true, key: "t" }], run: () => vault.newTab() });
    ctx.addCommand({ id: "close", name: "Close tab", hotkeys: [{ mod: true, key: "w" }], run: () => vault.closeTab(vault.activeTabId) });
    ctx.addCommand({
      id: "next",
      name: "Next tab",
      hotkeys: [{ mod: true, key: "Tab" }, { mod: true, key: "PageDown" }],
      run: () => vault.cycleTab(1),
    });
    ctx.addCommand({
      id: "previous",
      name: "Previous tab",
      hotkeys: [{ mod: true, shift: true, key: "Tab" }, { mod: true, key: "PageUp" }],
      run: () => vault.cycleTab(-1),
    });
  },
};
