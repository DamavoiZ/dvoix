<script lang="ts">
  import { vault } from "../vault.svelte";
</script>

<main class="flex h-full items-center justify-center">
  <div class="max-w-md space-y-6 text-center">
    <h1 class="text-4xl font-bold tracking-tight text-zinc-100">dvoix</h1>
    <p class="text-zinc-400">
      Notizen, Projekte und Tracking als ganz normale Dateien auf deinem PC. Wähle einen Ordner als Vault, alles darin
      bleibt lesbar, auch ohne dvoix.
    </p>
    <button class="rounded-lg bg-violet-600 px-5 py-2.5 font-medium text-white hover:bg-violet-500" onclick={() => vault.pick()}>
      Vault-Ordner öffnen
    </button>
    {#if vault.error}<p class="text-sm text-red-400">{vault.error}</p>{/if}
  </div>
</main>
