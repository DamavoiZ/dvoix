<script lang="ts">
  // The reading view: the note as it looks when it is read, not edited.
  // The HTML comes from the Rust core, so it is the same markdown the app
  // indexes and searches.
  import { api, type RenderedNote } from "../../api";
  import type { FileViewProps } from "../types";

  let { path, content, onchange, onopen }: FileViewProps = $props();

  let rendered = $state<RenderedNote | null>(null);
  let failed = $state(false);

  $effect(() => {
    const text = content;
    let stale = false;
    failed = false;
    void api.renderMarkdown(text).then(
      (r) => {
        if (!stale) rendered = r;
      },
      () => {
        if (!stale) failed = true;
      },
    );
    return () => {
      stale = true;
    };
  });

  /** The frontmatter as rows for the properties table. */
  const properties = $derived(
    Object.entries(rendered?.frontmatter ?? {}).map(([key, value]) => ({
      key,
      value: formatValue(value),
    })),
  );

  function formatValue(value: unknown): string {
    if (Array.isArray(value)) return value.map((v) => String(v)).join(", ");
    if (value && typeof value === "object") return JSON.stringify(value);
    return String(value);
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
    <p class="py-8 text-sm text-red-400">Die Notiz konnte nicht dargestellt werden.</p>
  {:else if rendered}
    <div class="reading-inner">
      {#if properties.length}
        <table class="properties">
          <tbody>
            {#each properties as row (row.key)}
              <tr>
                <th>{row.key}</th>
                <td>{row.value}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
      <div class="prose">
        <!-- The HTML is built in the Rust core; raw HTML from a note is escaped there. -->
        {@html rendered.html}
      </div>
    </div>
  {:else}
    <p class="py-8 text-sm text-zinc-600">Wird geladen …</p>
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
  .reading :global(.prose li) {
    margin: 0.2em 0;
  }
  .reading :global(.prose li > input[type="checkbox"]) {
    margin-right: 0.45em;
    accent-color: var(--color-violet-500);
    vertical-align: -0.1em;
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
    margin: 0 0 1em;
    padding: 0.9rem 1rem;
    border-radius: 0.5rem;
    background: var(--color-zinc-900);
    border: 1px solid var(--color-zinc-800);
    overflow-x: auto;
  }
  .reading :global(.prose pre code) {
    padding: 0;
    background: none;
    font-size: 0.85em;
    line-height: 1.6;
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

  .properties {
    width: 100%;
    margin: 0 0 1.5rem;
    font-size: 0.85rem;
    border-collapse: collapse;
    border: 1px solid var(--color-zinc-800);
    border-radius: 0.5rem;
    overflow: hidden;
  }
  .properties tr + tr {
    border-top: 1px solid var(--color-zinc-800);
  }
  .properties th {
    width: 1%;
    padding: 0.35em 0.9em 0.35em 0.7em;
    text-align: left;
    font-family: var(--font-mono);
    font-weight: 400;
    color: var(--color-violet-300);
    white-space: nowrap;
    vertical-align: top;
  }
  .properties td {
    padding: 0.35em 0.7em;
    color: var(--color-zinc-400);
  }
</style>
