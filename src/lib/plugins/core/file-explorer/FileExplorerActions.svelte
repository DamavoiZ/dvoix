<script lang="ts">
  // Buttons of the file explorer (new file, new folder, collapse all). They
  // sit in the panel header so the explorer needs no second toolbar row.
  import { ChevronsDownUp, FolderPlus } from "@lucide/svelte";
  import { newFolder } from "./fileMenu";
  import { creatableFileTypes } from "../../../filetypes";
  import { ui } from "../../../ui.svelte";
  import { vault } from "../../../vault.svelte";
  import { explorerSettings } from "./settings";

  /** New items go into the folder of the open file, like in Obsidian (or the root, see settings). */
  function currentFolder(): string {
    if (explorerSettings.value.newFileLocation === "root") return "";
    const p = vault.openPath;
    return p && p.includes("/") ? p.slice(0, p.lastIndexOf("/")) : "";
  }

  const btn = "flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200";
</script>

{#each creatableFileTypes() as type (type.id)}
  <button class={btn} title="{type.label} erstellen" aria-label="{type.label} erstellen" onclick={() => vault.createFile(type, currentFolder())}>
    <type.icon size={16} strokeWidth={1.75} />
  </button>
{/each}
<button class={btn} title="Ordner erstellen" aria-label="Ordner erstellen" onclick={() => newFolder(currentFolder())}>
  <FolderPlus size={16} strokeWidth={1.75} />
</button>
<button class={btn} title="Alle einklappen" aria-label="Alle einklappen" onclick={() => ui.collapseAll()}>
  <ChevronsDownUp size={16} strokeWidth={1.75} />
</button>
