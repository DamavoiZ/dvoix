<script lang="ts">
  import { onMount } from "svelte";
  import type { EditorView } from "@codemirror/view";
  import type { FileViewProps } from "../types";
  import { createMarkdownEditor, revealLine, setDoc } from "./editor";
  import { reveal } from "../../reveal.svelte";

  let { path, content, onchange, onopen }: FileViewProps = $props();

  let host: HTMLDivElement;
  let view: EditorView | undefined;

  onMount(() => {
    view = createMarkdownEditor({
      parent: host,
      doc: content,
      onchange: (doc) => onchange(doc),
      onlink: (target) => onopen(target),
    });
    view.focus();
    showRequestedLine();
    return () => view?.destroy();
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
