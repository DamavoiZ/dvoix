<script lang="ts">
  // The changes of a file, shown over the app: the working tree, the index or
  // a commit against the version before. Old and new sit side by side and
  // scroll together, or share one column; the choice is kept in the settings.
  import { Columns2, Rows2, X } from "@lucide/svelte";
  import { createDiffView, type DiffLayout } from "./diffView";
  import { gitSettings } from "./settings";
  import { gitState } from "./state.svelte";

  let host = $state<HTMLDivElement>();
  const layout = $derived(gitSettings.value.diffLayout);
  /** Line endings alone are no change worth a view; CodeMirror shows both the same. */
  const lf = (text: string) => text.replace(/\r\n?/g, "\n");
  const unchanged = $derived(!!gitState.diff && lf(gitState.diff.old) === lf(gitState.diff.new));

  function setLayout(value: DiffLayout) {
    gitSettings.value.diffLayout = value;
    gitSettings.save();
  }

  // A new diff or another layout builds the view again; it is cheap and keeps
  // the two CodeMirror setups apart.
  $effect(() => {
    const diff = gitState.diff;
    if (!host || !diff || unchanged) return;
    const view = createDiffView(host, diff.old, diff.new, layout);
    return () => view.destroy();
  });

  const toggle = "flex h-6 items-center gap-1.5 rounded px-2 text-xs";
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && gitState.closeDiff()} />

{#if gitState.diff}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="fixed inset-0 z-40 bg-black/40" onclick={() => gitState.closeDiff()}></div>
  <div
    class="fixed top-[6vh] left-1/2 z-50 flex h-[88vh] w-[min(1400px,96vw)] -translate-x-1/2 flex-col overflow-hidden rounded-xl border border-zinc-700 bg-zinc-900 shadow-2xl"
    role="dialog"
    aria-label="Changes"
  >
    <header class="flex shrink-0 items-center gap-2 border-b border-zinc-800 px-4 py-2">
      <span class="truncate text-sm text-zinc-200">{gitState.diff.title}</span>
      <div class="ml-auto flex items-center gap-0.5 rounded-md bg-zinc-950 p-0.5" role="group" aria-label="View">
        <button
          class="{toggle} {layout === 'split' ? 'bg-zinc-800 text-zinc-100' : 'text-zinc-500 hover:text-zinc-200'}"
          aria-pressed={layout === "split"}
          title="Before and after side by side"
          onclick={() => setLayout("split")}
        >
          <Columns2 size={13} strokeWidth={1.75} />
          Nebeneinander
        </button>
        <button
          class="{toggle} {layout === 'inline' ? 'bg-zinc-800 text-zinc-100' : 'text-zinc-500 hover:text-zinc-200'}"
          aria-pressed={layout === "inline"}
          title="Everything in one column, removed lines above the new ones"
          onclick={() => setLayout("inline")}
        >
          <Rows2 size={13} strokeWidth={1.75} />
          Untereinander
        </button>
      </div>
      <button
        class="flex h-6 w-6 items-center justify-center rounded text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
        aria-label="Close"
        onclick={() => gitState.closeDiff()}
      >
        <X size={14} strokeWidth={1.75} />
      </button>
    </header>
    {#if unchanged}
      <p class="p-4 text-sm text-zinc-500">No changes to this file.</p>
    {:else}
      {#if layout === "split"}
        <div class="grid shrink-0 grid-cols-2 border-b border-zinc-800 text-xs text-zinc-500">
          <span class="px-4 py-1">Before</span>
          <span class="border-l border-zinc-800 px-4 py-1">After</span>
        </div>
      {/if}
      <div class="min-h-0 flex-1" bind:this={host}></div>
    {/if}
  </div>
{/if}
