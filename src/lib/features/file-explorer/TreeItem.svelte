<script lang="ts">
  import { ChevronRight } from "@lucide/svelte";
  import Self from "./TreeItem.svelte";
  import type { TreeNode } from "./tree";
  import { contextMenu } from "../../components/contextMenuState.svelte";
  import { DRAG_TYPE, entryMenu } from "./fileMenu";
  import { vault } from "../../vault.svelte";
  import { ui } from "../../ui.svelte";
  import { fileTypeFor } from "../../filetypes";

  let { node, depth = 0 }: { node: TreeNode; depth?: number } = $props();

  const entry = $derived(node.entry);
  const expanded = $derived(ui.expanded.has(entry.path));
  const active = $derived(vault.openPath === entry.path);
  const renaming = $derived(ui.renaming === entry.path);
  const isNote = $derived(fileTypeFor(entry.path)?.id === "markdown");
  // Like Obsidian: notes without ".md", other files with a small extension badge.
  const label = $derived(isNote ? entry.name.replace(/\.[^.]+$/, "") : entry.name.replace(/\.[^.]+$/, "") || entry.name);
  const badge = $derived(!entry.isDir && !isNote && entry.extension ? entry.extension.toUpperCase() : "");
  /** Hidden entries (shown by the hidden-files plugin) are dimmed, like in a file manager. */
  const hidden = $derived(entry.path.split("/").some((part) => part.startsWith(".")));
  /** Folder a drop on this row moves into. */
  const dropFolder = $derived(entry.isDir ? entry.path : entry.path.split("/").slice(0, -1).join("/"));

  let draft = $state("");
  let dropTarget = $state(false);

  function click(e: MouseEvent) {
    if (entry.isDir) ui.toggleFolder(entry.path);
    else void vault.openFile(entry.path, { newTab: e.ctrlKey || e.metaKey });
  }

  function startEdit(el: HTMLInputElement) {
    draft = entry.name;
    el.value = draft;
    el.focus();
    const dot = entry.isDir ? -1 : draft.lastIndexOf(".");
    el.setSelectionRange(0, dot > 0 ? dot : draft.length);
  }

  async function finishRename() {
    if (ui.renaming !== entry.path) return;
    ui.renaming = null;
    await vault.rename(entry.path, draft);
  }

  function dragOver(e: DragEvent) {
    if (!e.dataTransfer?.types.includes(DRAG_TYPE)) return;
    e.preventDefault();
    e.stopPropagation();
    dropTarget = true;
  }

  function drop(e: DragEvent) {
    dropTarget = false;
    const from = e.dataTransfer?.getData(DRAG_TYPE);
    if (!from) return;
    e.preventDefault();
    e.stopPropagation();
    void vault.move(from, dropFolder);
  }
</script>

<div
  class="group flex h-7 items-center gap-1 rounded-md pr-2 text-[13px] {active
    ? 'bg-zinc-800 text-zinc-50'
    : 'text-zinc-300 hover:bg-zinc-800/60'} {dropTarget && entry.isDir ? 'ring-1 ring-violet-500 ring-inset' : ''}"
  style:padding-left="{0.5 + depth * 1}rem"
  role="treeitem"
  aria-selected={active}
  aria-expanded={entry.isDir ? expanded : undefined}
  tabindex="-1"
  draggable={!renaming}
  ondragstart={(e) => {
    e.dataTransfer?.setData(DRAG_TYPE, entry.path);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }}
  ondragover={dragOver}
  ondragleave={() => (dropTarget = false)}
  ondrop={drop}
  oncontextmenu={(e) => contextMenu.show(e, entryMenu(entry))}
>
  <span class="flex w-4 shrink-0 justify-center text-zinc-500">
    {#if entry.isDir}
      <ChevronRight size={14} class="transition-transform {expanded ? 'rotate-90' : ''}" />
    {/if}
  </span>

  {#if renaming}
    <input
      class="h-6 w-full rounded bg-zinc-950 px-1.5 outline-1 outline-violet-500"
      use:startEdit
      oninput={(e) => (draft = e.currentTarget.value)}
      onblur={finishRename}
      onkeydown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
        if (e.key === "Escape") ui.renaming = null;
      }}
    />
  {:else}
    <button class="flex h-full min-w-0 flex-1 items-center gap-2 text-left" onclick={click} onauxclick={(e) => {
      if (e.button === 1 && !entry.isDir) void vault.openFile(entry.path, { newTab: true });
    }}>
      <span class="truncate" class:font-medium={entry.isDir} class:opacity-60={hidden}>{label}</span>
      {#if badge}
        <span class="shrink-0 rounded bg-zinc-800 px-1 py-px text-[9px] font-semibold tracking-wider text-zinc-400">{badge}</span>
      {/if}
    </button>
  {/if}
</div>

{#if entry.isDir && expanded}
  <div class="relative">
    <!-- indent guide like in Obsidian -->
    <div class="absolute top-0 bottom-0 w-px bg-zinc-800" style:left="{0.5 + depth * 1 + 0.5}rem"></div>
    {#each node.children as child (child.entry.path)}
      <Self node={child} depth={depth + 1} />
    {/each}
  </div>
{/if}
