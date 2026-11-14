<script lang="ts">
  // The outline of the open note: its headings as a tree. Clicking a heading
  // jumps to it, in the editor and in the reading view.
  import { reveal } from "../../../reveal.svelte";
  import { vault } from "../../../vault.svelte";
  import { outlineEntries, parseHeadings, type Heading, type OutlineEntry } from "./headings";

  /** The path of the open note, if it is a note. */
  const path = $derived(vault.openPath && /\.(md|markdown)$/i.test(vault.openPath) ? vault.openPath : null);

  /** The headings of the open note, nested as they are in the note. */
  let entries = $state<OutlineEntry[]>([]);

  let timer: ReturnType<typeof setTimeout> | undefined;
  let lastPath: string | null = null;
  let lastText = "";
  // Typing changes the text on every key press; the whole note is only
  // walked once the typing settles, and only when it really changed.
  const update = (text: string) => {
    if (text === lastText) return;
    lastText = text;
    entries = outlineEntries(parseHeadings(text));
  };
  $effect(() => {
    const p = path;
    const text = vault.content;
    clearTimeout(timer);
    if (!p) {
      lastPath = null;
      lastText = "";
      entries = [];
      return;
    }
    if (p !== lastPath) {
      lastPath = p;
      update(text);
    } else if (vault.dirty) {
      timer = setTimeout(() => update(text), 150);
    } else {
      update(text);
    }
    return () => clearTimeout(timer);
  });

  function jump(heading: Heading, index: number) {
    if (!path) return;
    reveal.show(path, heading.line, heading.text, index);
  }
</script>

{#if !path}
  <p class="px-4 py-3 text-xs text-zinc-500">Open a note to see its outline.</p>
{:else if !entries.length}
  <p class="px-4 py-3 text-xs text-zinc-500">This note has no headings.</p>
{:else}
  <ul class="px-1.5 py-1.5">
    {#each entries as entry, index (entry.heading.line)}
      <li>
        <button
          class="block w-full truncate rounded-md py-1 pr-2.5 text-left text-sm text-zinc-400 transition-colors hover:bg-zinc-800/60 hover:text-zinc-100"
          style:padding-left="{8 + entry.depth * 12}px"
          title={entry.heading.text}
          onclick={() => jump(entry.heading, index)}
        >
          {entry.heading.text || "Untitled"}
        </button>
      </li>
    {/each}
  </ul>
{/if}
