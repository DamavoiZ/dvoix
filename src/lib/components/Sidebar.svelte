<script lang="ts">
  import FileTree from "./FileTree.svelte";
  import SearchPanel from "./SearchPanel.svelte";
  import TagsPanel from "./TagsPanel.svelte";
  import { vault } from "../vault.svelte";

  const tabs = [
    { id: "files", label: "Dateien" },
    { id: "search", label: "Suche" },
    { id: "tags", label: "Tags" },
  ] as const;
  let tab = $state<(typeof tabs)[number]["id"]>("files");
</script>

<aside class="flex w-72 shrink-0 flex-col border-r border-zinc-800 bg-zinc-900/40">
  <header class="flex items-center justify-between px-4 pt-4 pb-3">
    <div class="min-w-0">
      <div class="truncate font-semibold text-zinc-100">{vault.info?.name}</div>
      <div class="truncate text-xs text-zinc-500" title={vault.info?.path}>{vault.info?.path}</div>
    </div>
    <button class="text-xs text-zinc-500 hover:text-zinc-200" title="Anderen Vault öffnen" onclick={() => vault.close()}>⇄</button>
  </header>

  <div class="flex gap-1 px-3 pb-3">
    {#each tabs as t (t.id)}
      <button
        class="flex-1 rounded px-2 py-1 text-xs {tab === t.id ? 'bg-zinc-800 text-zinc-100' : 'text-zinc-400 hover:text-zinc-200'}"
        onclick={() => (tab = t.id)}>{t.label}</button
      >
    {/each}
  </div>

  {#if tab === "files"}
    <FileTree />
  {:else if tab === "search"}
    <SearchPanel />
  {:else}
    <TagsPanel />
  {/if}
</aside>
