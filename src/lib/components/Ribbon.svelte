<script lang="ts">
  // The thin icon bar on the far left, like Obsidian's ribbon.
  import { ArrowLeftRight, FolderTree, Hash, Search } from "@lucide/svelte";
  import type { Component } from "svelte";
  import { ui, type PanelId } from "../ui.svelte";
  import { vault } from "../vault.svelte";

  const panels: { id: PanelId; label: string; icon: Component }[] = [
    { id: "files", label: "Dateien", icon: FolderTree },
    { id: "search", label: "Suche", icon: Search },
    { id: "tags", label: "Tags", icon: Hash },
  ];
</script>

<nav class="flex w-11 shrink-0 flex-col items-center gap-1 border-r border-zinc-800 bg-zinc-950 py-2" aria-label="Ribbon">
  {#each panels as p (p.id)}
    {@const active = ui.panelOpen && ui.panel === p.id}
    <button
      class="flex h-8 w-8 items-center justify-center rounded-md transition-colors {active
        ? 'bg-zinc-800 text-zinc-100'
        : 'text-zinc-500 hover:bg-zinc-800/60 hover:text-zinc-200'}"
      title={p.label}
      aria-label={p.label}
      aria-pressed={active}
      onclick={() => ui.togglePanel(p.id)}
    >
      <p.icon size={18} strokeWidth={1.75} />
    </button>
  {/each}

  <div class="flex-1"></div>

  <button
    class="flex h-8 w-8 items-center justify-center rounded-md text-zinc-500 hover:bg-zinc-800/60 hover:text-zinc-200"
    title="Anderen Vault öffnen"
    aria-label="Anderen Vault öffnen"
    onclick={() => vault.close()}
  >
    <ArrowLeftRight size={18} strokeWidth={1.75} />
  </button>
</nav>
