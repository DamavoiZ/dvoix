// Markdown checkboxes (`- [ ]`, `- [x]`) as clickable boxes in the editor.
// A basic editor function, so a core feature; the task overview with due
// dates and priorities stays the optional tasks plugin.
import { SquareCheck } from "@lucide/svelte";
import type { Plugin } from "../../plugins/types";
import CheckboxSettings from "./CheckboxSettings.svelte";
import { checkboxExtensions } from "./editor";
import { checkboxSettings } from "./settings";

export const checkboxesFeature: Plugin = {
  manifest: {
    id: "checkboxes",
    name: "Checkboxes",
    description: "Shows [ ] and [x] in the editor as clickable checkboxes.",
    version: "1.0.0",
  },
  icon: SquareCheck,
  async onload(ctx) {
    await checkboxSettings.load(ctx);
    ctx.addSettingTab({ component: CheckboxSettings });

    // The extensions follow the setting: added or removed when it changes.
    let remove: (() => void) | undefined;
    const apply = (on: boolean) => {
      if (on === !!remove) return;
      if (on) remove = ctx.registerEditorExtension(checkboxExtensions());
      else {
        remove?.();
        remove = undefined;
      }
    };
    apply(checkboxSettings.value.clickable);
    ctx.register(checkboxSettings.onChange((s) => apply(s.clickable)));
  },
};
