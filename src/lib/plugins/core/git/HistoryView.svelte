<script lang="ts">
  // The history of the repository in a tab: the last commits, each one with
  // the files it changed. Like the history view of the Obsidian plugin "Git".
  import { ChevronDown, ChevronRight, RefreshCw } from "@lucide/svelte";
  import { api, type GitCommit } from "../../../api";
  import { vault } from "../../../vault.svelte";
  import { gitState } from "./state.svelte";

  const LIMIT = 50;

  let commits = $state<GitCommit[]>([]);
  /** The files of the commits that were opened, by commit id. */
  let files = $state<Record<string, string[]>>({});
  let open = $state<string | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);

  async function load() {
    loading = true;
    try {
      commits = await api.gitLog(LIMIT);
      error = null;
    } catch (e) {
      commits = [];
      error = String(e).replace(/^Error: /, "");
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    // A change on disk makes a new commit, the history grows.
    void vault.revision;
    void load();
  });

  /** Opens a commit and loads the files it changed. */
  async function toggle(commit: GitCommit) {
    if (open === commit.id) {
      open = null;
      return;
    }
    open = commit.id;
    if (files[commit.id]) return;
    try {
      files = { ...files, [commit.id]: await api.gitCommitFiles(commit.id) };
    } catch {
      files = { ...files, [commit.id]: [] };
    }
  }

  /** The date of a commit, in the words of the system. */
  function formatDate(iso: string): string {
    const date = new Date(iso);
    if (Number.isNaN(date.getTime())) return iso;
    return date.toLocaleString("en-US", { dateStyle: "short", timeStyle: "short" });
  }
</script>

<div class="flex h-full flex-col">
  <header class="flex shrink-0 items-center gap-2 border-b border-zinc-800 px-4 py-2">
    <span class="text-xs font-medium tracking-wide text-zinc-500 uppercase">History</span>
    <button
      class="ml-auto flex h-6 w-6 items-center justify-center rounded text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
      title="Refresh"
      aria-label="Refresh"
      onclick={() => void load()}
    >
      <RefreshCw size={13} strokeWidth={1.75} class={loading ? "animate-spin" : ""} />
    </button>
  </header>

  <div class="min-h-0 flex-1 overflow-y-auto px-2 py-2">
    {#if error}
      <p class="px-2 py-2 text-sm text-red-300">{error}</p>
    {:else if !commits.length && !loading}
      <p class="px-2 py-2 text-sm text-zinc-500">No commits yet. The vault is not under version control yet.</p>
    {:else}
      <ul>
        {#each commits as commit (commit.id)}
          <li class="border-b border-zinc-800/60 last:border-b-0">
            <button
              class="flex w-full items-start gap-2 px-2 py-2 text-left hover:bg-zinc-800/70"
              onclick={() => void toggle(commit)}
            >
              {#if open === commit.id}
                <ChevronDown size={14} strokeWidth={1.75} class="mt-0.5 shrink-0 text-zinc-500" />
              {:else}
                <ChevronRight size={14} strokeWidth={1.75} class="mt-0.5 shrink-0 text-zinc-500" />
              {/if}
              <span class="min-w-0 flex-1">
                <span class="block truncate text-sm text-zinc-200">{commit.message}</span>
                <span class="mt-0.5 block text-xs text-zinc-500">
                  <span class="font-mono text-zinc-400">{commit.shortId}</span>
                  {commit.author} · {formatDate(commit.date)}
                </span>
              </span>
            </button>
            {#if open === commit.id}
              <ul class="pb-2 pl-8">
                {#each files[commit.id] ?? [] as file (file)}
                  <li>
                    <button
                      class="w-full truncate px-2 py-0.5 text-left font-mono text-xs text-zinc-400 hover:text-zinc-100 hover:underline"
                      title={file}
                      onclick={() => void gitState.showDiff(file, false, commit.id)}
                    >
                      {file}
                    </button>
                  </li>
                {:else}
                  <li class="px-2 py-0.5 text-xs text-zinc-600">No files in the vault changed.</li>
                {/each}
              </ul>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>
