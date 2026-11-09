import { FileSearch } from "@lucide/svelte";
import type { Plugin } from "../../types";
import { ui } from "../../../ui.svelte";
import QuickSwitcher from "./QuickSwitcher.svelte";

export const QUICK_SWITCHER = "quick-switcher";

export const quickSwitcherPlugin: Plugin = {
  manifest: {
    id: QUICK_SWITCHER,
    name: "Schnellöffner",
    description: "Strg+O: Dateien per unscharfer Suche öffnen oder neue Notizen anlegen.",
    version: "1.0.0",
  },
  icon: FileSearch,
  onload(ctx) {
    ctx.addOverlay({ id: QUICK_SWITCHER, component: QuickSwitcher });
    ctx.addCommand({
      id: "open",
      name: "Schnellöffner",
      hotkeys: [{ mod: true, key: "o" }],
      inModal: true,
      run: () => ui.toggleModal(QUICK_SWITCHER),
    });
    ctx.register(() => {
      if (ui.modal === QUICK_SWITCHER) ui.modal = null;
    });
  },
};
