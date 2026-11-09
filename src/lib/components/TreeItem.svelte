<script lang="ts">
  import Self from "./TreeItem.svelte";
  import type { TreeNode } from "./tree";
  import { vault } from "../vault.svelte";
  import { fileTypeFor } from "../filetypes";

  let { node, depth = 0 }: { node: TreeNode; depth?: number } = $props();

  let expanded = $state(true);
  let renaming = $state(false);
  let draft = $state("");

  const entry = $derived(node.entry);
  const active = $derived(vault.openPath === entry.path);
  // Hide the extension of files we have a view for, like Obsidian does for notes.
  const label = $derived(
    entry.isDir || fileTypeFor(entry.path)?.id === "fallback"
      ? entry.name
      : entry.name.replace(/\.[^.]+$/, ""),
  );

  function click() {
    if (entry.isDir) expanded = !expanded;
    else void vault.openFile(entry.path);
  }

  function startRename() {
    draft = entry.name;
    renaming = true;
  }

  async function finishRename() {
    renaming = false;
    await vault.rename(entry.path, draft.trim());
  }

  function focusSelect(el: HTMLInputElement) {
    el.focus();
    const dot = entry.isDir ? -1 : draft.lastIndexOf(".");
    el.setSelectionRange(0, dot > 0 ? dot : draft.length);
  }
</script>

<div
  class="group flex items-center gap-1.5 rounded px-2 py-0.5 text-sm hover:bg-zinc-800/70"
  class:bg-zinc-800={active}
  class:text-white={active}
  style:padding-left="{0.5 + depth * 0.85}rem"
>
  {#if renaming}
    <input
      class="w-full rounded bg-zinc-900 px-1 text-sm outline outline-violet-500"
      bind:value={draft}
      use:focusSelect
      onblur={finishRename}
      onkeydown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
        if (e.key === "Escape") renaming = false;
      }}
    />
  {:else}
    <button class="flex min-w-0 flex-1 items-center gap-1.5 text-left" onclick={click} ondblclick={startRename}>
      <span class="w-3 shrink-0 text-center text-xs text-zinc-500">
        {entry.isDir ? (expanded ? "▾" : "▸") : fileTypeFor(entry.path)?.icon}
      </span>
      <span class="truncate">{label}</span>
    </button>
    <span class="hidden shrink-0 gap-1 text-xs text-zinc-500 group-hover:flex">
      {#if entry.isDir}
        {#each vault.creatable() as type (type.id)}
          <button class="hover:text-zinc-200" title="{type.label} hier anlegen" onclick={() => vault.createFile(type, entry.path)}>
            +{type.icon}
          </button>
        {/each}
      {/if}
      <button class="hover:text-zinc-200" title="Umbenennen" onclick={startRename}>✎</button>
      <button class="hover:text-red-400" title="In den Papierkorb (.trash)" onclick={() => vault.trash(entry.path)}>🗑</button>
    </span>
  {/if}
</div>

{#if entry.isDir && expanded}
  {#each node.children as child (child.entry.path)}
    <Self node={child} depth={depth + 1} />
  {/each}
{/if}
