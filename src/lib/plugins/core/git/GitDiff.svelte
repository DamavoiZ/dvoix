<script lang="ts">
  // The changes of a file, shown over the app: the diff of the working tree,
  // of the index or of a commit.
  import { X } from "@lucide/svelte";
  import { gitState } from "./state.svelte";

  /** The color a line of the diff gets. */
  function lineClass(line: string): string {
    if (line.startsWith("+++") || line.startsWith("---")) return "text-zinc-500";
    if (line.startsWith("@@")) return "text-violet-300";
    if (line.startsWith("+")) return "bg-green-950/40 text-green-300";
    if (line.startsWith("-")) return "bg-red-950/40 text-red-300";
    return "text-zinc-400";
  }
</script>

{#if gitState.diff}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="fixed inset-0 z-40 bg-black/40" onclick={() => gitState.closeDiff()}></div>
  <div
    class="fixed top-[8vh] left-1/2 z-50 flex max-h-[80vh] w-[min(960px,94vw)] -translate-x-1/2 flex-col overflow-hidden rounded-xl border border-zinc-700 bg-zinc-900 shadow-2xl"
    role="dialog"
    aria-label="Änderungen"
  >
    <header class="flex shrink-0 items-center gap-2 border-b border-zinc-800 px-4 py-2">
      <span class="truncate text-sm text-zinc-200">{gitState.diff.title}</span>
      <button
        class="ml-auto flex h-6 w-6 items-center justify-center rounded text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
        aria-label="Schließen"
        onclick={() => gitState.closeDiff()}
      >
        <X size={14} strokeWidth={1.75} />
      </button>
    </header>
    <div class="min-h-0 flex-1 overflow-auto p-2 font-mono text-xs leading-relaxed">
      {#if gitState.diff.text.trim()}
        {#each gitState.diff.text.split("\n") as line, i (i)}
          <div class="whitespace-pre-wrap break-all {lineClass(line)}">{line || " "}</div>
        {/each}
      {:else}
        <p class="p-2 text-sm text-zinc-500">Keine Änderungen an dieser Datei.</p>
      {/if}
    </div>
  </div>
{/if}
