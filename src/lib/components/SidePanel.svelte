<script lang="ts">
  // The panel next to the ribbon. Which panels exist depends on the plugins
  // that are on (file explorer, search, tags, ...). Resizable by dragging its
  // right edge, collapsible from the ribbon. Its top row is part of the app's
  // own title bar and moves the window.
  import { PanelLeftClose } from "@lucide/svelte";
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
        <span class="truncate text-xs font-medium tracking-wide text-zinc-500 uppercase">{panel.label}</span>
      {/if}
      {#if panel.actions}
        {#key panel.id}
          <div class="flex min-w-0 items-center gap-0.5 {panel.header !== false ? 'ml-2' : ''}">
            <panel.actions />
          </div>
        {/key}
      {/if}
      <button
        class="ml-auto flex h-7 w-7 items-center justify-center rounded-md text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
        title="Seitenleiste einklappen"
        aria-label="Seitenleiste einklappen"
        onclick={() => ui.togglePanel(panel.id)}
      >
        <PanelLeftClose size={16} strokeWidth={1.75} />
      </button>
    </header>

    {#key panel.id}
      <panel.component />
    {/key}

    <footer class="truncate border-t border-zinc-800 px-4 py-2 text-xs text-zinc-500" title={vault.info?.path}>
      {vault.info?.name}
    </footer>

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="absolute top-0 -right-1 z-10 h-full w-2 cursor-col-resize {resizing ? 'bg-violet-500/40' : 'hover:bg-violet-500/20'}"
      onpointerdown={startResize}
      ondblclick={() => {
        ui.setWidth(PANEL_DEFAULT);
        ui.save();
      }}
    ></div>
  </aside>
{/if}
