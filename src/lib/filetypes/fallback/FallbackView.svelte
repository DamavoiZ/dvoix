<script lang="ts">
  import type { FileViewProps } from "../types";

  let { path, content }: FileViewProps = $props();
</script>

<div class="flex h-full flex-col gap-3 p-8 text-sm text-zinc-400">
  <p>
    There is no view for <code class="text-zinc-200">{path}</code>. Is the matching plugin perhaps turned off in the
    settings?
  </p>
  <pre class="overflow-auto rounded bg-zinc-900 p-4 font-mono text-xs text-zinc-300">{content.slice(0, 20000)}</pre>
</div>
