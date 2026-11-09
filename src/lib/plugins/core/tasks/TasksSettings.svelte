<script lang="ts">
  import Setting from "../../../components/settings/Setting.svelte";
  import Toggle from "../../../components/settings/Toggle.svelte";
  import { taskSettings } from "./settings";

  const s = $derived(taskSettings.value);
  function set<K extends "checkboxes" | "highlightMeta">(key: K, v: boolean) {
    taskSettings.value[key] = v;
    taskSettings.save();
  }
</script>

<Setting name="Checkboxes in the editor" description="Shows [ ] and [x] as clickable checkboxes and greys out completed tasks.">
  <Toggle label="Checkboxes in the editor" checked={s.checkboxes} onchange={(v) => set("checkboxes", v)} />
</Setting>
<Setting name="Highlight due date and priority" description="Highlights 📅 2026-12-01, due:…, !high and ⏫ in the editor.">
  <Toggle label="Highlight due date and priority" checked={s.highlightMeta} onchange={(v) => set("highlightMeta", v)} />
</Setting>
