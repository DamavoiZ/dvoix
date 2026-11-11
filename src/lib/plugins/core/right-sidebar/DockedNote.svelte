<script lang="ts">
  // A note docked in the right sidebar, shown as it is read. The open note
  // comes from the editor, every other note from the file on disk.
  import { onMount } from "svelte";
  import { api, type RenderedNote } from "../../../api";
  import { embedImage, isImage, parseEmbed } from "../../../attachments";
  import { highlightCodeBlocks } from "../../../filetypes/markdown/codeHighlight";
  import { parseFrontmatter } from "../../../filetypes/markdown/frontmatter";
  import { vault } from "../../../vault.svelte";

  let { path }: { path: string } = $props();

  let rendered = $state<RenderedNote | null>(null);
  let prose = $state<HTMLElement | null>(null);
  /** The text of the note, when it is not the open note. */
  let text = $state<string | null>(null);
  /** Whether the note is gone from the vault. */
  let missing = $state(false);
  /** The note that is shown, so a new one starts without the old text. */
  let shownPath = "";
  /** The body of the last render that landed, so an unchanged one is not rendered again. */
  let lastBody: string | undefined;
  /** A fresh read of the file is only asked for once the changes have settled. */
  let readTimer: ReturnType<typeof setTimeout> | undefined;
  /** Counts the reads asked for; the answer to an older one is dropped. */
  let readGen = 0;
  onMount(() => () => {
    readGen++;
    clearTimeout(readTimer);
  });

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
    const p = path;
    const changed = vault.changes.notes;
    // Every path of the last batch: a batch names more than one file, and this
    // note may be any of them.
    void changed.number;
    const paths = changed.paths;
    const same = p === shownPath;
    shownPath = p;
    if (p === vault.openPath) {
      // While the note is open, its text comes from the editor, not from disk.
      readGen++;
      clearTimeout(readTimer);
      text = vault.content;
      missing = false;
      return;
    }
    // A batch that names neither this note nor a folder holding it leaves it
    // alone; re-reading on every change of the vault would cost a file read
    // per batch for every docked note. Nothing lives below a note, so a path
    // of the batch is either the note itself or one of its folders. A read
    // an earlier batch asked for stays pending: this run must not cancel it,
    // or an unrelated change right after a relevant one loses the re-read.
    if (same && paths.length && !paths.some((q) => q === p || p.startsWith(`${q}/`))) return;
    const gen = ++readGen;
    const load = () => {
      void api.readFile(p).then(
        (content) => {
          if (gen !== readGen) return;
          text = content;
          missing = false;
        },
        () => {
          if (gen !== readGen) return;
          text = null;
          missing = true;
        },
      );
    };
    clearTimeout(readTimer);
    if (same) {
      // A change on disk: wait for the burst to end before reading the file.
      readTimer = setTimeout(load, 200);
    } else {
      text = null;
      rendered = null;
      missing = false;
      lastBody = undefined;
      load();
    }
  });

  const content = $derived(path === vault.openPath ? vault.content : text);

  $effect(() => {
    const c = content;
    if (c === null) return;
    // Only the properties changed: the text stays as it is, so it is not
    // rendered again (like in the reading view).
    const body = bodyOf(c);
    if (body === lastBody) return;
    let stale = false;
    // While typing, render once the text has settled, not on every key press.
    // Text read from a file is rendered right away.
    const delay = path === vault.openPath && vault.dirty ? 200 : 0;
    const timer = setTimeout(() => {
      void api.renderMarkdown(c).then((r) => {
        if (stale) return;
        rendered = r;
        // Remembered only once the render has landed, not when it is asked
        // for: when the cleanup cancels a pending render (e.g. a tab switch
        // right after typing), the next run must render this body again
        // instead of skipping it as done, or the preview loses the last keys.
        lastBody = body;
      });
    }, delay);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  });

  /** The part of the note that is shown as text; the properties do not change it. */
  function bodyOf(text: string): string {
    const fm = parseFrontmatter(text);
    return fm ? text.slice(fm.to + fm.eol.length) : text;
  }

  /** Wikilinks and links to other notes open in the main area, everything else stays. */
  function handleClick(event: MouseEvent) {
    const link = (event.target as HTMLElement).closest("a");
    if (!link) return;
    const wikilink = link.getAttribute("data-wikilink");
    if (wikilink) {
      event.preventDefault();
      void vault.openLink(wikilink);
      return;
    }
    const href = link.getAttribute("href");
    if (!href) return;
    // Links to the web stay in the webview.
    if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith("//")) return;
    event.preventDefault();
    void vault.openLink(href.split("#")[0]);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="reading docked h-full overflow-y-auto px-3 py-3" onclick={handleClick}>
  {#if rendered}
    <div class="reading-inner">
      <div class="prose" bind:this={prose}>
        <!-- The HTML is built by the Rust core; raw HTML from a note is escaped there. -->
        {@html rendered.html}
      </div>
    </div>
  {:else if missing}
    <p class="py-4 text-xs text-zinc-600">This note is gone from the vault.</p>
  {:else}
    <p class="py-4 text-xs text-zinc-600">Loading …</p>
  {/if}
</div>
