<script lang="ts">
  import Select from "../../components/Select.svelte";
  import Setting from "../../components/settings/Setting.svelte";
  import { cleanFolder } from "../../attachments";
  import { explorerSettings } from "./settings";

  const input = "w-56 rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1 text-sm text-zinc-200 outline-none focus:border-violet-500";
</script>

<Setting
  name="Default location for new notes"
  description="Where the buttons at the top of the explorer create new files. Right-clicking a folder always creates the file there."
>
  <Select
    label="Default location for new notes"
    value={explorerSettings.value.newFileLocation}
    options={[
      { value: "root", label: "Vault root" },
      { value: "current", label: "Same folder as the current file" },
      { value: "folder", label: "In the folder specified below" },
    ]}
    onchange={(v: "current" | "root" | "folder") => {
      explorerSettings.value.newFileLocation = v;
      explorerSettings.save();
    }}
  />
</Setting>
{#if explorerSettings.value.newFileLocation === "folder"}
  <Setting name="Folder to create new notes in" description="Relative to the vault root. It is created when needed.">
    <input
      class={input}
      aria-label="Folder to create new notes in"
      value={explorerSettings.value.newFileFolder}
      placeholder="Inbox"
      onchange={(e) => {
        explorerSettings.value.newFileFolder = cleanFolder(e.currentTarget.value);
        e.currentTarget.value = explorerSettings.value.newFileFolder;
        explorerSettings.save();
      }}
    />
  </Setting>
{/if}
