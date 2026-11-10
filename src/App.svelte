<script lang="ts">
  import { onMount } from "svelte";
  import Ribbon from "./lib/components/Ribbon.svelte";
  import SidePanel from "./lib/components/SidePanel.svelte";
  import RightSidebar from "./lib/components/RightSidebar.svelte";
  import RightSidebarToggle from "./lib/components/RightSidebarToggle.svelte";
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import FilePane from "./lib/components/FilePane.svelte";
  import Welcome from "./lib/components/Welcome.svelte";
  import WindowControls from "./lib/components/WindowControls.svelte";
  import SettingsDialog, { SETTINGS } from "./lib/components/settings/SettingsDialog.svelte";
  import { flushConfig } from "./lib/config";
  import { plugins } from "./lib/plugins";
  import { settings } from "./lib/settings.svelte";
  import { ui } from "./lib/ui.svelte";
  import { vault } from "./lib/vault.svelte";
  import { appWindow, dragRegion } from "./lib/window.svelte";

  let ready = $state(false);

  onMount(() => {
    void appWindow.init();
    vault.init().finally(() => (ready = true));
    // Save pending edits and settings before the window closes.
    const beforeUnload = () => {
      void vault.flush();
      void flushConfig();
    };
    window.addEventListener("beforeunload", beforeUnload);
    return () => window.removeEventListener("beforeunload", beforeUnload);
  });

  // Appearance settings as CSS variables (used by the editor theme and app.css).
  $effect(() => {
    const root = document.documentElement.style;
    root.setProperty("--editor-font-size", `${settings.appearance.fontSize}px`);
    root.fontSize = `${settings.appearance.interfaceFontSize}px`;
    root.setProperty("--editor-max-width", settings.app.readableLineLength ? "48rem" : "none");
  });

  /** App-wide shortcuts: plugin commands plus Ctrl+, for the settings. Captured before the editor sees them. */
  function shortcuts(e: KeyboardEvent) {
    if (!vault.info) return;
    const mod = e.ctrlKey || e.metaKey;
    if (mod && !e.shiftKey && !e.altKey && e.key === ",") {
      ui.toggleModal(SETTINGS);
    } else {
      const cmd = plugins.commandFor(e, ui.modal !== null);
      if (!cmd) return;
      void cmd.run();
    }
    e.preventDefault();
    e.stopPropagation();
  }
</script>

<svelte:window onkeydowncapture={shortcuts} />

{#if !ready}
  <div class="h-full" use:dragRegion></div>
{:else if vault.info}
  <div class="flex h-full">
    <Ribbon />
    <SidePanel />
    <FilePane />
    <!-- The window buttons stay in the top right corner of the window, even
         when a sidebar stands on the right edge below them. -->
    <div class="flex shrink-0 flex-col">
      <div class="flex h-9 shrink-0 items-center justify-end border-b border-zinc-800 bg-zinc-900/60" use:dragRegion>
        <RightSidebarToggle />
        <WindowControls />
      </div>
      <div class="flex min-h-0 flex-1">
        <RightSidebar />
      </div>
    </div>
  </div>
  <ContextMenu />
  {#each plugins.overlays as overlay (overlay.pluginId + overlay.id)}
    <overlay.component />
  {/each}
  <SettingsDialog />
{:else}
  <div class="flex h-full flex-col">
    <div class="flex h-9 shrink-0 justify-end" use:dragRegion><WindowControls /></div>
    <div class="min-h-0 flex-1"><Welcome /></div>
  </div>
{/if}
