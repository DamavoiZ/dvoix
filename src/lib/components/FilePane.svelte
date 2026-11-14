<script lang="ts">
  // The open file: the file type's view and footers from features and plugins
  // (backlinks). A tab can also show a plugin view instead. The headers (tab
  // bar) and window buttons sit in the title row above (see `App.svelte`).
  import { ArrowLeft, ArrowRight, FileText, Search } from "@lucide/svelte";
  import { fileTypeFor } from "../filetypes";
  import { plugins } from "../plugins";
  import { ui } from "../ui.svelte";
  import { vault } from "../vault.svelte";

  const path = $derived(vault.openPath);
  const segments = $derived(path ? path.split("/") : []);
  const type = $derived(path ? fileTypeFor(path) : undefined);
  /** A plugin view (e.g. the graph) shown in the active tab instead of a file. */
  const view = $derived(vault.openView ? plugins.view(vault.openView) : undefined);
  const SWITCHER = "quick-switcher:open";
  const navBtn =
    "flex h-6 w-6 items-center justify-center rounded text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-200 disabled:pointer-events-none disabled:opacity-30";
</script>

<main class="flex min-w-0 flex-1 flex-col">
  {#if view}
    <div class="min-h-0 flex-1">
      {#key view.id}
        <view.component />
      {/key}
    </div>
  {:else if path && type}
    <header class="flex h-9 shrink-0 items-center justify-between gap-3 border-b border-zinc-800 bg-zinc-950/60 px-3 text-xs">
      <div class="flex min-w-0 items-center gap-1.5 truncate">
        <div class="mr-0.5 flex shrink-0 items-center gap-0.5">
          <button
            class={navBtn}
            title="Navigate back (Alt+Left)"
            aria-label="Navigate back"
            disabled={!vault.canGoBack}
            onclick={() => vault.goBack()}
          >
            <ArrowLeft size={14} strokeWidth={1.75} />
          </button>
          <button
            class={navBtn}
            title="Navigate forward (Alt+Right)"
            aria-label="Navigate forward"
            disabled={!vault.canGoForward}
            onclick={() => vault.goForward()}
          >
            <ArrowRight size={14} strokeWidth={1.75} />
          </button>
        </div>
        <type.icon size={13} strokeWidth={1.75} class="shrink-0 text-zinc-500" />
        <nav class="flex min-w-0 items-center gap-1 truncate" aria-label="File path" title={path}>
          {#each segments as seg, i (i)}
            {#if i > 0}<span class="text-zinc-600">/</span>{/if}
            {#if i < segments.length - 1}
              <button
                type="button"
                class="truncate rounded px-1 py-0.5 text-zinc-500 transition-colors hover:bg-zinc-800/70 hover:text-zinc-200"
                title="Reveal folder in explorer"
                onclick={() => ui.revealFolder(segments.slice(0, i + 1).join("/"))}
              >
                {seg}
              </button>
            {:else}
              <span class="truncate font-medium text-zinc-200">
                {type.id === "markdown" ? seg.replace(/\.md$/i, "") : seg}
              </span>
            {/if}
          {/each}
        </nav>
      </div>
      <div class="flex shrink-0 items-center gap-3">
        <span class="inline-flex items-center gap-1.5 text-[11px] {vault.dirty ? 'text-amber-300/90' : 'text-zinc-500'}">
          <span class="h-1.5 w-1.5 rounded-full {vault.dirty ? 'bg-amber-400' : 'bg-zinc-600'}"></span>
          {vault.dirty ? "Saving …" : "Saved"}
        </span>
        {#if type.headerActions}
          <type.headerActions {path} />
        {/if}
      </div>
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
    <div class="flex flex-1 flex-col items-center justify-center gap-4 px-6 text-center">
      <div class="flex h-11 w-11 items-center justify-center rounded-xl border border-zinc-800 bg-zinc-900/70 text-zinc-500">
        <FileText size={20} strokeWidth={1.5} />
      </div>
      <div class="space-y-1">
        <p class="text-sm font-medium text-zinc-200">No file open</p>
        <p class="text-xs text-zinc-500">Pick a file from the explorer or jump straight to a note.</p>
      </div>
      {#if plugins.has(SWITCHER)}
        <button
          class="inline-flex items-center gap-2 rounded-lg border border-zinc-700/80 bg-zinc-900 px-3.5 py-2 text-xs font-medium text-zinc-200 transition-colors hover:border-zinc-600 hover:bg-zinc-800"
          onclick={() => plugins.run(SWITCHER)}
        >
          <Search size={13} class="text-violet-400" />
          Find a file
          <kbd>Ctrl+O</kbd>
        </button>
      {/if}
    </div>
  {/if}
  {#if vault.error}
    <div class="flex items-center justify-between border-t border-red-900 bg-red-950/50 px-4 py-2 text-xs text-red-300">
      <span>{vault.error}</span>
      <button class="ml-3 rounded px-1.5 py-0.5 text-red-200 hover:bg-red-900/50" onclick={() => (vault.error = null)}>Dismiss</button>
    </div>
  {/if}
</main>
