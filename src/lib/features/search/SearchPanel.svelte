<script lang="ts">
  import { Search, X } from "@lucide/svelte";
  import { api, type SearchHit } from "../../api";
  import { vault } from "../../vault.svelte";

  let query = $state("");
  let hits = $state<SearchHit[]>([]);

  $effect(() => {
    const q = query;
    void vault.changes.notes.number; // search again when files change
    const timer = setTimeout(async () => {
      hits = q.trim() ? await api.search(q).catch(() => []) : [];
    }, 120);
    return () => clearTimeout(timer);
  });
</script>

<div class="px-3 pt-2.5 pb-2">
  <label class="flex items-center gap-2 rounded-md border border-zinc-700/80 bg-zinc-950 px-2.5 py-1.5 focus-within:border-violet-500">
    <Search size={14} class="shrink-0 text-zinc-500" />
    <input
      class="min-w-0 flex-1 bg-transparent text-sm text-zinc-200 outline-none placeholder:text-zinc-500"
      placeholder="Full-text search …"
      bind:value={query}
    />
    {#if query}
      <button
        type="button"
        class="rounded p-0.5 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-200"
        aria-label="Clear search"
        onclick={() => (query = "")}
      >
        <X size={13} />
      </button>
    {/if}
  </label>
</div>
<div class="flex-1 overflow-y-auto px-1.5 pb-4">
  {#each hits as hit (hit.path)}
    <button
      class="block w-full rounded-md px-2.5 py-2 text-left transition-colors hover:bg-zinc-800/70"
      onclick={() => vault.openFile(hit.path)}
    >
      <div class="truncate text-sm font-medium text-zinc-100">{hit.title}</div>
      <div class="mt-0.5 line-clamp-2 text-xs leading-relaxed text-zinc-400">{hit.snippet}</div>
    </button>
  {:else}
    {#if query.trim()}
      <p class="px-2.5 py-2 text-xs text-zinc-500">Nothing found for "{query.trim()}".</p>
    {:else}
      <p class="px-2.5 py-2 text-xs text-zinc-500">Type to search across all notes in your vault.</p>
    {/if}
  {/each}
</div>
