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

  // Appearance settings as CSS variables and root attributes (used by Tailwind tokens, the editor theme, and app.css).
  $effect(() => {
    const el = document.documentElement;
    const root = el.style;
    const a = settings.appearance;
    el.dataset.theme = a.theme;
    el.dataset.accent = a.accentColor;
    el.classList.toggle("dark", a.theme !== "light");
    const appFont = a.interfaceFont === "system" ? "var(--font-system)" : a.interfaceFont === "mono" ? "var(--font-mono)" : "var(--font-sans)";
    const edFont = a.editorFont === "serif" ? "var(--font-serif)" : a.editorFont === "mono" ? "var(--font-mono)" : "var(--font-sans)";
    root.setProperty("--app-font-sans", appFont);
    root.setProperty("--editor-font-family", edFont);
    root.setProperty("--editor-font-size", `${a.fontSize}px`);
    root.fontSize = `${a.interfaceFontSize}px`;
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
  <div class="flex h-full flex-col">
    <div class="flex min-h-0 flex-1">
      <Ribbon />
      <SidePanel />
      <!-- The top row of the main area and the right sidebar is the title bar:
           the tab bar takes the space on the left, the sidebar toggle and the
           window buttons stay in the top right corner above the sidebar. -->
      <div class="flex min-w-0 flex-1 flex-col">
        <div class="flex shrink-0 items-start" use:dragRegion>
          <div class="flex min-w-0 flex-1 flex-col">
            {#each plugins.paneHeaders as header (header.pluginId + header.id)}
              <header.component />
            {:else}
              <div class="h-9 border-b border-zinc-800 bg-zinc-900/60"></div>
            {/each}
          </div>
          <div class="flex h-9 shrink-0 items-center border-b border-zinc-800 bg-zinc-900/60">
            <RightSidebarToggle />
            <WindowControls />
          </div>
        </div>
        <div class="flex min-h-0 flex-1">
          <FilePane />
          <RightSidebar />
        </div>
      </div>
    </div>
    {#if plugins.statusBarLeft.length || plugins.statusBarRight.length}
      <footer class="flex h-6 shrink-0 items-center justify-between gap-4 border-t border-zinc-800 bg-zinc-900/60 px-3 text-[11px] text-zinc-500">
        <div class="flex min-w-0 items-center gap-3">
          {#each plugins.statusBarLeft as item (item.pluginId + item.id)}
            <item.component />
          {/each}
        </div>
        <div class="flex shrink-0 items-center gap-3">
          {#each plugins.statusBarRight as item (item.pluginId + item.id)}
            <item.component />
          {/each}
        </div>
      </footer>
    {/if}
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
