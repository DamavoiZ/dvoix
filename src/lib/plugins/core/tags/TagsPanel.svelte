<script lang="ts">
  import { api, type NoteSummary, type TagCount } from "../../../api";
  import { vault } from "../../../vault.svelte";

  let tags = $state<TagCount[]>([]);
  let selected = $state<string | null>(null);
  let notes = $state<NoteSummary[]>([]);

  $effect(() => {
    void vault.revision;
    api.tags().then((t) => (tags = t)).catch(() => (tags = []));
  });

  $effect(() => {
    void vault.revision;
    const tag = selected;
    if (tag) api.notesWithTag(tag).then((n) => (notes = n)).catch(() => (notes = []));
    else notes = [];
  });
</script>

<div class="flex flex-wrap gap-1 px-3 pb-3">
  {#each tags as t (t.tag)}
    <button
      class="rounded-full px-2 py-0.5 text-xs {selected === t.tag ? 'bg-violet-600 text-white' : 'bg-zinc-800 text-zinc-300 hover:bg-zinc-700'}"
      onclick={() => (selected = selected === t.tag ? null : t.tag)}
    >
      #{t.tag} <span class="opacity-60">{t.count}</span>
    </button>
  {:else}
    <p class="text-sm text-zinc-500">No tags yet. Use <code>#tag</code> in the text or <code>tags:</code> in the frontmatter.</p>
  {/each}
</div>
<div class="flex-1 overflow-y-auto px-1 pb-4">
  {#each notes as note (note.path)}
    <button class="block w-full truncate rounded px-2 py-1 text-left text-sm hover:bg-zinc-800/70" onclick={() => vault.openFile(note.path)}>
      {note.title}
    </button>
  {/each}
</div>
