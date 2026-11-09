<script lang="ts">
  import { onMount } from "svelte";
  import Ribbon from "./lib/components/Ribbon.svelte";
  import SidePanel from "./lib/components/SidePanel.svelte";
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import FilePane from "./lib/components/FilePane.svelte";
  import Welcome from "./lib/components/Welcome.svelte";
  import SettingsDialog, { SETTINGS } from "./lib/components/settings/SettingsDialog.svelte";
  import { flushConfig } from "./lib/config";
  import { plugins } from "./lib/plugins";
  import { settings } from "./lib/settings.svelte";
  import { ui } from "./lib/ui.svelte";
  import { vault } from "./lib/vault.svelte";

  let ready = $state(false);

  onMount(() => {
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

  /** App-wide shortcuts: plugin commands plus Strg+, for the settings. Captured before the editor sees them. */
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
  <div class="h-full"></div>
{:else if vault.info}
  <div class="flex h-full">
    <Ribbon />
    <SidePanel />
    <FilePane />
  </div>
  <ContextMenu />
  {#each plugins.overlays as overlay (overlay.pluginId + overlay.id)}
    <overlay.component />
  {/each}
  <SettingsDialog />
{:else}
  <Welcome />
{/if}
