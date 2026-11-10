<script lang="ts">
  import Setting from "../../../components/settings/Setting.svelte";
  import Toggle from "../../../components/settings/Toggle.svelte";
  import { taskSettings } from "./settings";

  const s = $derived(taskSettings.value);
  function set(v: boolean) {
    taskSettings.value.highlightMeta = v;
    taskSettings.save();
  }
</script>

<Setting name="Highlight due date and priority" description="Highlights 📅 2026-12-01, due:…, !high and ⏫ in the editor.">
  <Toggle label="Highlight due date and priority" checked={s.highlightMeta} onchange={set} />
</Setting>
