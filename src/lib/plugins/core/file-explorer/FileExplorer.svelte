<script lang="ts">
  // File explorer: the folder tree of the vault. Its buttons sit in the panel header (FileExplorerActions).
  import TreeItem from "./TreeItem.svelte";
  import { buildTree, mergeEntries } from "./tree";
  import { contextMenu } from "../../../components/contextMenuState.svelte";
  import { DRAG_TYPE, createItems } from "./fileMenu";
  import { vault } from "../../../vault.svelte";
  import { plugins } from "../../manager.svelte";

  // Other plugins can add entries only the explorer shows (hidden folders).
  const tree = $derived(buildTree(mergeEntries(vault.entries, plugins.explorerEntries)));
  let rootDrop = $state(false);
</script>

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
