<script lang="ts">
  import { onMount } from "svelte";
  import Ribbon from "./lib/components/Ribbon.svelte";
  import SidePanel from "./lib/components/SidePanel.svelte";
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import FilePane from "./lib/components/FilePane.svelte";
  import Welcome from "./lib/components/Welcome.svelte";
  import QuickSwitcher from "./lib/components/QuickSwitcher.svelte";
  import { ui } from "./lib/ui.svelte";
  import { vault } from "./lib/vault.svelte";

  let ready = $state(false);

  onMount(() => {
    vault.init().finally(() => (ready = true));
    // Save pending edits before the window closes.
    const beforeUnload = () => void vault.flush();
    window.addEventListener("beforeunload", beforeUnload);
    return () => window.removeEventListener("beforeunload", beforeUnload);
  });

  /** App-wide shortcuts. Captured before the editor sees them. */
  function shortcuts(e: KeyboardEvent) {
    if (!vault.info || !(e.ctrlKey || e.metaKey) || e.altKey) return;
    const key = e.key.toLowerCase();
    if (key === "o" && !e.shiftKey) ui.switcherOpen = !ui.switcherOpen;
    else if (ui.switcherOpen) return;
    else if (key === "t" && !e.shiftKey) void vault.newTab();
    else if (key === "w" && !e.shiftKey) void vault.closeTab(vault.activeTabId);
    else if (key === "tab") void vault.cycleTab(e.shiftKey ? -1 : 1);
    else if (key === "pagedown") void vault.cycleTab(1);
    else if (key === "pageup") void vault.cycleTab(-1);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
</script>

<svelte:window onkeydowncapture={shortcuts} />

{#if !ready}
  <div class="h-full"></div>
{:else if vault.info}
  <div class="flex h-full">
    <Ribbon />
    <SidePanel />
    <FilePane />
  </div>
  <ContextMenu />
  <QuickSwitcher />
{:else}
  <Welcome />
{/if}
