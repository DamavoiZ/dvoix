<script lang="ts">
  // Minimize, maximize and close for the app's own title bar. Only shown when
  // the window has no native frame (Windows and Linux).
  import { Copy, Minus, Square, X } from "@lucide/svelte";
  import { appWindow } from "../window.svelte";

  const btn = "flex h-full w-11 items-center justify-center text-zinc-400 transition-colors";
</script>

{#if appWindow.custom}
  <div class="flex h-full shrink-0">
    <button class="{btn} hover:bg-zinc-800 hover:text-zinc-100" title="Minimize" aria-label="Minimize" onclick={appWindow.minimize}>
      <Minus size={15} strokeWidth={1.5} />
    </button>
    <button
      class="{btn} hover:bg-zinc-800 hover:text-zinc-100"
      title={appWindow.maximized ? "Restore" : "Maximize"}
      aria-label={appWindow.maximized ? "Restore" : "Maximize"}
      onclick={appWindow.toggleMaximize}
    >
      {#if appWindow.maximized}
        <Copy size={12} strokeWidth={1.5} class="-scale-x-100" />
      {:else}
        <Square size={12} strokeWidth={1.5} />
      {/if}
    </button>
    <button class="{btn} hover:bg-red-600 hover:text-white" title="Close" aria-label="Close" onclick={appWindow.close}>
      <X size={16} strokeWidth={1.5} />
    </button>
  </div>
{/if}
