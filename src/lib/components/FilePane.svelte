<script lang="ts">
  // The open file: headers from features and plugins (tab bar), the file type's view,
  // footers from features and plugins (backlinks). A tab can also show a plugin view instead.
  // The top row doubles as the title bar: empty space there moves the window, the
  // window buttons sit at the right of the window (see `App.svelte`).
  import { fileTypeFor } from "../filetypes";
  import { plugins } from "../plugins";
  import { vault } from "../vault.svelte";
  import { dragRegion } from "../window.svelte";

  const path = $derived(vault.openPath);
  const type = $derived(path ? fileTypeFor(path) : undefined);
  /** A plugin view (e.g. the graph) shown in the active tab instead of a file. */
  const view = $derived(vault.openView ? plugins.view(vault.openView) : undefined);
  const SWITCHER = "quick-switcher:open";
</script>

<main class="flex min-w-0 flex-1 flex-col">
  <div class="flex shrink-0 items-start" use:dragRegion>
    <div class="flex min-w-0 flex-1 flex-col">
      {#each plugins.paneHeaders as header (header.pluginId + header.id)}
        <header.component />
      {:else}
        <div class="h-9 border-b border-zinc-800 bg-zinc-900/60"></div>
      {/each}
    </div>
  </div>
  {#if view}
    <div class="min-h-0 flex-1">
      {#key view.id}
        <view.component />
      {/key}
    </div>
  {:else if path && type}
    <header class="flex items-center justify-between border-b border-zinc-800 px-4 py-2 text-xs text-zinc-500">
      <span class="truncate">{path}</span>
      <span>{vault.dirty ? "Saving …" : "Saved"}</span>
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
      <p>Pick a file on the left or create a new one.</p>
      {#if plugins.has(SWITCHER)}
        <button class="rounded-md px-3 py-1.5 text-violet-300 hover:bg-zinc-800" onclick={() => plugins.run(SWITCHER)}>
          Find a file <kbd class="ml-1 rounded bg-zinc-800 px-1.5 py-0.5 text-xs text-zinc-400">Ctrl+O</kbd>
        </button>
      {/if}
    </div>
  {/if}
  {#if vault.error}
    <div class="border-t border-red-900 bg-red-950/50 px-4 py-2 text-xs text-red-300">
      {vault.error}
      <button class="ml-2 underline" onclick={() => (vault.error = null)}>close</button>
    </div>
  {/if}
</main>
