<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { EditorView, lineNumbers } from "@codemirror/view";
  import type { Extension } from "@codemirror/state";
  import type { FileViewProps } from "../types";
  import { createMarkdownEditor, revealLine, setDoc, setExtensions, setLivePreview } from "./editor";
  import ReadingView from "./ReadingView.svelte";
  import { editorStatus } from "../../editorStatus.svelte";
  import { reveal } from "../../reveal.svelte";
  import { plugins } from "../../plugins/manager.svelte";
  import { settings } from "../../settings.svelte";

  let { path, content, onchange, onopen }: FileViewProps = $props();

  let host: HTMLDivElement | undefined = $state();
  let view: EditorView | undefined = $state();
  /** The text the editor last produced, so it is not put into it again. */
  let editorText = $state<string | undefined>();
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
      editorStatus.clear();
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

  onMount(() => () => {
    view?.destroy();
    editorStatus.clear();
  });

  /** Gives an edited note back the line endings of the file, so only the edited lines change on disk. */
  function withLineEndings(doc: string): string {
    return crlf ? doc.replace(/\n/g, "\r\n") : doc;
  }

  function showRequestedLine() {
    if (!view) return;
    const target = reveal.take(path);
    if (target) revealLine(view, target.line);
  }
</script>

<div class="relative h-full">
  {#if mode === "read"}
    <ReadingView {path} {content} {onchange} {onopen} />
  {:else}
    <div class="h-full overflow-hidden" bind:this={host}></div>
  {/if}
</div>
