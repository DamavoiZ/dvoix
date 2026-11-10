import { pluginSettings } from "../../pluginSettings.svelte";

export type TaskShow = "open" | "done" | "all";
export type TaskGroup = "due" | "note";

export const taskSettings = pluginSettings({
  /** Highlight due dates and priorities in the editor. */
  highlightMeta: true,
  /** Last chosen filter and grouping of the task panel. */
  show: "open" as TaskShow,
  group: "due" as TaskGroup,
});
