<script lang="ts">
  // Editor tabs above the open file: click to switch, middle click or × to close,
  // drag to reorder, "+" (Strg+T) for an empty tab.
  import { File, Plus, X } from "@lucide/svelte";
  import { fileTypeFor } from "../filetypes";
  import { contextMenu } from "./contextMenu.svelte";
  import { vault, type Tab } from "../vault.svelte";

  const TAB_DRAG = "application/x-dvoix-tab";

  let dragId = $state<number | null>(null);
  /** Insert position while dragging (index in the tab list). */
  let dropAt = $state<number | null>(null);

  function title(tab: Tab): string {
    if (!tab.path) return "Neuer Tab";
    const name = tab.path.split("/").pop()!;
    return fileTypeFor(tab.path)?.id === "markdown" ? name.replace(/\.md$/i, "") : name;
  }

  function menu(e: MouseEvent, tab: Tab) {
    contextMenu.show(e, [
      { label: "Schließen", icon: X, action: () => void vault.closeTab(tab.id) },
      { label: "Andere Tabs schließen", action: () => void vault.closeOtherTabs(tab.id) },
    ]);
  }

  function dragOver(e: DragEvent, index: number) {
    if (dragId === null || !e.dataTransfer?.types.includes(TAB_DRAG)) return;
    e.preventDefault();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    dropAt = e.clientX < r.left + r.width / 2 ? index : index + 1;
  }

  function drop(e: DragEvent) {
    if (dragId === null || dropAt === null) return;
    e.preventDefault();
    const from = vault.tabs.findIndex((t) => t.id === dragId);
    vault.moveTab(dragId, dropAt > from ? dropAt - 1 : dropAt);
    dragId = dropAt = null;
  }
</script>

<div class="flex h-9 shrink-0 items-end gap-px overflow-x-auto border-b border-zinc-800 bg-zinc-900/60 px-1.5 pt-1.5" role="tablist">
  {#each vault.tabs as tab, i (tab.id)}
    {@const active = tab.id === vault.activeTabId}
    {@const type = tab.path ? fileTypeFor(tab.path) : undefined}
    <div
      class="group relative flex h-[calc(100%+1px)] max-w-52 min-w-24 shrink items-center gap-1.5 -mb-px rounded-t-md border border-b-0 pr-1 pl-2.5 text-[13px] {active
        ? 'border-zinc-800 bg-zinc-950 text-zinc-100'
        : 'border-transparent text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-200'}"
      role="tab"
      tabindex="0"
      aria-selected={active}
      title={tab.path ?? "Neuer Tab"}
      draggable="true"
      ondragstart={(e) => {
        dragId = tab.id;
        e.dataTransfer?.setData(TAB_DRAG, String(tab.id));
        if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
      }}
      ondragend={() => (dragId = dropAt = null)}
      ondragover={(e) => dragOver(e, i)}
      ondrop={drop}
      onmousedown={(e) => {
        if (e.button === 1) e.preventDefault(); // no autoscroll on middle click
      }}
      onauxclick={(e) => e.button === 1 && vault.closeTab(tab.id)}
      onclick={() => vault.activateTab(tab.id)}
      onkeydown={(e) => (e.key === "Enter" || e.key === " ") && vault.activateTab(tab.id)}
      oncontextmenu={(e) => menu(e, tab)}
    >
      {#if dropAt === i}<span class="absolute top-1 bottom-1 -left-px w-0.5 rounded bg-violet-500"></span>{/if}
      {#if dropAt === i + 1 && i === vault.tabs.length - 1}<span class="absolute top-1 -right-px bottom-1 w-0.5 rounded bg-violet-500"></span>{/if}
      {#if type}
        <type.icon size={14} strokeWidth={1.75} class="shrink-0 text-zinc-500" />
      {:else}
        <File size={14} strokeWidth={1.75} class="shrink-0 text-zinc-600" />
      {/if}
      <span class="min-w-0 flex-1 truncate">{title(tab)}</span>
      <button
        class="flex h-5 w-5 shrink-0 items-center justify-center rounded text-zinc-500 hover:bg-zinc-700 hover:text-zinc-100 {active
          ? ''
          : 'opacity-0 group-hover:opacity-100'}"
        title="Schließen (Strg+W)"
        aria-label="Tab schließen"
        onclick={(e) => {
          e.stopPropagation();
          void vault.closeTab(tab.id);
        }}
      >
        <X size={13} />
      </button>
    </div>
  {/each}
  <button
    class="mb-1 ml-1 flex h-6 w-6 shrink-0 items-center justify-center self-center rounded text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
    title="Neuer Tab (Strg+T)"
    aria-label="Neuer Tab"
    onclick={() => vault.newTab()}
  >
    <Plus size={15} />
  </button>
</div>
