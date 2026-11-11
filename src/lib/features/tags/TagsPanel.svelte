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

<div class="flex flex-wrap gap-1 px-3 pb-3">
  {#each tags as t (t.tag)}
    <button
      class="rounded-full px-2 py-0.5 text-xs {selected === t.tag ? 'bg-violet-600 text-white' : 'bg-zinc-800 text-zinc-300 hover:bg-zinc-700'}"
      onclick={() => pick(t.tag)}
    >
      #{t.tag} <span class="opacity-60">{t.count}</span>
    </button>
  {:else}
    <p class="text-sm text-zinc-500">No tags yet. Use <code>#tag</code> in the text or <code>tags:</code> in the frontmatter.</p>
  {/each}
</div>
<div class="flex-1 overflow-y-auto px-1 pb-4">
  {#each shownNotes as note (note.path)}
    <button class="block w-full truncate rounded px-2 py-1 text-left text-sm hover:bg-zinc-800/70" onclick={() => vault.openFile(note.path)}>
      {note.title}
    </button>
  {/each}
  {#if notes.length > shownNotes.length}
    <button
      class="block w-full rounded px-2 py-1 text-left text-xs text-zinc-500 hover:text-zinc-300"
      onclick={() => (noteLimit += NOTE_PAGE)}
    >
      Show {Math.min(notes.length - shownNotes.length, NOTE_PAGE)} more ({notes.length - shownNotes.length} hidden)
    </button>
  {/if}
</div>
