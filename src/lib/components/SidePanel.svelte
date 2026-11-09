<script lang="ts">
  // The panel next to the ribbon: file explorer, search or tags. Resizable by
  // dragging its right edge, collapsible from the ribbon.
  import FileExplorer from "./FileExplorer.svelte";
  import SearchPanel from "./SearchPanel.svelte";
  import TagsPanel from "./TagsPanel.svelte";
  import { ui } from "../ui.svelte";
  import { vault } from "../vault.svelte";

  const titles = { files: "Dateien", search: "Suche", tags: "Tags" };

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

{#if ui.panelOpen}
  <aside class="relative flex shrink-0 flex-col border-r border-zinc-800 bg-zinc-900/60" style:width="{ui.panelWidth}px">
    {#if ui.panel !== "files"}
      <header class="px-4 pt-3 pb-2 text-xs font-medium tracking-wide text-zinc-500 uppercase">{titles[ui.panel]}</header>
    {/if}

    {#if ui.panel === "files"}
      <FileExplorer />
    {:else if ui.panel === "search"}
      <SearchPanel />
    {:else}
      <TagsPanel />
    {/if}

    <footer class="truncate border-t border-zinc-800 px-4 py-2 text-xs text-zinc-500" title={vault.info?.path}>
      {vault.info?.name}
    </footer>

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="absolute top-0 -right-1 z-10 h-full w-2 cursor-col-resize {resizing ? 'bg-violet-500/40' : 'hover:bg-violet-500/20'}"
      onpointerdown={startResize}
      ondblclick={() => {
        ui.setWidth(280);
        ui.save();
      }}
    ></div>
  </aside>
{/if}
