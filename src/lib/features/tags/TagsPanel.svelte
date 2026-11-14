<script lang="ts">
  import { api, type NoteSummary, type TagCount } from "../../api";
  import { vault } from "../../vault.svelte";

  let tags = $state<TagCount[]>([]);
  let selected = $state<string | null>(null);
  let notes = $state<NoteSummary[]>([]);
  /** How many notes of a tag are drawn; the rest wait behind "Show more". */
  const NOTE_PAGE = 200;
  let noteLimit = $state(NOTE_PAGE);

  let tagTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    void vault.changes.notes.number;
    // Once per burst of changes, not once per changed note.
    clearTimeout(tagTimer);
    tagTimer = setTimeout(() => {
      api.tags().then((t) => (tags = t)).catch(() => (tags = []));
    }, 150);
    return () => clearTimeout(tagTimer);
  });

  let noteTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    void vault.changes.notes.number;
    const tag = selected;
    clearTimeout(noteTimer);
    noteTimer = setTimeout(() => {
      if (tag) api.notesWithTag(tag).then((n) => (notes = n)).catch(() => (notes = []));
      else notes = [];
    }, 150);
    return () => clearTimeout(noteTimer);
  });

  function pick(tag: string) {
    selected = selected === tag ? null : tag;
    noteLimit = NOTE_PAGE;
  }

  const shownNotes = $derived(notes.length <= noteLimit ? notes : notes.slice(0, noteLimit));
</script>

<div class="flex flex-wrap gap-1.5 border-b border-zinc-800/70 px-3 pt-2.5 pb-3">
  {#each tags as t (t.tag)}
    <button
      class="inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs transition-colors {selected === t.tag
        ? 'bg-violet-600 text-white'
        : 'bg-zinc-800/90 text-zinc-300 hover:bg-zinc-700'}"
      onclick={() => pick(t.tag)}
    >
      <span>#{t.tag}</span>
      <span class="text-[11px] opacity-70">{t.count}</span>
    </button>
  {:else}
    <p class="text-xs leading-relaxed text-zinc-500">No tags yet. Use <code>#tag</code> in the text or <code>tags:</code> in the frontmatter.</p>
  {/each}
</div>
<div class="flex-1 overflow-y-auto px-1.5 pt-2 pb-4">
  {#if selected}
    <div class="px-2 pb-1 text-[11px] font-medium tracking-wide text-zinc-500 uppercase">
      #{selected} · {notes.length} {notes.length === 1 ? "note" : "notes"}
    </div>
  {/if}
  {#each shownNotes as note (note.path)}
    <button
      class="block w-full truncate rounded-md px-2 py-1.5 text-left text-sm text-zinc-200 transition-colors hover:bg-zinc-800/70"
      onclick={() => vault.openFile(note.path)}
    >
      {note.title}
    </button>
  {/each}
  {#if notes.length > shownNotes.length}
    <button
      class="block w-full rounded-md px-2 py-1 text-left text-xs text-zinc-500 transition-colors hover:text-zinc-300"
      onclick={() => (noteLimit += NOTE_PAGE)}
    >
      Show {Math.min(notes.length - shownNotes.length, NOTE_PAGE)} more ({notes.length - shownNotes.length} hidden)
    </button>
  {/if}
</div>
