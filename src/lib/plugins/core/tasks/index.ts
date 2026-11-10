import { ListChecks } from "@lucide/svelte";
import type { Plugin } from "../../types";
import TasksPanel from "./TasksPanel.svelte";
import TasksSettings from "./TasksSettings.svelte";
import { taskMetaExtensions } from "./editor";
import { taskSettings } from "./settings";

export const tasksPlugin: Plugin = {
  manifest: {
    id: "tasks",
    name: "Tasks",
    description: "Markdown checkboxes as tasks: collected in an overview with due date and priority, highlighted in the editor.",
    version: "1.0.0",
  },
  icon: ListChecks,
  async onload(ctx) {
    await taskSettings.load(ctx);
    ctx.addPanel({ id: "tasks", label: "Tasks", icon: ListChecks, component: TasksPanel, order: 40 });
    ctx.addSettingTab({ component: TasksSettings });

    // The highlighting follows the setting: added or removed when it changes.
    // Clickable checkboxes are the core feature `checkboxes`, not part of this plugin.
    let remove: (() => void) | undefined;
    const apply = (on: boolean) => {
      if (on === !!remove) return;
      if (on) remove = ctx.registerEditorExtension(taskMetaExtensions());
      else {
        remove?.();
        remove = undefined;
      }
    };
    apply(taskSettings.value.highlightMeta);
    ctx.register(taskSettings.onChange((s) => apply(s.highlightMeta)));
  },
};
