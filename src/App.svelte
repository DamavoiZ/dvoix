<script lang="ts">
  import { onMount } from "svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import FilePane from "./lib/components/FilePane.svelte";
  import Welcome from "./lib/components/Welcome.svelte";
  import { vault } from "./lib/vault.svelte";

  let ready = $state(false);

  onMount(() => {
    vault.init().finally(() => (ready = true));
    // Save pending edits before the window closes.
    const beforeUnload = () => void vault.flush();
    window.addEventListener("beforeunload", beforeUnload);
    return () => window.removeEventListener("beforeunload", beforeUnload);
  });
</script>

{#if !ready}
  <div class="h-full"></div>
{:else if vault.info}
  <div class="flex h-full">
    <Sidebar />
    <FilePane />
  </div>
{:else}
  <Welcome />
{/if}
