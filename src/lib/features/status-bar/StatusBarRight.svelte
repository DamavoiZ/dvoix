<script lang="ts">
  import { editorStatus } from "../../editorStatus.svelte";
  import { fileTypeFor } from "../../filetypes";
  import { settings, type ViewMode } from "../../settings.svelte";
  import { vault } from "../../vault.svelte";

  const isMarkdown = $derived(
    vault.openPath ? fileTypeFor(vault.openPath)?.id === "markdown" : false,
  );

  const MODE_LABEL: Record<ViewMode, string> = {
    read: "Read",
    edit: "Live preview",
    source: "Source",
  };

  const MODE_ORDER: ViewMode[] = ["read", "edit", "source"];

  function cycleMode() {
    const idx = MODE_ORDER.indexOf(settings.app.viewMode);
    settings.app.viewMode = MODE_ORDER[(idx + 1) % MODE_ORDER.length];
    settings.saveApp();
  }
</script>

{#if isMarkdown}
  <div class="flex items-center gap-3">
    {#if editorStatus.cursor && settings.app.viewMode !== "read"}
      <span class="tabular-nums">
        Ln {editorStatus.cursor.line}, Col {editorStatus.cursor.col}
      </span>
    {/if}
    <button
      type="button"
      class="rounded px-1.5 py-0.5 text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
      title="Cycle view mode (Read / Live preview / Source)"
      onclick={cycleMode}
    >
      {MODE_LABEL[settings.app.viewMode]}
    </button>
  </div>
{/if}
