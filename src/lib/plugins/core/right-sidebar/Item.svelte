<script lang="ts">
  // One panel in the right sidebar: a panel of a plugin or a note docked in
  // the sidebar. Its header can be dragged to change the order of the panels.
  import { FileText, X } from "@lucide/svelte";
  import { plugins } from "../../../plugins/manager.svelte";
  import DockedNote from "./DockedNote.svelte";
  import { itemKey, rightSidebarSettings, type DockedItem } from "./settings";

  /** The kind of drag & drop that moves a panel in the sidebar. */
  const PANEL_DRAG = "application/x-dvoix-panel";

  let {
    item,
    dragging,
    ondragstart,
    ondragend,
    ondrop,
  }: {
    item: DockedItem;
    /** The panel that is being dragged, if any. */
    dragging: string | null;
    ondragstart: (key: string) => void;
    ondragend: () => void;
    ondrop: (key: string) => void;
  } = $props();

  /** The panel of a plugin, if this item is one. */
  const panel = $derived(item.kind === "panel" ? plugins.rightPanels.find((p) => p.id === item.id) : undefined);

  /** The name in the header: the label of the panel or the name of the note. */
  const name = $derived(item.kind === "note" ? (item.id.split("/").pop() ?? item.id) : (panel?.label ?? item.id));

  function close() {
    const value = rightSidebarSettings.value;
    value.items = value.items.filter((i) => itemKey(i) !== itemKey(item));
    rightSidebarSettings.save();
  }

  /** The dragged panel takes the place of this one. */
  function drop() {
    const value = rightSidebarSettings.value;
    const from = value.items.findIndex((i) => itemKey(i) === dragging);
    const to = value.items.findIndex((i) => itemKey(i) === itemKey(item));
    if (from < 0 || to < 0 || from === to) return;
    const next = value.items.filter((_, i) => i !== from);
    next.splice(to, 0, value.items[from]);
    value.items = next;
    rightSidebarSettings.save();
  }
</script>

<div class="border-b border-zinc-800 last:border-b-0">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header
    class="flex h-8 items-center gap-1.5 pr-1.5 pl-3 text-xs text-zinc-400 select-none {dragging === itemKey(item)
      ? 'opacity-40'
      : ''}"
    draggable="true"
    ondragstart={(e) => {
      ondragstart(itemKey(item));
      e.dataTransfer?.setData(PANEL_DRAG, itemKey(item));
      if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
    }}
    ondragend={ondragend}
    ondragover={(e) => {
      if (!e.dataTransfer?.types.includes(PANEL_DRAG)) return;
      e.preventDefault();
    }}
    ondrop={(e) => {
      if (!e.dataTransfer?.types.includes(PANEL_DRAG)) return;
      e.preventDefault();
      drop();
      ondragend();
    }}
  >
    {#if panel}
      <panel.icon size={14} strokeWidth={1.75} class="shrink-0 text-zinc-500" />
    {:else}
      <FileText size={14} strokeWidth={1.75} class="shrink-0 text-zinc-500" />
    {/if}
    <span class="truncate" title={item.kind === "note" ? item.id : name}>{name}</span>
    <button
      class="ml-auto flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
      title="Remove from sidebar"
      aria-label="Remove from sidebar"
      onclick={close}
    >
      <X size={13} />
    </button>
  </header>

  {#key itemKey(item)}
    {#if panel}
      <panel.component />
    {:else}
      <DockedNote path={item.id} />
    {/if}
  {/key}
</div>
