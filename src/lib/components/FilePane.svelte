<script lang="ts">
  import { api, type NoteSummary } from "../api";
  import { fileTypeFor } from "../filetypes";
  import { ui } from "../ui.svelte";
  import { vault } from "../vault.svelte";
  import TabBar from "./TabBar.svelte";

  const path = $derived(vault.openPath);
  const type = $derived(path ? fileTypeFor(path) : undefined);

  let backlinks = $state<NoteSummary[]>([]);
  $effect(() => {
    void vault.revision;
    const p = path;
    if (p && type?.id === "markdown") api.backlinks(p).then((b) => (backlinks = b)).catch(() => (backlinks = []));
    else backlinks = [];
  });
</script>

<main class="flex min-w-0 flex-1 flex-col">
  <TabBar />
  {#if path && type}
    <header class="flex items-center justify-between border-b border-zinc-800 px-4 py-2 text-xs text-zinc-500">
      <span class="truncate">{path}</span>
      <span>{vault.dirty ? "Speichert …" : "Gespeichert"}</span>
    </header>
    <div class="min-h-0 flex-1">
      {#key path}
        <type.view
          {path}
          content={vault.content}
          onchange={(c) => vault.update(c)}
          onopen={(target) => vault.openLink(target)}
        />
      {/key}
    </div>
    {#if backlinks.length}
      <footer class="border-t border-zinc-800 px-4 py-2 text-xs text-zinc-500">
        Verlinkt von:
        {#each backlinks as b (b.path)}
          <button class="ml-2 text-violet-300 hover:underline" onclick={() => vault.openFile(b.path)}>{b.title}</button>
        {/each}
      </footer>
    {/if}
  {:else}
    <div class="flex flex-1 flex-col items-center justify-center gap-3 text-sm text-zinc-500">
      <p>Wähle links eine Datei oder lege eine neue an.</p>
      <button class="rounded-md px-3 py-1.5 text-violet-300 hover:bg-zinc-800" onclick={() => (ui.switcherOpen = true)}>
        Datei suchen <kbd class="ml-1 rounded bg-zinc-800 px-1.5 py-0.5 text-xs text-zinc-400">Strg+O</kbd>
      </button>
    </div>
  {/if}
  {#if vault.error}
    <div class="border-t border-red-900 bg-red-950/50 px-4 py-2 text-xs text-red-300">
      {vault.error}
      <button class="ml-2 underline" onclick={() => (vault.error = null)}>schließen</button>
    </div>
  {/if}
</main>
