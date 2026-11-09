<script lang="ts">
  import Select from "../../../components/Select.svelte";
  import Setting from "../../../components/settings/Setting.svelte";
  import { explorerSettings } from "./settings";
</script>

<Setting name="Neue Dateien anlegen in" description="Gilt für die Knöpfe oben im Explorer. Per Rechtsklick auf einen Ordner landet die Datei immer dort.">
  <Select
    label="Neue Dateien anlegen in"
    value={explorerSettings.value.newFileLocation}
    options={[
      { value: "current", label: "Ordner der offenen Datei" },
      { value: "root", label: "Vault-Wurzel" },
    ]}
    onchange={(v: "current" | "root") => {
      explorerSettings.value.newFileLocation = v;
      explorerSettings.save();
    }}
  />
</Setting>
