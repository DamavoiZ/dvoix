<script lang="ts">
  // A note docked in the right sidebar, shown as it is read. The open note
  // comes from the editor, every other note from the file on disk.
  import { api, type RenderedNote } from "../../../api";
  import { vault } from "../../../vault.svelte";

  let { path }: { path: string } = $props();

  let rendered = $state<RenderedNote | null>(null);
  /** The text of the note, when it is not the open note. */
  let text = $state<string | null>(null);
  /** Whether the note is gone from the vault. */
  let missing = $state(false);
  /** The note that is shown, so a new one starts without the old text. */
  let shownPath = "";

  $effect(() => {
    const p = path;
    // A note in the vault changed: the file is read again.
    void vault.changes.notes.number;
    if (p !== shownPath) {
      shownPath = p;
      rendered = null;
      missing = false;
    }
    if (p === vault.openPath) return;
    let stale = false;
    void api.readFile(p).then(
      (content) => {
        if (stale) return;
        text = content;
        missing = false;
      },
      () => {
        if (stale) return;
        text = null;
        missing = true;
      },
    );
    return () => {
      stale = true;
    };
  });

  const content = $derived(path === vault.openPath ? vault.content : text);

  $effect(() => {
    const c = content;
    if (c === null) return;
    let stale = false;
    void api.renderMarkdown(c).then((r) => {
      if (!stale) rendered = r;
    });
    return () => {
      stale = true;
    };
  });

  /** A wikilink in the note opens its target in the main area. */
  function handleClick(event: MouseEvent) {
    const link = (event.target as HTMLElement).closest("a");
    const wikilink = link?.getAttribute("data-wikilink");
    if (!wikilink) return;
    event.preventDefault();
    void vault.openLink(wikilink);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="reading docked h-full overflow-y-auto px-3 py-3" onclick={handleClick}>
  {#if rendered}
    <div class="reading-inner">
      <div class="prose">
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
