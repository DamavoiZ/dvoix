<script lang="ts">
  import Setting from "../../../components/settings/Setting.svelte";
  import Toggle from "../../../components/settings/Toggle.svelte";
  import { gitSettings } from "./settings";

  const s = $derived(gitSettings.value);
  function set(key: "autoRefresh" | "stageAllWhenNothingStaged" | "showUntracked" | "pushAfterCommit", v: boolean) {
    gitSettings.value[key] = v;
    gitSettings.save();
  }
  function setInterval(seconds: number) {
    gitSettings.value.refreshSeconds = seconds;
    gitSettings.save();
  }
</script>

<Setting name="Automatisch aktualisieren" description="Lädt den Zustand des Repositories regelmäßig neu, damit auch Änderungen von anderen Programmen auffallen.">
  <Toggle label="Automatisch aktualisieren" checked={s.autoRefresh} onchange={(v) => set("autoRefresh", v)} />
</Setting>

<Setting name="Intervall" description="Wie oft der Zustand neu geladen wird, in Sekunden.">
  <input
    type="number"
    min="5"
    max="600"
    class="w-24 rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1 text-sm outline-none focus:border-violet-500"
    value={s.refreshSeconds}
    onchange={(e) => setInterval(Math.max(5, Number(e.currentTarget.value) || 30))}
  />
</Setting>

<Setting name="Beim Commit alles stagen" description="Legt alle Änderungen in den Commit, wenn nichts gestaged ist. Sonst gehen nur die gestageden Änderungen hinein.">
  <Toggle label="Beim Commit alles stagen" checked={s.stageAllWhenNothingStaged} onchange={(v) => set("stageAllWhenNothingStaged", v)} />
</Setting>

<Setting name="Unversionierte Dateien zeigen" description="Zeigt Dateien, die git noch nicht kennt, in der Liste der Änderungen.">
  <Toggle label="Unversionierte Dateien zeigen" checked={s.showUntracked} onchange={(v) => set("showUntracked", v)} />
</Setting>

<Setting name="Nach dem Commit pushen" description="Schickt den Branch nach jedem Commit auf das Remote.">
  <Toggle label="Nach dem Commit pushen" checked={s.pushAfterCommit} onchange={(v) => set("pushAfterCommit", v)} />
</Setting>
