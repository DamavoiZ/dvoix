<script lang="ts">
  // The open file: headers from plugins (tab bar), the file type's view,
  // footers from plugins (backlinks).
  import { fileTypeFor } from "../filetypes";
  import { plugins } from "../plugins";
  import { vault } from "../vault.svelte";

  const path = $derived(vault.openPath);
  const type = $derived(path ? fileTypeFor(path) : undefined);
  const SWITCHER = "quick-switcher:open";
</script>

<main class="flex min-w-0 flex-1 flex-col">
  {#each plugins.paneHeaders as header (header.pluginId + header.id)}
    <header.component />
  {/each}
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
    {#each plugins.fileFooters as footer (footer.pluginId + footer.id)}
      <footer.component {path} />
    {/each}
  {:else}
    <div class="flex flex-1 flex-col items-center justify-center gap-3 text-sm text-zinc-500">
      <p>Wähle links eine Datei oder lege eine neue an.</p>
      {#if plugins.has(SWITCHER)}
        <button class="rounded-md px-3 py-1.5 text-violet-300 hover:bg-zinc-800" onclick={() => plugins.run(SWITCHER)}>
          Datei suchen <kbd class="ml-1 rounded bg-zinc-800 px-1.5 py-0.5 text-xs text-zinc-400">Strg+O</kbd>
        </button>
      {/if}
    </div>
  {/if}
  {#if vault.error}
    <div class="border-t border-red-900 bg-red-950/50 px-4 py-2 text-xs text-red-300">
      {vault.error}
      <button class="ml-2 underline" onclick={() => (vault.error = null)}>schließen</button>
    </div>
  {/if}
</main>
