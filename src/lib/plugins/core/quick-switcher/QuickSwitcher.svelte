<script lang="ts">
  // Quick switcher (Ctrl+O): fuzzy search over all files of the vault, like Obsidian's.
  import { FilePlus } from "@lucide/svelte";
  import { fileTypeFor } from "../../../filetypes";
  import { fuzzyMatch, highlight } from "../../../fuzzy";
  import { ui } from "../../../ui.svelte";
  import { vault } from "../../../vault.svelte";

  const LIMIT = 60;

  interface Item {
    path: string;
    /** What is shown and searched: the path, notes without ".md". */
    label: string;
    nameStart: number;
    badge: string;
    indices: number[];
  }

  let query = $state("");
  let selected = $state(0);
  let list: HTMLDivElement | undefined = $state();

  const files = $derived.by(() => {
    const all = vault.entries.filter((e) => !e.isDir);
    const rank = new Map(vault.recent.map((p, i) => [p, i]));
    // Without a query: recently opened first, then newest changes.
    return all.sort((a, b) => (rank.get(a.path) ?? 1e6) - (rank.get(b.path) ?? 1e6) || b.modified - a.modified);
  });

  const items = $derived.by((): Item[] => {
    const q = query.trim();
    const scored: { item: Item; score: number }[] = [];
    for (const f of files) {
      const isNote = fileTypeFor(f.path)?.id === "markdown";
      const label = isNote ? f.path.replace(/\.md$/i, "") : f.path;
      const m = fuzzyMatch(q, label);
      if (!m) continue;
      const item = { path: f.path, label, nameStart: label.lastIndexOf("/") + 1, badge: isNote ? "" : f.extension.toUpperCase(), indices: m.indices };
      scored.push({ item, score: m.score });
    }
    if (q) scored.sort((a, b) => b.score - a.score);
    return scored.slice(0, LIMIT).map((s) => s.item);
  });

  /** Offer to create a note when nothing has exactly this name. */
  const canCreate = $derived.by(() => {
    const q = query.trim();
    return !!q && !vault.resolveLink(q);
  });
  const count = $derived(items.length + (canCreate ? 1 : 0));

  $effect(() => {
    void query;
    selected = 0;
  });

  $effect(() => {
    const el = list?.querySelector<HTMLElement>(`[data-index="${selected}"]`);
    el?.scrollIntoView({ block: "nearest" });
  });

  function focus(el: HTMLInputElement) {
    requestAnimationFrame(() => el.focus());
  }

  function close() {
    if (ui.modal === "quick-switcher") ui.modal = null;
    query = "";
  }

  function choose(index: number, newTab: boolean) {
    const q = query.trim();
    const item = items[index]; // read before close() resets the query
    close();
    if (item) void vault.openFile(item.path, { newTab });
    else if (q) void vault.openLink(q, { newTab });
  }

  function keydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (e.key === "Escape") close();
    else if (e.key === "ArrowDown" || (mod && e.key === "n")) selected = count ? (selected + 1) % count : 0;
    else if (e.key === "ArrowUp" || (mod && e.key === "p")) selected = count ? (selected - 1 + count) % count : 0;
    else if (e.key === "Enter" && e.shiftKey && query.trim()) choose(items.length, mod); // always create
    else if (e.key === "Enter" && count) choose(selected, mod);
    else return;
    e.preventDefault();
  }
</script>

{#if ui.modal === "quick-switcher"}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="fixed inset-0 z-40 bg-black/40" onclick={close}></div>
  <div
    class="fixed top-[12vh] left-1/2 z-50 flex max-h-[70vh] w-[min(640px,92vw)] -translate-x-1/2 flex-col overflow-hidden rounded-xl border border-zinc-700 bg-zinc-900 shadow-2xl"
    role="dialog"
    aria-label="Quick switcher"
  >
    <input
      class="border-b border-zinc-800 bg-transparent px-4 py-3 text-[15px] text-zinc-100 outline-none placeholder:text-zinc-600"
      placeholder="Find a file …"
      use:focus
      bind:value={query}
      onkeydown={keydown}
      onblur={close}
    />
    <div class="min-h-0 flex-1 overflow-y-auto p-1.5" bind:this={list} role="listbox">
      {#each items as item, i (item.path)}
        <button
          class="flex w-full items-center gap-2 rounded-md px-3 py-1.5 text-left {i === selected ? 'bg-zinc-800' : ''}"
          data-index={i}
          role="option"
          aria-selected={i === selected}
          onmousedown={(e) => e.preventDefault()}
          onmousemove={() => (selected = i)}
          onclick={(e) => choose(i, e.ctrlKey || e.metaKey)}
        >
          <span class="min-w-0 flex-1 truncate">
            <span class="text-sm text-zinc-100">
              {#each highlight(item.label.slice(item.nameStart), item.indices, item.nameStart) as part, k (k)}
                <span class={part.hit ? "font-semibold text-violet-300" : ""}>{part.text}</span>
              {/each}
            </span>
            {#if item.nameStart}
              <span class="ml-2 text-xs text-zinc-500">
                {#each highlight(item.label.slice(0, item.nameStart - 1), item.indices) as part, k (k)}
                  <span class={part.hit ? "text-violet-400" : ""}>{part.text}</span>
                {/each}
              </span>
            {/if}
          </span>
          {#if item.badge}
            <span class="shrink-0 rounded bg-zinc-800 px-1 py-px text-[9px] font-semibold tracking-wider text-zinc-400">{item.badge}</span>
          {/if}
        </button>
      {/each}
      {#if canCreate}
        <button
          class="flex w-full items-center gap-2 rounded-md px-3 py-1.5 text-left text-sm text-zinc-300 {selected === items.length ? 'bg-zinc-800' : ''}"
          data-index={items.length}
          role="option"
          aria-selected={selected === items.length}
          onmousedown={(e) => e.preventDefault()}
          onmousemove={() => (selected = items.length)}
          onclick={(e) => choose(items.length, e.ctrlKey || e.metaKey)}
        >
          <FilePlus size={15} strokeWidth={1.75} class="text-zinc-500" />
          Create note "{query.trim()}"
        </button>
      {/if}
      {#if !count}
        <p class="px-3 py-2 text-sm text-zinc-500">No files in the vault.</p>
      {/if}
    </div>
    <footer class="flex gap-4 border-t border-zinc-800 px-4 py-2 text-[11px] text-zinc-500">
      <span><kbd>↑↓</kbd> to select</span>
      <span><kbd>Enter</kbd> to open</span>
      {#if vault.tabsEnabled}<span><kbd>Ctrl+Enter</kbd> in new tab</span>{/if}
      <span><kbd>Shift+Enter</kbd> to create new</span>
      <span><kbd>Esc</kbd> to close</span>
    </footer>
  </div>
{/if}
