<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { EditorView, lineNumbers } from "@codemirror/view";
  import type { Extension } from "@codemirror/state";
  import { BookOpen, Code, Pencil } from "@lucide/svelte";
  import type { FileViewProps } from "../types";
  import { createMarkdownEditor, revealLine, setDoc, setExtensions, setLivePreview } from "./editor";
  import ReadingView from "./ReadingView.svelte";
  import { reveal } from "../../reveal.svelte";
  import { plugins } from "../../plugins/manager.svelte";
  import { settings, type ViewMode } from "../../settings.svelte";

  let { path, content, onchange, onopen }: FileViewProps = $props();

  let host: HTMLDivElement | undefined = $state();
  let view: EditorView | undefined = $state();
  /** The text the editor last produced, so it is not put into it again. */
  let editorText = $state<string | undefined>();
  /** The heading the reading view should scroll to, e.g. from the outline. */
  let headingTarget = $state<string | null>(null);
  /** Whether the file uses Windows line endings; decided once per note. */
  let crlf = false;

  /** How the note is shown: reading view, live preview or markdown source. */
  const mode = $derived(settings.app.viewMode);

  /** Extensions from plugins (e.g. task checkboxes) and from the editor settings. */
  const extensions = $derived.by((): Extension[] => {
    void plugins.editorRevision;
    const out = [...plugins.editorExtensions()];
    if (settings.app.lineNumbers) out.push(lineNumbers());
    out.push(EditorView.contentAttributes.of({ spellcheck: settings.app.spellcheck ? "true" : "false" }));
    return out;
  });

  // The editor only exists while the note is edited; the reading view needs none.
  $effect(() => {
    if (mode === "read") {
      view?.destroy();
      view = undefined;
      return;
    }
    if (!host || view) return;
    const doc = untrack(() => content);
    crlf = doc.includes("\r\n");
    view = createMarkdownEditor({
      parent: host,
      doc,
      onchange: (doc) => {
        const text = withLineEndings(doc);
        editorText = text;
        onchange(text);
      },
      onlink: (target) => onopen(target),
      extensions,
    });
    view.focus();
    showRequestedLine();
  });

  // The live preview shows the syntax only on the line of the cursor.
  $effect(() => {
    const live = mode === "edit";
    if (view) setLivePreview(view, live);
  });

  // A plugin or setting changed: swap the extensions in place.
  $effect(() => {
    const ext = extensions;
    if (view) setExtensions(view, ext);
  });

  // The file was reloaded from disk (external edit): show the new content.
  // The text the editor itself produced is skipped, so a keystroke does not
  // copy the whole note twice.
  $effect(() => {
    const text = content;
    if (!view || text === editorText) return;
    setDoc(view, text);
  });

  // Another panel (e.g. tasks or the outline) asked to show a line of this file.
  $effect(() => {
    void reveal.target;
    showRequestedLine();
  });

  onMount(() => () => view?.destroy());

  /** Gives an edited note back the line endings of the file, so only the edited lines change on disk. */
  function withLineEndings(doc: string): string {
    return crlf ? doc.replace(/\n/g, "\r\n") : doc;
  }

  function showRequestedLine() {
    if (view) {
      const target = reveal.take(path);
      if (target) revealLine(view, target.line);
      return;
    }
    // Without an editor the reading view shows the note: it scrolls to the
    // heading instead of to the line.
    const heading = reveal.takeHeading(path);
    if (heading) headingTarget = heading;
  }

  function setMode(next: ViewMode) {
    settings.app.viewMode = next;
    settings.saveApp();
  }

  const modeButton =
    "rounded p-1 transition-colors hover:bg-zinc-800 hover:text-zinc-200";
</script>

<div class="relative h-full">
  {#if mode === "read"}
    <ReadingView {path} {content} {onchange} {onopen} heading={headingTarget} />
  {:else}
    <div class="h-full overflow-hidden" bind:this={host}></div>
  {/if}

  <!-- The three ways to show a note, like in Obsidian. -->
  <div class="pointer-events-none absolute top-1.5 right-3 z-10">
    <div
      class="pointer-events-auto flex items-center gap-0.5 rounded-lg border border-zinc-800 bg-zinc-900/90 p-0.5 shadow-lg"
    >
      <button
        class="{modeButton} {mode === 'read' ? 'bg-zinc-800 text-zinc-100' : 'text-zinc-500'}"
        title="Read (Ctrl+E)"
        aria-label="Lesen"
        aria-pressed={mode === "read"}
        onclick={() => setMode("read")}
      >
        <BookOpen size={14} />
      </button>
      <button
        class="{modeButton} {mode === 'edit' ? 'bg-zinc-800 text-zinc-100' : 'text-zinc-500'}"
        title="Edit (Ctrl+E)"
        aria-label="Edit"
        aria-pressed={mode === "edit"}
        onclick={() => setMode("edit")}
      >
        <Pencil size={14} />
      </button>
      <button
        class="{modeButton} {mode === 'source' ? 'bg-zinc-800 text-zinc-100' : 'text-zinc-500'}"
        title="Source (Ctrl+Shift+E)"
        aria-label="Source"
        aria-pressed={mode === "source"}
        onclick={() => setMode("source")}
      >
        <Code size={14} />
      </button>
    </div>
  </div>
</div>
