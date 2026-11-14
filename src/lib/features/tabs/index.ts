import { PanelTop } from "@lucide/svelte";
import type { Plugin } from "../../plugins/types";
import { vault } from "../../vault.svelte";
import TabBar from "./TabBar.svelte";

export const tabsFeature: Plugin = {
  manifest: {
    id: "tabs",
    name: "Tabs",
    description: "Keep several files open in tabs.",
    version: "1.0.0",
  },
  icon: PanelTop,
  onload(ctx) {
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
    ctx.addCommand({
      id: "back",
      name: "Navigate back",
      hotkeys: [{ alt: true, key: "ArrowLeft" }],
      when: () => vault.canGoBack,
      run: () => vault.goBack(),
    });
    ctx.addCommand({
      id: "forward",
      name: "Navigate forward",
      hotkeys: [{ alt: true, key: "ArrowRight" }],
      when: () => vault.canGoForward,
      run: () => vault.goForward(),
    });
  },
};
