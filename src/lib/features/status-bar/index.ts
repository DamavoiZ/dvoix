import type { Plugin } from "../../plugins/types";
import StatusBarLeft from "./StatusBarLeft.svelte";
import StatusBarRight from "./StatusBarRight.svelte";

export const statusBarFeature: Plugin = {
  manifest: {
    id: "status-bar",
    name: "Status bar",
    description: "Shows word count, reading time, cursor position, and view mode in the bottom bar.",
    version: "1.0.0",
  },
  onload(ctx) {
    ctx.addStatusBarItem({ id: "stats", component: StatusBarLeft, align: "left", order: 10 });
    ctx.addStatusBarItem({ id: "editor", component: StatusBarRight, align: "right", order: 10 });
  },
};
