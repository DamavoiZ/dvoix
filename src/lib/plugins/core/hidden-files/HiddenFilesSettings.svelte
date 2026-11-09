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

<Setting name="Show hidden files" description="Also toggleable with the command „Show/hide hidden files“.">
  <Toggle label="Show hidden files" checked={s.visible} onchange={(v) => set("visible", v)} />
</Setting>

<Setting name="Include single files" description="Shows files such as .gitignore as well as folders such as .obsidian.">
  <Toggle label="Include single files" checked={s.includeFiles} onchange={(v) => set("includeFiles", v)} />
</Setting>

<Setting name="Show .dvoix folder" description="The folder with this vault's settings and index. Changes inside it only appear after showing it again.">
  <Toggle label="Show .dvoix folder" checked={s.includeAppDir} onchange={(v) => set("includeAppDir", v)} />
</Setting>

<Setting name="Always hide" description="Names, separated by commas, that stay hidden wherever they are. .git contains thousands of internal files and is best kept out.">
  <input
    type="text"
    spellcheck="false"
    placeholder=".git, .trash"
    class="w-48 rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1 font-mono text-sm outline-none focus:border-violet-500"
    value={s.exclude.join(", ")}
    onchange={(e) => setExclude(e.currentTarget.value)}
  />
</Setting>
