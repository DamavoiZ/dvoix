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
  import { reveal } from "../../reveal.svelte";

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

  /**
   * Wikilinks and links to other notes open in the app, everything else stays.
   */
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

  // The outline asked for a heading: scroll to the one with that position or text.
  $effect(() => {
    void reveal.target;
    if (!prose) return;
    const heading = reveal.takeHeading(path);
    if (!heading) return;
    const all = [...prose.querySelectorAll("h1,h2,h3,h4,h5,h6")];
    const byIndex = heading.index !== undefined ? all[heading.index] : undefined;
    const target =
      byIndex?.textContent?.trim() === heading.text
        ? byIndex
        : all.find((h) => h.textContent?.trim() === heading.text);
    target?.scrollIntoView({ block: "start" });
  });
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

