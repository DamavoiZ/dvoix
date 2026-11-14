<script lang="ts">
  // "Linked from": notes that link to the open note.
  import { Link2 } from "@lucide/svelte";
  import { api, type NoteSummary } from "../../api";
  import { fileTypeFor } from "../../filetypes";
  import { vault } from "../../vault.svelte";

  let { path }: { path: string } = $props();

  /** How many links are listed at first; the rest wait behind "more". */
  const PAGE = 50;

  let backlinks = $state<NoteSummary[]>([]);
  let limit = $state(PAGE);
  /** The note the shown links belong to; a switch drops them right away. */
  let shownFor = $state("");
  /** Drops answers that arrive after a newer request. */
  let generation = 0;
  let lastPath = "";
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    void vault.changes.notes.number;
    const p = path;
    const switched = p !== lastPath;
    lastPath = p;
    if (switched) {
      // Another note: the old links and the expanded list must not stay on
      // screen while the new ones load. An autosave of the same note keeps
      // both, so "more" stays open while typing.
      generation++;
      backlinks = [];
      limit = PAGE;
      shownFor = p;
    }
    const run = generation;
    const fetch = () => {
      if (fileTypeFor(p)?.id !== "markdown") {
        backlinks = [];
        return;
      }
      api
        .backlinks(p)
        .then((b) => {
          if (run === generation) backlinks = b;
        })
        .catch(() => {
          if (run === generation) backlinks = [];
        });
    };
    // Once per batch of changes, not once per changed note; a switch loads
    // right away so the new note's links appear without the debounce delay.
    clearTimeout(timer);
    if (switched) fetch();
    else timer = setTimeout(fetch, 150);
    return () => clearTimeout(timer);
  });

  const shown = $derived(backlinks.length <= limit ? backlinks : backlinks.slice(0, limit));
  const hiddenCount = $derived(backlinks.length - shown.length);
</script>

{#if shownFor === path && backlinks.length}
  <footer class="flex flex-wrap items-center gap-1.5 border-t border-zinc-800 bg-zinc-950/50 px-4 py-2 text-xs text-zinc-500">
    <span class="mr-1 inline-flex items-center gap-1 text-[11px] font-medium text-zinc-400">
      <Link2 size={12} strokeWidth={1.75} />
      Linked from
    </span>
    {#each shown as b (b.path)}
      <button
        class="rounded-md border border-zinc-800 bg-zinc-900/70 px-2 py-0.5 text-xs text-violet-300 transition-colors hover:border-zinc-700 hover:bg-zinc-800"
        onclick={() => vault.openFile(b.path)}
      >
        {b.title}
      </button>
    {/each}
    {#if hiddenCount > 0}
      <button
        class="rounded-md px-1.5 py-0.5 text-xs text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-200"
        onclick={() => (limit += PAGE)}
      >
        +{hiddenCount} more
      </button>
    {/if}
  </footer>
{/if}
