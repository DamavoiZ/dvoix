<script lang="ts">
  import Setting from "../../../components/settings/Setting.svelte";
  import { explorerSettings } from "./settings";
</script>

<Setting name="Neue Dateien anlegen in" description="Gilt für die Knöpfe oben im Explorer. Per Rechtsklick auf einen Ordner landet die Datei immer dort.">
  <select
    class="rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1 text-sm text-zinc-200 outline-none focus:border-violet-500"
    value={explorerSettings.value.newFileLocation}
    onchange={(e) => {
      explorerSettings.value.newFileLocation = e.currentTarget.value as "current" | "root";
      explorerSettings.save();
    }}
  >
    <option value="current">Ordner der offenen Datei</option>
    <option value="root">Vault-Wurzel</option>
  </select>
</Setting>
