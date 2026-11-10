<script lang="ts">
  // Settings page of the attachments feature.
  import Select from "../../components/Select.svelte";
  import Setting from "../../components/settings/Setting.svelte";
  import { cleanFolder } from "../../attachments";
  import { attachmentSettings, type AttachmentLocation } from "./settings";

  const input = "w-56 rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1 text-sm text-zinc-200 outline-none focus:border-violet-500";
</script>

<Setting name="Default location for new attachments" description="Where images and files are saved when you paste or drop them into a note.">
  <Select
    label="Default location for new attachments"
    value={attachmentSettings.value.location}
    options={[
      { value: "root", label: "Vault root" },
      { value: "folder", label: "In the folder specified below" },
      { value: "current", label: "Same folder as the current note" },
      { value: "subfolder", label: "In a subfolder of the current note's folder" },
    ]}
    onchange={(v: AttachmentLocation) => {
      attachmentSettings.value.location = v;
      attachmentSettings.save();
    }}
  />
</Setting>
{#if attachmentSettings.value.location === "folder" || attachmentSettings.value.location === "subfolder"}
  <Setting
    name={attachmentSettings.value.location === "folder" ? "Attachment folder path" : "Subfolder name"}
    description={attachmentSettings.value.location === "folder"
      ? "Pasted and dropped files are saved in this folder, relative to the vault root. It is created when needed."
      : "Pasted and dropped files are saved in this subfolder next to the note."}
  >
    <input
      class={input}
      aria-label="Attachment folder"
      value={attachmentSettings.value.folder}
      placeholder="attachments"
      onchange={(e) => {
        attachmentSettings.value.folder = cleanFolder(e.currentTarget.value);
        e.currentTarget.value = attachmentSettings.value.folder;
        attachmentSettings.save();
      }}
    />
  </Setting>
{/if}
