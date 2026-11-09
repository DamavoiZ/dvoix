<script lang="ts">
  import TreeItem from "./TreeItem.svelte";
  import { buildTree } from "./tree";
  import { vault } from "../vault.svelte";

  const tree = $derived(buildTree(vault.entries));
</script>

<div class="flex items-center gap-1 px-3 pb-2 text-xs text-zinc-400">
  {#each vault.creatable() as type (type.id)}
    <button class="rounded px-2 py-1 hover:bg-zinc-800 hover:text-zinc-100" onclick={() => vault.createFile(type)}>
      + {type.label}
    </button>
  {/each}
  <button class="rounded px-2 py-1 hover:bg-zinc-800 hover:text-zinc-100" onclick={() => vault.createFolder()}>+ Ordner</button>
</div>

<nav class="flex-1 overflow-y-auto px-1 pb-4">
  {#each tree as node (node.entry.path)}
    <TreeItem {node} />
  {:else}
    <p class="px-3 py-2 text-sm text-zinc-500">Der Vault ist noch leer.</p>
  {/each}
</nav>
