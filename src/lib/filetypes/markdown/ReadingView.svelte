<script lang="ts">
  // The reading view: the note as it looks when it is read, not edited.
  // The HTML comes from the Rust core, so it is the same markdown the app
  // indexes and searches.
  import { api, type RenderedNote } from "../../api";
  import type { FileViewProps } from "../types";
  import Properties from "./Properties.svelte";
  import { applyChange, parseFrontmatter } from "./frontmatter";
  import { highlightCodeBlocks } from "./codeHighlight";
  import { embedImage, isImage, parseEmbed } from "../../attachments";

  let { path, content, onchange, onopen }: FileViewProps = $props();

  let rendered = $state<RenderedNote | null>(null);
  let failed = $state(false);
  let prose = $state<HTMLElement | null>(null);
  /** The text of the note without the properties, as it was shown last. */
  let lastBody = $state<string | undefined>();
  /** The render that is running, so an older one does not overwrite a newer. */
  let running: { stale: boolean } | undefined;

  // Code blocks are colored after the HTML is in place; `{@html}` replaces
  // them on every render, so this runs again for each new HTML.
  $effect(() => {
    if (prose && rendered) void highlightCodeBlocks(prose);
  });

  // `![[photo.png]]` comes from the core as a link with `data-embed`; images
  // are shown in its place, other files stay links.
  $effect(() => {
    if (!prose || !rendered) return;
    for (const link of prose.querySelectorAll<HTMLElement>("a[data-embed]")) {
      const { target, width } = parseEmbed(link.dataset.embed ?? "");
      if (isImage(target)) link.replaceWith(embedImage(target, width ?? (Number(link.textContent) || undefined)));
    }
  });

  $effect(() => {
    const text = content;
    const body = bodyOf(text);
    // Only the properties changed: the text of the note stays as it is, so
    // it is not rendered again.
    if (body === lastBody) return;
    lastBody = body;
    if (running) running.stale = true;
    const run = { stale: false };
    running = run;
    failed = false;
    void api.renderMarkdown(text).then(
      (r) => {
        if (!run.stale) rendered = r;
      },
      () => {
        if (!run.stale) failed = true;
      },
    );
  });

  /** The part of the note that is shown as text; the properties are shown by themselves. */
  function bodyOf(text: string): string {
    const fm = parseFrontmatter(text);
    return fm ? text.slice(fm.to + fm.eol.length) : text;
  }

  /** Wikilinks and links to other notes open in the app, everything else stays. */
  function handleClick(event: MouseEvent) {
    const link = (event.target as HTMLElement).closest("a");
    if (!link) return;
    const wikilink = link.getAttribute("data-wikilink");
    if (wikilink) {
      event.preventDefault();
      onopen(wikilink);
      return;
    }
    const href = link.getAttribute("href");
    if (!href) return;
    // Links to the web stay in the webview.
    if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith("//")) return;
    event.preventDefault();
    onopen(href.split("#")[0]);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="reading h-full overflow-y-auto px-8 pt-8 pb-32" onclick={handleClick}>
  {#if failed}
    <p class="py-8 text-sm text-red-400">The note could not be displayed.</p>
  {:else if rendered}
    <div class="reading-inner">
      <!-- Edits change only the lines of the property, the note is saved like any other edit. -->
      <Properties source={content} apply={(change) => onchange(applyChange(content, change))} {onopen} />
      <div class="prose" bind:this={prose}>
        <!-- The HTML is built in the Rust core; raw HTML from a note is escaped there. -->
        {@html rendered.html}
      </div>
    </div>
  {:else}
    <p class="py-8 text-sm text-zinc-600">Loading …</p>
  {/if}
</div>

<style>
  /* The rendered markdown is inserted as HTML, so these rules have to be global. */
  .reading :global(.reading-inner) {
    max-width: 48rem;
    margin: 0 auto;
  }
  .reading :global(.prose) {
    font-size: 1rem;
    line-height: 1.75;
    color: var(--color-zinc-200);
  }
  .reading :global(.prose h1),
  .reading :global(.prose h2),
  .reading :global(.prose h3),
  .reading :global(.prose h4) {
    margin: 1.6em 0 0.5em;
    font-weight: 650;
    line-height: 1.3;
    color: var(--color-zinc-100);
  }
  .reading :global(.prose h1) {
    font-size: 1.7em;
  }
  .reading :global(.prose h2) {
    font-size: 1.35em;
  }
  .reading :global(.prose h3) {
    font-size: 1.15em;
  }
  .reading :global(.prose h4) {
    font-size: 1em;
  }
  .reading :global(.prose p) {
    margin: 0 0 1em;
  }
  .reading :global(.prose ul),
  .reading :global(.prose ol) {
    margin: 0 0 1em;
    padding-left: 1.5em;
  }
  /* Tailwind's reset removes the markers of lists, so they are set again. */
  .reading :global(.prose ul) {
    list-style: disc;
  }
  .reading :global(.prose ol) {
    list-style: decimal;
  }
  .reading :global(.prose ul ul) {
    list-style: circle;
  }
  .reading :global(.prose ul ul ul) {
    list-style: square;
  }
  .reading :global(.prose li) {
    margin: 0.2em 0;
  }
  .reading :global(.prose li::marker) {
    color: var(--color-zinc-500);
  }
  .reading :global(.prose li > ul),
  .reading :global(.prose li > ol) {
    margin: 0.2em 0 0;
  }
  /* A task has its checkbox instead of a bullet; the box sits where the bullet would be. */
  .reading :global(.prose li:has(> input[type="checkbox"])) {
    list-style: none;
  }
  .reading :global(.prose li > input[type="checkbox"]) {
    margin: 0 0.45em 0 -1.4em;
  }
  .reading :global(.prose a) {
    color: var(--color-violet-300);
    text-decoration: none;
    border-bottom: 1px solid var(--color-violet-500);
  }
  .reading :global(.prose a:hover) {
    border-bottom-color: var(--color-violet-300);
  }
  .reading :global(.prose a.wikilink) {
    color: var(--color-violet-300);
    border-bottom-color: var(--color-violet-700);
  }
  .reading :global(.prose strong) {
    font-weight: 650;
    color: var(--color-zinc-100);
  }
  .reading :global(.prose blockquote) {
    margin: 0 0 1em;
    padding-left: 1em;
    border-left: 3px solid var(--color-zinc-700);
    color: var(--color-zinc-400);
  }
  .reading :global(.prose code) {
    font-family: var(--font-mono);
    font-size: 0.85em;
    padding: 0.12em 0.35em;
    border-radius: 4px;
    background: var(--color-zinc-800);
  }
  .reading :global(.prose pre) {
    position: relative;
    margin: 0 0 1em;
    padding: 0.9rem 1rem;
    border-radius: 0.5rem;
    background: var(--color-zinc-900);
    border: 1px solid var(--color-zinc-800);
    overflow-x: auto;
  }
  /* The language of the block, set by `highlightCodeBlocks`. */
  .reading :global(.prose pre[data-language]::before) {
    content: attr(data-language);
    position: absolute;
    top: 0.4rem;
    right: 0.7rem;
    font-family: var(--font-mono);
    font-size: 0.7rem;
    color: var(--color-zinc-500);
    text-transform: lowercase;
  }
  .reading :global(.prose pre code) {
    padding: 0;
    background: none;
    font-size: 0.85em;
    line-height: 1.6;
    color: var(--color-zinc-200);
  }
  .reading :global(.prose table) {
    width: 100%;
    margin: 0 0 1em;
    border-collapse: collapse;
    font-size: 0.92em;
  }
  .reading :global(.prose th),
  .reading :global(.prose td) {
    padding: 0.4em 0.7em;
    border: 1px solid var(--color-zinc-800);
    text-align: left;
  }
  .reading :global(.prose th) {
    background: var(--color-zinc-900);
    font-weight: 600;
  }
  .reading :global(.prose hr) {
    margin: 1.6em 0;
    border: none;
    border-top: 1px solid var(--color-zinc-800);
  }
  .reading :global(.prose img) {
    max-width: 100%;
    border-radius: 0.5rem;
  }
</style>
