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

<Setting name="Refresh automatically" description="Reloads the repository state regularly so that changes made by other programs show up too.">
  <Toggle label="Refresh automatically" checked={s.autoRefresh} onchange={(v) => set("autoRefresh", v)} />
</Setting>

<Setting name="Interval" description="How often the state is reloaded, in seconds.">
  <input
    type="number"
    min="5"
    max="600"
    class="w-24 rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1 text-sm outline-none focus:border-violet-500"
    value={s.refreshSeconds}
    onchange={(e) => setInterval(Math.max(5, Number(e.currentTarget.value) || 30))}
  />
</Setting>

<Setting name="Stage everything on commit" description="Puts all changes into the commit when nothing is staged. Otherwise only the staged changes go in.">
  <Toggle label="Stage everything on commit" checked={s.stageAllWhenNothingStaged} onchange={(v) => set("stageAllWhenNothingStaged", v)} />
</Setting>

<Setting name="Show untracked files" description="Shows files that git does not know yet in the list of changes.">
  <Toggle label="Show untracked files" checked={s.showUntracked} onchange={(v) => set("showUntracked", v)} />
</Setting>

<Setting name="Push after commit" description="Pushes the branch to the remote after every commit.">
  <Toggle label="Push after commit" checked={s.pushAfterCommit} onchange={(v) => set("pushAfterCommit", v)} />
</Setting>
