<script lang="ts">
  // Segmented Read / Edit / Source switcher in the file pane header for Markdown notes.
  import { BookOpen, Code, Pencil } from "@lucide/svelte";
  import { settings, type ViewMode } from "../../settings.svelte";

  let { path: _path }: { path: string } = $props();

  const mode = $derived(settings.app.viewMode);

  function setMode(next: ViewMode) {
    settings.app.viewMode = next;
    settings.saveApp();
  }

  const modeButton =
    "flex h-6 items-center gap-1 rounded px-1.5 text-[11px] font-medium transition-colors";
</script>

<div
  class="flex items-center gap-0.5 rounded-md border border-zinc-800 bg-zinc-950/80 p-0.5"
  role="group"
  aria-label="Note view mode"
>
  <button
    class="{modeButton} {mode === 'read' ? 'bg-zinc-800 text-zinc-100' : 'text-zinc-500 hover:text-zinc-200'}"
    title="Read (Ctrl+E)"
    aria-label="Read"
    aria-pressed={mode === "read"}
    onclick={() => setMode("read")}
  >
    <BookOpen size={13} strokeWidth={1.75} />
    <span class="hidden sm:inline">Read</span>
  </button>
  <button
    class="{modeButton} {mode === 'edit' ? 'bg-zinc-800 text-zinc-100' : 'text-zinc-500 hover:text-zinc-200'}"
    title="Edit (Ctrl+E)"
    aria-label="Edit"
    aria-pressed={mode === "edit"}
    onclick={() => setMode("edit")}
  >
    <Pencil size={13} strokeWidth={1.75} />
    <span class="hidden sm:inline">Edit</span>
  </button>
  <button
    class="{modeButton} {mode === 'source' ? 'bg-zinc-800 text-zinc-100' : 'text-zinc-500 hover:text-zinc-200'}"
    title="Source (Ctrl+Shift+E)"
    aria-label="Source"
    aria-pressed={mode === "source"}
    onclick={() => setMode("source")}
  >
    <Code size={13} strokeWidth={1.75} />
    <span class="hidden sm:inline">Source</span>
  </button>
</div>
