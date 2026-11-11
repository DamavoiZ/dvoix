<script lang="ts">
  // "Linked from": notes that link to the open note.
  import { api, type NoteSummary } from "../../api";
  import { fileTypeFor } from "../../filetypes";
  import { vault } from "../../vault.svelte";

  let { path }: { path: string } = $props();

  /** How many links are listed at first; the rest wait behind "more". */
  const PAGE = 50;

  let backlinks = $state<NoteSummary[]>([]);
  let limit = $state(PAGE);
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    void vault.changes.notes.number;
    const p = path;
    limit = PAGE;
    // Once per batch of changes, not once per changed note.
    clearTimeout(timer);
    timer = setTimeout(() => {
      if (fileTypeFor(p)?.id === "markdown") api.backlinks(p).then((b) => (backlinks = b)).catch(() => (backlinks = []));
      else backlinks = [];
    }, 150);
    return () => clearTimeout(timer);
  });

  const shown = $derived(backlinks.length <= limit ? backlinks : backlinks.slice(0, limit));
  const hiddenCount = $derived(backlinks.length - shown.length);
</script>

{#if backlinks.length}
  <footer class="border-t border-zinc-800 px-4 py-2 text-xs text-zinc-500">
    Linked from:
    {#each shown as b (b.path)}
      <button class="ml-2 text-violet-300 hover:underline" onclick={() => vault.openFile(b.path)}>{b.title}</button>
    {/each}
    {#if hiddenCount > 0}
      <button class="ml-2 text-zinc-400 hover:text-zinc-200 hover:underline" onclick={() => (limit += PAGE)}>
        and {hiddenCount} more
      </button>
    {/if}
  </footer>
{/if}
