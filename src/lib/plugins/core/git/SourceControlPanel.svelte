<script lang="ts">
  // Source control of the vault: what changed, what is staged and the buttons
  // to stage, commit, push and pull. Like the view of the same name in the
  // Obsidian plugin "Git".
  import { Download, FileDiff, GitBranchPlus, Plus, RefreshCw, Trash, Undo2, Upload, X } from "@lucide/svelte";
  import { vault } from "../../../vault.svelte";
  import { gitSettings } from "./settings";
  import { gitState } from "./state.svelte";

  const repo = $derived(gitState.repo);
  const changes = $derived(gitState.status);
  const untracked = $derived(gitSettings.value.showUntracked ? (changes?.untracked ?? []) : []);

  let newBranchOpen = $state(false);
  let newBranch = $state("");
  /** The branch in the select; it follows the branch that is checked out. */
  let selected = $state("");

  $effect(() => {
    selected = repo?.branch ?? "";
  });

  // A change on disk changes the state of the repository.
  $effect(() => {
    void vault.revision;
    void gitState.refresh();
  });

  /** The letter of a change and the color it gets. */
  function letter(code: string): string {
    switch (code) {
      case "A":
        return "text-green-400";
      case "D":
        return "text-red-400";
      case "M":
        return "text-amber-400";
      case "R":
        return "text-sky-400";
      default:
        return "text-zinc-500";
    }
  }

  const iconBtn =
    "flex h-6 w-6 items-center justify-center rounded text-zinc-600 hover:bg-zinc-800 hover:text-zinc-200";

  async function createBranch() {
    const name = newBranch.trim();
    if (!name) return;
    newBranch = "";
    newBranchOpen = false;
    await gitState.createBranch(name);
  }
</script>

<div class="flex h-full flex-col">
  {#if !repo}
    <div class="space-y-3 px-3 py-4 text-sm text-zinc-500">
      <p>Der Vault ist kein Git-Repository.</p>
      <button
        class="rounded-md bg-violet-600 px-2.5 py-1.5 text-xs font-medium text-white hover:bg-violet-500"
        onclick={() => void gitState.init()}
      >
        Repository anlegen
      </button>
      <p class="text-xs">Alle Dateien des Vaults liegen dann in einem Repository und lassen sich versionieren.</p>
    </div>
  {:else}
    <!-- branch and remote -->
    <div class="flex items-center gap-1 border-b border-zinc-800 px-2 py-2">
      <select
        class="min-w-0 flex-1 truncate rounded-md border border-zinc-700 bg-zinc-950 px-1.5 py-1 text-xs outline-none focus:border-violet-500"
        title="Branch wechseln"
        bind:value={selected}
        onchange={(e) => void gitState.checkout(e.currentTarget.value)}
      >
        {#if !gitState.branches.some((b) => b.name === repo.branch)}
          <option value={repo.branch}>{repo.branch}</option>
        {/if}
        {#each gitState.branches as branch (branch.name)}
          <option value={branch.name}>{branch.name}</option>
        {/each}
      </select>
      <button class={iconBtn} title="Neuer Branch" aria-label="Neuer Branch" onclick={() => (newBranchOpen = !newBranchOpen)}>
        <GitBranchPlus size={14} strokeWidth={1.75} />
      </button>
      <button class={iconBtn} title="Pushen" aria-label="Pushen" onclick={() => void gitState.push()}>
        <Upload size={14} strokeWidth={1.75} />
      </button>
      <button class={iconBtn} title="Ziehen" aria-label="Ziehen" onclick={() => void gitState.pull()}>
        <Download size={14} strokeWidth={1.75} />
      </button>
      <button class={iconBtn} title="Aktualisieren" aria-label="Aktualisieren" onclick={() => void gitState.refresh()}>
        <RefreshCw size={14} strokeWidth={1.75} class={gitState.busy ? "animate-spin" : ""} />
      </button>
    </div>

    {#if newBranchOpen}
      <div class="flex gap-1 border-b border-zinc-800 px-2 py-2">
        <input
          class="min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1 text-xs outline-none placeholder:text-zinc-600 focus:border-violet-500"
          placeholder="Name des Branches"
          bind:value={newBranch}
          onkeydown={(e) => {
            if (e.key === "Enter") void createBranch();
            if (e.key === "Escape") newBranchOpen = false;
          }}
        />
        <button
          class="rounded-md bg-violet-600 px-2 py-1 text-xs font-medium text-white hover:bg-violet-500"
          onclick={() => void createBranch()}
        >
          Anlegen
        </button>
      </div>
    {/if}

    {#if gitState.error}
      <div class="mx-2 mt-2 flex items-start gap-2 rounded-md border border-red-900/60 bg-red-950/40 px-2 py-1.5 text-xs text-red-300">
        <span class="min-w-0 flex-1 break-words">{gitState.error}</span>
        <button class="shrink-0 text-red-400 hover:text-red-200" aria-label="Fehler schließen" onclick={() => (gitState.error = null)}>
          <X size={12} strokeWidth={2} />
        </button>
      </div>
    {/if}

    <!-- commit -->
    <div class="space-y-1.5 border-b border-zinc-800 p-2">
      <textarea
        class="w-full resize-none rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm outline-none placeholder:text-zinc-600 focus:border-violet-500"
        rows="2"
        placeholder="Nachricht für den Commit …"
        value={gitState.message}
        oninput={(e) => (gitState.message = e.currentTarget.value)}
      ></textarea>
      <div class="flex items-center gap-2">
        <button
          class="rounded-md bg-violet-600 px-2.5 py-1 text-xs font-medium text-white hover:bg-violet-500 disabled:opacity-50"
          disabled={!gitState.hasChanges || gitState.busy}
          onclick={() => void gitState.commit()}
        >
          Committen
        </button>
        {#if (changes?.unstaged.length ?? 0) > 0 || untracked.length > 0}
          <button
            class="rounded-md px-2 py-1 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
            onclick={() => void gitState.stageAll()}
          >
            Alle stagen
          </button>
        {/if}
        {#if repo.ahead > 0 || repo.behind > 0}
          <span class="ml-auto text-[10px] text-zinc-500">{repo.ahead}↑ {repo.behind}↓</span>
        {/if}
      </div>
    </div>

    <!-- the files -->
    <div class="min-h-0 flex-1 overflow-y-auto pb-4">
      {#if (changes?.staged.length ?? 0) > 0}
        <h3 class="px-3 pt-2 pb-1 text-xs font-medium text-zinc-400">
          Gestaged <span class="text-zinc-600">{changes!.staged.length}</span>
        </h3>
        <ul>
          {#each changes!.staged as file (file.path)}
            <li class="group flex items-center gap-1 px-2 py-0.5 hover:bg-zinc-800/70">
              <span class="w-3 shrink-0 text-center font-mono text-xs {letter(file.status)}">{file.status}</span>
              <button
                class="min-w-0 flex-1 truncate text-left text-xs text-zinc-300 hover:text-zinc-100"
                title={file.path}
                onclick={() => void gitState.showDiff(file.path, true)}
              >
                {file.path}
              </button>
              <button
                class={iconBtn}
                title="Aus dem Index nehmen"
                aria-label="Aus dem Index nehmen"
                onclick={() => void gitState.unstage([file.path])}
              >
                <Undo2 size={13} strokeWidth={1.75} />
              </button>
            </li>
          {/each}
        </ul>
      {/if}

      {#if (changes?.unstaged.length ?? 0) > 0}
        <h3 class="px-3 pt-2 pb-1 text-xs font-medium text-zinc-400">
          Geändert <span class="text-zinc-600">{changes!.unstaged.length}</span>
        </h3>
        <ul>
          {#each changes!.unstaged as file (file.path)}
            <li class="group flex items-center gap-1 px-2 py-0.5 hover:bg-zinc-800/70">
              <span class="w-3 shrink-0 text-center font-mono text-xs {letter(file.status)}">{file.status}</span>
              <button
                class="min-w-0 flex-1 truncate text-left text-xs text-zinc-300 hover:text-zinc-100"
                title={file.path}
                onclick={() => void gitState.showDiff(file.path, false)}
              >
                {file.path}
              </button>
              <button
                class={iconBtn}
                title="Änderungen ansehen"
                aria-label="Änderungen ansehen"
                onclick={() => void gitState.showDiff(file.path, false)}
              >
                <FileDiff size={13} strokeWidth={1.75} />
              </button>
              <button
                class={iconBtn}
                title="Stagen"
                aria-label="Stagen"
                onclick={() => void gitState.stage([file.path])}
              >
                <Plus size={13} strokeWidth={1.75} />
              </button>
              <button
                class={iconBtn}
                title="Änderungen verwerfen"
                aria-label="Änderungen verwerfen"
                onclick={() => void gitState.discard(file.path, false)}
              >
                <Trash size={13} strokeWidth={1.75} />
              </button>
            </li>
          {/each}
        </ul>
      {/if}

      {#if untracked.length > 0}
        <h3 class="px-3 pt-2 pb-1 text-xs font-medium text-zinc-400">
          Unversioniert <span class="text-zinc-600">{untracked.length}</span>
        </h3>
        <ul>
          {#each untracked as file (file.path)}
            <li class="group flex items-center gap-1 px-2 py-0.5 hover:bg-zinc-800/70">
              <span class="w-3 shrink-0 text-center font-mono text-xs text-zinc-500">?</span>
              <button
                class="min-w-0 flex-1 truncate text-left text-xs text-zinc-300 hover:text-zinc-100"
                title={file.path}
                onclick={() => void gitState.showDiff(file.path, false)}
              >
                {file.path}
              </button>
              <button
                class={iconBtn}
                title="Stagen"
                aria-label="Stagen"
                onclick={() => void gitState.stage([file.path])}
              >
                <Plus size={13} strokeWidth={1.75} />
              </button>
              <button
                class={iconBtn}
                title="Datei löschen"
                aria-label="Datei löschen"
                onclick={() => void gitState.discard(file.path, true)}
              >
                <Trash size={13} strokeWidth={1.75} />
              </button>
            </li>
          {/each}
        </ul>
      {/if}

      {#if !gitState.hasChanges}
        <p class="px-3 pt-3 text-xs text-zinc-500">Keine Änderungen. Der Vault ist auf dem Stand des letzten Commits.</p>
      {/if}
    </div>
  {/if}
</div>
