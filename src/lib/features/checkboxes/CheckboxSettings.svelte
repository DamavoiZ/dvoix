<script lang="ts">
  // Settings page of the checkboxes feature.
  import Setting from "../../components/settings/Setting.svelte";
  import Toggle from "../../components/settings/Toggle.svelte";
  import { checkboxSettings } from "./settings";
</script>

<Setting name="Clickable checkboxes" description="Shows [ ] and [x] in the editor as checkboxes you can click and greys out checked items.">
  <Toggle
    label="Clickable checkboxes"
    checked={checkboxSettings.value.clickable}
    onchange={(v) => {
      checkboxSettings.value.clickable = v;
      checkboxSettings.save();
    }}
  />
</Setting>
