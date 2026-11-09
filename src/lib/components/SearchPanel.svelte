<script lang="ts">
  import { api, type SearchHit } from "../api";
  import { vault } from "../vault.svelte";

  let query = $state("");
  let hits = $state<SearchHit[]>([]);

  $effect(() => {
    const q = query;
    void vault.revision; // search again when files change
    const timer = setTimeout(async () => {
      hits = q.trim() ? await api.search(q).catch(() => []) : [];
    }, 120);
    return () => clearTimeout(timer);
  });
</script>

<div class="px-3 pb-2">
  <input
    class="w-full rounded bg-zinc-900 px-2 py-1.5 text-sm outline-none placeholder:text-zinc-600 focus:ring-1 focus:ring-violet-500"
    placeholder="Volltextsuche …"
    bind:value={query}
  />
</div>
<div class="flex-1 overflow-y-auto px-1 pb-4">
  {#each hits as hit (hit.path)}
    <button class="block w-full rounded px-2 py-1.5 text-left hover:bg-zinc-800/70" onclick={() => vault.openFile(hit.path)}>
      <div class="truncate text-sm text-zinc-100">{hit.title}</div>
      <div class="line-clamp-2 text-xs text-zinc-500">{hit.snippet}</div>
    </button>
  {:else}
    {#if query.trim()}<p class="px-3 text-sm text-zinc-500">Nichts gefunden.</p>{/if}
  {/each}
</div>
