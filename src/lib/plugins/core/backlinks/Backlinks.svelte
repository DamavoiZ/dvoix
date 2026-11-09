<script lang="ts">
  // "Linked from": notes that link to the open note.
  import { api, type NoteSummary } from "../../../api";
  import { fileTypeFor } from "../../../filetypes";
  import { vault } from "../../../vault.svelte";

  let { path }: { path: string } = $props();

  let backlinks = $state<NoteSummary[]>([]);
  $effect(() => {
    void vault.revision;
    const p = path;
    if (fileTypeFor(p)?.id === "markdown") api.backlinks(p).then((b) => (backlinks = b)).catch(() => (backlinks = []));
    else backlinks = [];
  });
</script>

{#if backlinks.length}
  <footer class="border-t border-zinc-800 px-4 py-2 text-xs text-zinc-500">
    Linked from:
    {#each backlinks as b (b.path)}
      <button class="ml-2 text-violet-300 hover:underline" onclick={() => vault.openFile(b.path)}>{b.title}</button>
    {/each}
  </footer>
{/if}
