<script lang="ts">
  // File explorer: the folder tree of the vault. Its buttons sit in the panel header (FileExplorerActions).
  import TreeItem from "./TreeItem.svelte";
  import { buildTree, mergeEntries } from "./tree";
  import { contextMenu } from "../../components/contextMenuState.svelte";
  import { DRAG_TYPE, createItems } from "./fileMenu";
  import { vault } from "../../vault.svelte";
  import { plugins } from "../../plugins/manager.svelte";

  // Other plugins can add entries only the explorer shows (hidden folders).
  // The tree follows the list of the vault, which is only replaced when the
  // shape of the vault changes: a file that was only written stays as it is.
  const tree = $derived.by(() => {
    void vault.changes.structure.number;
    return buildTree(mergeEntries(vault.entries, plugins.explorerEntries));
  });
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
    <p class="px-3 py-4 text-xs leading-relaxed text-zinc-500">
      The vault is still empty. Right-click or use the buttons above to create something.
    </p>
  {/each}
</div>
