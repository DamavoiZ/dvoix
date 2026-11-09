import { ListChecks } from "@lucide/svelte";
import type { Plugin } from "../../types";
import TasksPanel from "./TasksPanel.svelte";
import TasksSettings from "./TasksSettings.svelte";
import { taskEditorExtensions } from "./editor";
import { taskSettings } from "./settings";

export const tasksPlugin: Plugin = {
  manifest: {
    id: "tasks",
    name: "Aufgaben",
    description: "Markdown-Checkboxen als Aufgaben: anklickbar im Editor und gesammelt in einer Übersicht mit Fälligkeit und Priorität.",
    version: "1.0.0",
  },
  icon: ListChecks,
  async onload(ctx) {
    await taskSettings.load(ctx);
    ctx.addPanel({ id: "tasks", label: "Aufgaben", icon: ListChecks, component: TasksPanel, order: 40 });
    ctx.addSettingTab({ component: TasksSettings });

    // Editor extensions follow the settings: rebuilt when a toggle changes.
    let removeExtensions = ctx.registerEditorExtension(taskEditorExtensions(taskSettings.value));
    let last = JSON.stringify([taskSettings.value.checkboxes, taskSettings.value.highlightMeta]);
    ctx.register(
      taskSettings.onChange((s) => {
        const key = JSON.stringify([s.checkboxes, s.highlightMeta]);
        if (key === last) return;
        last = key;
        removeExtensions();
        removeExtensions = ctx.registerEditorExtension(taskEditorExtensions(s));
      }),
    );
  },
};
