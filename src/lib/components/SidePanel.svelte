<script lang="ts">
  // The panel next to the ribbon. Which panels exist depends on the plugins
  // that are on (file explorer, search, tags, ...). Resizable by dragging its
  // right edge, collapsible from the ribbon. Its top row is part of the app's
  // own title bar and moves the window.
  import { Folder, PanelLeftClose } from "@lucide/svelte";
  import { plugins } from "../plugins";
  import { PANEL_DEFAULT, ui } from "../ui.svelte";
  import { vault } from "../vault.svelte";
  import { dragRegion } from "../window.svelte";

  /** The chosen panel, or the first one if its plugin was turned off. */
  const panel = $derived(plugins.panels.find((p) => p.id === ui.panel) ?? plugins.panels[0]);

  let resizing = $state(false);

  function startResize(e: PointerEvent) {
    resizing = true;
    const startX = e.clientX;
    const startWidth = ui.panelWidth;
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => ui.setWidth(startWidth + ev.clientX - startX);
    const up = () => {
      resizing = false;
      ui.save();
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", up);
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", up);
  }
</script>

{#if ui.panelOpen && panel}
  <aside
    class="relative flex shrink-0 flex-col border-r border-zinc-800 bg-zinc-900/60"
    style:width="{ui.panelWidth}px"
  >
    <header class="flex h-9 shrink-0 items-center border-b border-zinc-800 pr-1.5 {panel.header === false && panel.actions ? 'pl-1.5' : 'pl-4'}" use:dragRegion>
      {#if panel.header !== false}
        <span class="truncate text-xs font-medium tracking-wide text-zinc-400 uppercase">{panel.label}</span>
      {/if}
      {#if panel.actions}
        {#key panel.id}
          <div class="flex min-w-0 items-center gap-0.5 {panel.header !== false ? 'ml-2' : ''}">
            <panel.actions />
          </div>
        {/key}
      {/if}
      <button
        class="ml-auto flex h-7 w-7 items-center justify-center rounded-md text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-200"
        title="Collapse sidebar"
        aria-label="Collapse sidebar"
        onclick={() => ui.togglePanel(panel.id)}
      >
        <PanelLeftClose size={16} strokeWidth={1.75} />
      </button>
    </header>

    {#key panel.id}
      <panel.component />
    {/key}

    <footer class="flex h-8 shrink-0 items-center gap-2 border-t border-zinc-800 bg-zinc-950/30 px-3 text-xs text-zinc-400" title={vault.info?.path}>
      <Folder size={13} strokeWidth={1.75} class="shrink-0 text-zinc-500" />
      <span class="truncate">{vault.info?.name}</span>
    </footer>

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="group/resize absolute top-0 -right-1 z-10 flex h-full w-2 cursor-col-resize justify-center"
      onpointerdown={startResize}
      ondblclick={() => {
        ui.setWidth(PANEL_DEFAULT);
        ui.save();
      }}
    >
      <div class="h-full w-px transition-colors {resizing ? 'bg-violet-500' : 'bg-transparent group-hover/resize:bg-violet-500/50'}"></div>
    </div>
  </aside>
{/if}
