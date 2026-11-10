<script lang="ts">
  // The right edge of the window: a narrow band with an icon per panel and the
  // column with the panels next to it. A note can be dragged from the tab bar
  // into the column, and the panels can be dragged to change their order. The
  // button that shows and hides all of it stands in the title row of the
  // window (see `ToggleButton.svelte`).
  import { TAB_DRAG } from "../../../drag";
  import { plugins } from "../../../plugins/manager.svelte";
  import { vault } from "../../../vault.svelte";
  import Item from "./Item.svelte";
  import { clampWidth, itemKey, rightSidebarSettings, WIDTH_DEFAULT } from "./settings";

  /** The panels that can stand in the sidebar. */
  const panels = $derived(plugins.rightPanels);
  /** The panels in the column. A panel whose plugin is off is not shown. */
  const items = $derived(
    rightSidebarSettings.value.items.filter((i) => i.kind !== "panel" || panels.some((p) => p.id === i.id)),
  );
  const collapsed = $derived(rightSidebarSettings.value.collapsed);
  const width = $derived(rightSidebarSettings.value.width);

  /** The panel that is being dragged, if any. */
  let dragging = $state<string | null>(null);
  /** Whether a note is dragged over the sidebar. */
  let over = $state(false);
  let resizing = $state(false);

  /** Whether a panel is in the column. */
  function isOpen(panelId: string): boolean {
    return items.some((i) => i.kind === "panel" && i.id === panelId);
  }

  /** Opens a panel in the column, or closes it when it is open. */
  function toggle(panelId: string) {
    const value = rightSidebarSettings.value;
    if (isOpen(panelId) && !value.collapsed) {
      value.items = value.items.filter((i) => !(i.kind === "panel" && i.id === panelId));
    } else {
      if (!isOpen(panelId)) value.items = [...value.items, { kind: "panel", id: panelId }];
      value.collapsed = false;
    }
    rightSidebarSettings.save();
  }

  /** Puts a note into the column. */
  function dock(path: string) {
    const value = rightSidebarSettings.value;
    if (!value.items.some((i) => i.kind === "note" && i.id === path)) {
      value.items = [...value.items, { kind: "note", id: path }];
    }
    value.collapsed = false;
    rightSidebarSettings.save();
  }

  function onDragOver(event: DragEvent) {
    if (!event.dataTransfer?.types.includes(TAB_DRAG)) return;
    event.preventDefault();
    over = true;
  }

  function onDragLeave(event: DragEvent) {
    const el = event.currentTarget as HTMLElement;
    if (el.contains(event.relatedTarget as Node)) return;
    over = false;
  }

  function onDrop(event: DragEvent) {
    if (!event.dataTransfer?.types.includes(TAB_DRAG)) return;
    event.preventDefault();
    over = false;
    const tab = vault.tabs.find((t) => t.id === Number(event.dataTransfer?.getData(TAB_DRAG)));
    if (tab?.path) dock(tab.path);
  }

  /** The column is resized by dragging its left edge. */
  function startResize(event: PointerEvent) {
    resizing = true;
    const startX = event.clientX;
    const startWidth = width;
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    const move = (e: PointerEvent) => {
      rightSidebarSettings.value.width = clampWidth(startWidth - (e.clientX - startX));
    };
    const up = () => {
      resizing = false;
      rightSidebarSettings.save();
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", up);
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", up);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="flex h-full"
  ondragover={onDragOver}
  ondragleave={onDragLeave}
  ondrop={onDrop}
>
  {#if !collapsed}
    {#if items.length}
      <aside
        class="relative flex shrink-0 flex-col border-l border-zinc-800 bg-zinc-900/60 {over
          ? 'ring-2 ring-violet-500 ring-inset'
          : ''}"
        style:width="{width}px"
      >
        <div class="min-h-0 flex-1 overflow-y-auto">
          {#each items as item (itemKey(item))}
            <Item
              {item}
              {dragging}
              ondragstart={(key) => (dragging = key)}
              ondragend={() => (dragging = null)}
              ondrop={() => (dragging = null)}
            />
          {/each}
        </div>

        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="absolute top-0 -left-1 z-10 h-full w-2 cursor-col-resize {resizing ? 'bg-violet-500/40' : 'hover:bg-violet-500/20'}"
          onpointerdown={startResize}
          ondblclick={() => {
            rightSidebarSettings.value.width = WIDTH_DEFAULT;
            rightSidebarSettings.save();
          }}
        ></div>
      </aside>
    {/if}

    <nav
      class="flex w-11 shrink-0 flex-col items-center gap-1 border-l border-zinc-800 bg-zinc-950 py-2"
      aria-label="Right sidebar"
    >
      {#each panels as panel (panel.id)}
        {@const active = isOpen(panel.id)}
        <button
          class="flex h-8 w-8 items-center justify-center rounded-md transition-colors {active
            ? 'bg-zinc-800 text-zinc-100'
            : 'text-zinc-500 hover:bg-zinc-800/60 hover:text-zinc-200'}"
          title={panel.label}
          aria-label={panel.label}
          aria-pressed={active}
          onclick={() => toggle(panel.id)}
        >
          <panel.icon size={18} strokeWidth={1.75} />
        </button>
      {/each}
    </nav>
  {/if}
</div>
