<script lang="ts">
  import Select from "../../../components/Select.svelte";
  import Setting from "../../../components/settings/Setting.svelte";
  import { explorerSettings } from "./settings";
</script>

<Setting name="Create new files in" description="Applies to the buttons at the top of the explorer. Right-clicking a folder always creates the file there.">
  <Select
    label="Create new files in"
    value={explorerSettings.value.newFileLocation}
    options={[
      { value: "current", label: "Folder of the open file" },
      { value: "root", label: "Vault root" },
    ]}
    onchange={(v: "current" | "root") => {
      explorerSettings.value.newFileLocation = v;
      explorerSettings.save();
    }}
  />
</Setting>
