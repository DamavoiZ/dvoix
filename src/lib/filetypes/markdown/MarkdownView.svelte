<script lang="ts">
  import { onMount } from "svelte";
  import type { EditorView } from "@codemirror/view";
  import type { FileViewProps } from "../types";
  import { createMarkdownEditor, setDoc } from "./editor";

  let { content, onchange, onopen }: FileViewProps = $props();

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
    return () => view?.destroy();
  });

  // The file was reloaded from disk (external edit): show the new content.
  $effect(() => {
    if (view) setDoc(view, content);
  });
</script>

<div class="h-full overflow-hidden" bind:this={host}></div>
