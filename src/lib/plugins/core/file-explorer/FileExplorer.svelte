<script lang="ts">
  // File explorer: toolbar plus the folder tree of the vault.
  import { ChevronsDownUp, FolderPlus } from "@lucide/svelte";
  import TreeItem from "./TreeItem.svelte";
  import { buildTree } from "./tree";
  import { contextMenu } from "../../../components/contextMenuState.svelte";
  import { DRAG_TYPE, createItems, newFolder } from "./fileMenu";
  import { creatableFileTypes } from "../../../filetypes";
  import { ui } from "../../../ui.svelte";
  import { vault } from "../../../vault.svelte";
  import { explorerSettings } from "./settings";

  const tree = $derived(buildTree(vault.entries));
  let rootDrop = $state(false);

  /** New items go into the folder of the open file, like in Obsidian (or the root, see settings). */
  function currentFolder(): string {
    if (explorerSettings.value.newFileLocation === "root") return "";
    const p = vault.openPath;
    return p && p.includes("/") ? p.slice(0, p.lastIndexOf("/")) : "";
  }

  const btn = "flex h-7 w-7 items-center justify-center rounded-md text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200";
</script>

<div class="flex items-center justify-center gap-0.5 border-b border-zinc-800/70 px-2 py-1.5">
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
</div>

<div
  class="flex-1 overflow-y-auto px-1.5 py-1.5 {rootDrop ? 'bg-violet-500/5' : ''}"
  role="tree"
  tabindex="-1"
  oncontextmenu={(e) => contextMenu.show(e, createItems(""))}
  ondragover={(e) => {
    if (!e.dataTransfer?.types.includes(DRAG_TYPE)) return;
    e.preventDefault();
    rootDrop = true;
  }}
  ondragleave={() => (rootDrop = false)}
  ondrop={(e) => {
    rootDrop = false;
    const from = e.dataTransfer?.getData(DRAG_TYPE);
    if (from) void vault.move(from, "");
  }}
>
  {#each tree as node (node.entry.path)}
    <TreeItem {node} />
  {:else}
    <p class="px-3 py-2 text-sm text-zinc-500">Der Vault ist noch leer. Rechtsklick oder die Knöpfe oben legen etwas an.</p>
  {/each}
</div>
