<script lang="ts">
  // Buttons of the file explorer (new file, new folder, collapse all). They
  // sit in the panel header so the explorer needs no second toolbar row.
  import { ChevronsDownUp, FolderPlus } from "@lucide/svelte";
  import { newFolder } from "./fileMenu";
  import { creatableFileTypes } from "../../filetypes";
  import { ui } from "../../ui.svelte";
  import { vault } from "../../vault.svelte";
  import { explorerSettings } from "./settings";
  import { cleanFolder, folderOf } from "../../attachments";

  /** New items go into the folder of the open file, like in Obsidian (or the root or a fixed folder, see settings). */
  function currentFolder(): string {
    const { newFileLocation, newFileFolder } = explorerSettings.value;
    if (newFileLocation === "root") return "";
    if (newFileLocation === "folder") return cleanFolder(newFileFolder);
    return folderOf(vault.openPath);
  }

  const btn = "flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200";
</script>

{#each creatableFileTypes() as type (type.id)}
  <button class={btn} title="Create {type.label}" aria-label="Create {type.label}" onclick={() => vault.createFile(type, currentFolder())}>
    <type.icon size={16} strokeWidth={1.75} />
  </button>
{/each}
<button class={btn} title="Create folder" aria-label="Create folder" onclick={() => newFolder(currentFolder())}>
  <FolderPlus size={16} strokeWidth={1.75} />
</button>
<button class={btn} title="Collapse all" aria-label="Collapse all" onclick={() => ui.collapseAll()}>
  <ChevronsDownUp size={16} strokeWidth={1.75} />
</button>
