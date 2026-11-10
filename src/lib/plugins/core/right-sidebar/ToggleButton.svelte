<script lang="ts">
  // The button in the title row of the window that shows and hides the right
  // sidebar. It stands next to the window buttons, like the arrow in Obsidian.
  import { PanelRightClose, PanelRightOpen } from "@lucide/svelte";
  import { rightSidebarSettings, toggleColumn } from "./settings";

  const collapsed = $derived(rightSidebarSettings.value.collapsed);
  const label = $derived(collapsed ? "Show sidebar" : "Hide sidebar");
</script>

<button
  class="flex h-full w-9 shrink-0 items-center justify-center text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
  title={label}
  aria-label={label}
  onclick={toggleColumn}
>
  {#if collapsed}
    <PanelRightOpen size={16} strokeWidth={1.5} />
  {:else}
    <PanelRightClose size={16} strokeWidth={1.5} />
  {/if}
</button>
