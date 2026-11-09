<script lang="ts">
  import { contextMenu } from "./contextMenu.svelte";

  let menu: HTMLDivElement | undefined = $state();
  let pos = $state({ x: 0, y: 0 });

  // Keep the menu inside the window.
  $effect(() => {
    const open = contextMenu.open;
    if (!open || !menu) return;
    const r = menu.getBoundingClientRect();
    pos = {
      x: Math.min(open.x, window.innerWidth - r.width - 4),
      y: Math.min(open.y, window.innerHeight - r.height - 4),
    };
  });
</script>

<svelte:window
  onkeydown={(e) => e.key === "Escape" && contextMenu.close()}
  onblur={() => contextMenu.close()}
  onresize={() => contextMenu.close()}
/>

{#if contextMenu.open}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="fixed inset-0 z-40" onclick={() => contextMenu.close()} oncontextmenu={(e) => { e.preventDefault(); contextMenu.close(); }}></div>
  <div
    bind:this={menu}
    class="fixed z-50 min-w-48 rounded-lg border border-zinc-700 bg-zinc-900 p-1 text-sm shadow-xl"
    style:left="{pos.x || contextMenu.open.x}px"
    style:top="{pos.y || contextMenu.open.y}px"
    role="menu"
  >
    {#each contextMenu.open.items as item, i (i)}
      {#if item === "separator"}
        <div class="my-1 h-px bg-zinc-800"></div>
      {:else}
        <button
          class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left {item.danger
            ? 'text-red-400 hover:bg-red-500/15'
            : 'text-zinc-200 hover:bg-zinc-800'}"
          role="menuitem"
          onclick={() => {
            contextMenu.close();
            item.action();
          }}
        >
          {#if item.icon}<item.icon size={15} strokeWidth={1.75} />{:else}<span class="w-[15px]"></span>{/if}
          {item.label}
        </button>
      {/if}
    {/each}
  </div>
{/if}
