<script lang="ts">
  import { onMount } from "svelte";
  import { EditorView, lineNumbers } from "@codemirror/view";
  import type { Extension } from "@codemirror/state";
  import type { FileViewProps } from "../types";
  import { createMarkdownEditor, revealLine, setDoc, setExtensions } from "./editor";
  import { reveal } from "../../reveal.svelte";
  import { plugins } from "../../plugins/manager.svelte";
  import { settings } from "../../settings.svelte";

  let { path, content, onchange, onopen }: FileViewProps = $props();

  let host: HTMLDivElement;
  let view: EditorView | undefined = $state();

  /** Extensions from plugins (e.g. task checkboxes) and from the editor settings. */
  const extensions = $derived.by((): Extension[] => {
    void plugins.editorRevision;
    const out = [...plugins.editorExtensions()];
    if (settings.app.lineNumbers) out.push(lineNumbers());
    out.push(EditorView.contentAttributes.of({ spellcheck: settings.app.spellcheck ? "true" : "false" }));
    return out;
  });

  onMount(() => {
    view = createMarkdownEditor({
      parent: host,
      doc: content,
      onchange: (doc) => onchange(doc),
      onlink: (target) => onopen(target),
      extensions,
    });
    view.focus();
    showRequestedLine();
    return () => view?.destroy();
  });

  // A plugin or setting changed: swap the extensions in place.
  $effect(() => {
    const ext = extensions;
    if (view) setExtensions(view, ext);
  });

  // The file was reloaded from disk (external edit): show the new content.
  $effect(() => {
    if (view) setDoc(view, content);
  });

  // Another panel (e.g. tasks) asked to show a line of this file.
  $effect(() => {
    void reveal.target;
    showRequestedLine();
  });

  function showRequestedLine() {
    if (!view) return;
    const line = reveal.take(path);
    if (line !== null) revealLine(view, line);
  }
</script>

<div class="h-full overflow-hidden" bind:this={host}></div>
