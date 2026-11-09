<script lang="ts">
  // The thin icon bar on the far left, like Obsidian's ribbon. Its icons come
  // from plugins: one per side panel, plus plugin actions.
  import { ArrowLeftRight, Settings } from "@lucide/svelte";
  import { plugins } from "../plugins";
  import { ui } from "../ui.svelte";
  import { vault } from "../vault.svelte";
  import { SETTINGS } from "./settings/SettingsDialog.svelte";

  const items = $derived(
    [
      ...plugins.panels.map((p) => ({ key: `panel:${p.id}`, label: p.label, icon: p.icon, order: p.order ?? 100, panel: p.id, action: undefined })),
      ...plugins.ribbonActions.map((a) => ({ key: `action:${a.id}`, label: a.label, icon: a.icon, order: a.order ?? 100, panel: undefined, action: a.action })),
    ].sort((a, b) => a.order - b.order),
  );

  const btn = "flex h-8 w-8 items-center justify-center rounded-md transition-colors";
  const idle = "text-zinc-500 hover:bg-zinc-800/60 hover:text-zinc-200";
</script>

<nav class="flex w-11 shrink-0 flex-col items-center gap-1 border-r border-zinc-800 bg-zinc-950 py-2" aria-label="Ribbon">
  {#each items as item (item.key)}
    {@const active = !!item.panel && ui.panelOpen && ui.panel === item.panel}
    <button
      class="{btn} {active ? 'bg-zinc-800 text-zinc-100' : idle}"
      title={item.label}
      aria-label={item.label}
      aria-pressed={item.panel ? active : undefined}
      onclick={() => (item.panel ? ui.togglePanel(item.panel) : item.action?.())}
    >
      <item.icon size={18} strokeWidth={1.75} />
    </button>
  {/each}

  <div class="flex-1"></div>

  <button class="{btn} {idle}" title="Anderen Vault öffnen" aria-label="Anderen Vault öffnen" onclick={() => vault.close()}>
    <ArrowLeftRight size={18} strokeWidth={1.75} />
  </button>
  <button
    class="{btn} {ui.modal === SETTINGS ? 'bg-zinc-800 text-zinc-100' : idle}"
    title="Einstellungen (Strg+,)"
    aria-label="Einstellungen"
    onclick={() => ui.toggleModal(SETTINGS)}
  >
    <Settings size={18} strokeWidth={1.75} />
  </button>
</nav>
