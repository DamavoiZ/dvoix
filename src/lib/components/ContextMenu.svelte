<script lang="ts">
  import { contextMenu } from "./contextMenuState.svelte";

  let menu: HTMLDivElement | undefined = $state();
  let pos = $state<{ x: number; y: number } | null>(null);
  let active = $state<number>(-1);

  const actionIndices = $derived(
    contextMenu.open
      ? contextMenu.open.items.flatMap((item, i) => (item === "separator" ? [] : [i]))
      : [],
  );

  // Keep the menu inside the window and focus it for keyboard navigation.
  $effect(() => {
    const open = contextMenu.open;
    if (!open || !menu) {
      pos = null;
      active = -1;
      return;
    }
    const r = menu.getBoundingClientRect();
    pos = {
      x: Math.max(4, Math.min(open.x, window.innerWidth - r.width - 4)),
      y: Math.max(4, Math.min(open.y, window.innerHeight - r.height - 4)),
    };
    active = -1;
    menu.focus();
  });

  function keydown(e: KeyboardEvent) {
    if (!contextMenu.open) return;
    if (e.key === "Escape") {
      e.preventDefault();
      contextMenu.close();
      return;
    }
    if (!actionIndices.length) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Home" || e.key === "End") {
      e.preventDefault();
      const curPos = actionIndices.indexOf(active);
      if (e.key === "ArrowDown") {
        active = actionIndices[(curPos + 1) % actionIndices.length];
      } else if (e.key === "ArrowUp") {
        active = actionIndices[(curPos - 1 + actionIndices.length) % actionIndices.length];
      } else if (e.key === "Home") {
        active = actionIndices[0];
      } else {
        active = actionIndices[actionIndices.length - 1];
      }
    } else if ((e.key === "Enter" || e.key === " ") && active >= 0) {
      e.preventDefault();
      const item = contextMenu.open.items[active];
      if (item && item !== "separator") {
        contextMenu.close();
        item.action();
      }
    }
  }
</script>

<svelte:window
  onkeydown={keydown}
  onblur={() => contextMenu.close()}
  onresize={() => contextMenu.close()}
/>

{#if contextMenu.open}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="fixed inset-0 z-40" onclick={() => contextMenu.close()} oncontextmenu={(e) => { e.preventDefault(); contextMenu.close(); }}></div>
  <div
    bind:this={menu}
    class="fixed z-50 min-w-48 rounded-lg border border-zinc-700 bg-zinc-900 p-1 text-sm shadow-xl outline-none {pos ? 'opacity-100' : 'opacity-0'}"
    style:left="{(pos ?? contextMenu.open).x}px"
    style:top="{(pos ?? contextMenu.open).y}px"
    role="menu"
    tabindex="-1"
  >
    {#each contextMenu.open.items as item, i (i)}
      {#if item === "separator"}
        <div class="my-1 h-px bg-zinc-800"></div>
      {:else}
        <button
          class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors {item.danger
            ? i === active ? 'bg-red-500/15 text-red-400' : 'text-red-400 hover:bg-red-500/15'
            : i === active ? 'bg-zinc-800 text-zinc-100' : 'text-zinc-200 hover:bg-zinc-800'}"
          role="menuitem"
          onmouseenter={() => (active = i)}
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
