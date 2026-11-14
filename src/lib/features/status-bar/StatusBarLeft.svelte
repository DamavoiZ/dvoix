<script lang="ts">
  import { fileTypeFor } from "../../filetypes";
  import { parseFrontmatter } from "../../filetypes/markdown/frontmatter";
  import { plugins } from "../../plugins";
  import { vault } from "../../vault.svelte";

  const isMarkdown = $derived(
    vault.openPath ? fileTypeFor(vault.openPath)?.id === "markdown" : false,
  );

  const markdownStats = $derived.by(() => {
    if (!vault.openPath || !isMarkdown) return null;
    const text = vault.content;
    const fm = parseFrontmatter(text);
    const body = fm ? text.slice(fm.to + fm.eol.length) : text;
    const trimmed = body.trim();
    const words = trimmed ? trimmed.split(/\s+/).length : 0;
    const chars = body.length;
    const readTime =
      words === 0 ? "0 min read" : words < 200 ? "< 1 min read" : `~${Math.ceil(words / 200)} min read`;
    return { words, chars, readTime };
  });

  const viewLabel = $derived(
    vault.openView ? (plugins.view(vault.openView)?.label ?? vault.openView) : null,
  );

  const fileTypeLabel = $derived(
    vault.openPath ? (fileTypeFor(vault.openPath)?.label ?? "File") : null,
  );

  const noteCount = $derived(
    vault.entries.filter((e) => !e.isDir && e.name.toLowerCase().endsWith(".md")).length,
  );
</script>

{#if viewLabel}
  <span class="truncate text-zinc-400">{viewLabel}</span>
{:else if markdownStats}
  <div class="flex items-center gap-2.5 tabular-nums">
    <span>{markdownStats.words} {markdownStats.words === 1 ? "word" : "words"}</span>
    <span class="text-zinc-700" aria-hidden="true">·</span>
    <span>{markdownStats.chars} {markdownStats.chars === 1 ? "character" : "characters"}</span>
    <span class="text-zinc-700" aria-hidden="true">·</span>
    <span>{markdownStats.readTime}</span>
  </div>
{:else if vault.openPath}
  <div class="flex items-center gap-2.5 tabular-nums">
    <span>{fileTypeLabel}</span>
    {#if vault.content.length > 0}
      <span class="text-zinc-700" aria-hidden="true">·</span>
      <span>{vault.content.length} {vault.content.length === 1 ? "character" : "characters"}</span>
    {/if}
  </div>
{:else}
  <span class="tabular-nums">{noteCount} {noteCount === 1 ? "note" : "notes"}</span>
{/if}
