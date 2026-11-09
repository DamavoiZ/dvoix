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

<Setting name="Checkboxen im Editor" description="Zeigt [ ] und [x] als anklickbare Checkboxen und graut erledigte Aufgaben aus.">
  <Toggle label="Checkboxen im Editor" checked={s.checkboxes} onchange={(v) => set("checkboxes", v)} />
</Setting>
<Setting name="Fälligkeit und Priorität hervorheben" description="Markiert 📅 2026-12-01, due:…, !hoch und ⏫ im Editor.">
  <Toggle label="Fälligkeit und Priorität hervorheben" checked={s.highlightMeta} onchange={(v) => set("highlightMeta", v)} />
</Setting>
