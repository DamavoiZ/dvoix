// Settings of the checkboxes feature, in `.dvoix/plugins/checkboxes/data.json`.
import { pluginSettings } from "../../plugins/pluginSettings.svelte";

export const checkboxSettings = pluginSettings({
  /** `[ ]` and `[x]` in the editor as clickable checkboxes. Off shows the plain Markdown. */
  clickable: true,
});
