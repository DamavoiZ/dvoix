<script lang="ts">
  // Settings page of the hidden-files plugin.
  import Setting from "../../../components/settings/Setting.svelte";
  import Toggle from "../../../components/settings/Toggle.svelte";
  import { hiddenSettings } from "./settings";

  const s = $derived(hiddenSettings.value);
  function set(key: "visible" | "includeFiles" | "includeAppDir", v: boolean) {
    hiddenSettings.value[key] = v;
    hiddenSettings.save();
  }
  function setExclude(text: string) {
    hiddenSettings.value.exclude = text
      .split(",")
      .map((x) => x.trim())
      .filter(Boolean);
    hiddenSettings.save();
  }
</script>

<Setting name="Versteckte Dateien anzeigen" description="Auch über den Befehl „Versteckte Dateien ein-/ausblenden“ umschaltbar.">
  <Toggle label="Versteckte Dateien anzeigen" checked={s.visible} onchange={(v) => set("visible", v)} />
</Setting>

<Setting name="Auch einzelne Dateien" description="Zeigt neben Ordnern wie .obsidian auch Dateien wie .gitignore.">
  <Toggle label="Auch einzelne Dateien" checked={s.includeFiles} onchange={(v) => set("includeFiles", v)} />
</Setting>

<Setting name="Ordner .dvoix zeigen" description="Der Ordner mit den Einstellungen und dem Index dieses Vaults. Änderungen darin erscheinen erst nach erneutem Einblenden.">
  <Toggle label="Ordner .dvoix zeigen" checked={s.includeAppDir} onchange={(v) => set("includeAppDir", v)} />
</Setting>

<Setting name="Trotzdem ausblenden" description="Namen, durch Komma getrennt, die verborgen bleiben, egal wo sie liegen. .git enthält Tausende interne Dateien und bleibt am besten draußen.">
  <input
    type="text"
    spellcheck="false"
    placeholder=".git, .trash"
    class="w-48 rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1 font-mono text-sm outline-none focus:border-violet-500"
    value={s.exclude.join(", ")}
    onchange={(e) => setExclude(e.currentTarget.value)}
  />
</Setting>
