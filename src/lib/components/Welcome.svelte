<script lang="ts">
  import { FolderOpen } from "@lucide/svelte";
  import { vault } from "../vault.svelte";
</script>

<main class="flex h-full items-center justify-center px-6">
  <div class="w-full max-w-md rounded-2xl border border-zinc-800/80 bg-zinc-900/40 p-8 text-center shadow-2xl">
    <div class="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-xl border border-violet-500/30 bg-violet-500/10 text-violet-300">
      <FolderOpen size={22} strokeWidth={1.75} />
    </div>
    <h1 class="text-3xl font-bold tracking-tight text-zinc-100">dvoix</h1>
    <p class="mt-2.5 text-sm leading-relaxed text-zinc-400">
      Notes, projects and tracking as ordinary files on your computer. Pick a folder as the vault—everything in it
      stays readable, even without dvoix.
    </p>
    <button
      class="mt-6 inline-flex items-center gap-2 rounded-lg bg-violet-600 px-5 py-2.5 text-sm font-medium text-white shadow-sm transition-colors hover:bg-violet-500"
      onclick={() => vault.pick()}
    >
      <FolderOpen size={16} strokeWidth={1.75} />
      Open vault folder
    </button>
    {#if vault.error}<p class="mt-4 text-sm text-red-400">{vault.error}</p>{/if}
  </div>
</main>
