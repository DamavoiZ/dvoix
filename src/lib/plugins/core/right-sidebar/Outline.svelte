<script lang="ts">
  // The outline of the open note: its headings as a tree. Clicking a heading
  // jumps to it, in the editor and in the reading view.
  import { reveal } from "../../../reveal.svelte";
  import { vault } from "../../../vault.svelte";
  import { outlineEntries, parseHeadings, type Heading } from "./headings";

  /** The path of the open note, if it is a note. */
  const path = $derived(vault.openPath && /\.(md|markdown)$/i.test(vault.openPath) ? vault.openPath : null);

  /** The headings of the open note, nested as they are in the note. */
  const entries = $derived(path ? outlineEntries(parseHeadings(vault.content)) : []);

  function jump(heading: Heading) {
    if (!path) return;
    reveal.show(path, heading.line, heading.text);
  }
</script>

{#if !path}
  <p class="px-4 py-3 text-xs text-zinc-600">Open a note to see its outline.</p>
{:else if !entries.length}
  <p class="px-4 py-3 text-xs text-zinc-600">This note has no headings.</p>
{:else}
  <ul class="py-1">
    {#each entries as entry (entry.heading.line)}
      <li>
        <button
          class="block w-full truncate py-1 pr-3 text-left text-sm text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-100"
          style:padding-left="{10 + entry.depth * 12}px"
          title={entry.heading.text}
          onclick={() => jump(entry.heading)}
        >
          {entry.heading.text || "Untitled"}
        </button>
      </li>
    {/each}
  </ul>
{/if}
