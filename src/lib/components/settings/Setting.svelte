<script lang="ts">
  // One row in the settings dialog: name and description left, control right.
  import type { Snippet } from "svelte";

  let { name, description, children }: { name: string; description?: string; children: Snippet } = $props();
</script>

<div class="flex items-center justify-between gap-6 border-b border-zinc-800/70 py-3 last:border-b-0">
  <div class="min-w-0">
    <div class="text-sm text-zinc-100">{name}</div>
    {#if description}<div class="mt-0.5 text-xs text-zinc-500">{description}</div>{/if}
  </div>
  <div class="shrink-0">{@render children()}</div>
</div>
