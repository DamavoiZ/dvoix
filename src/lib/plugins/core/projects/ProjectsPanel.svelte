<script lang="ts">
  // Notes with `type: projekt` in their frontmatter, with status, due date and task progress.
  import { Plus } from "@lucide/svelte";
  import { api, type ProjectStatus, type ProjectSummary } from "../../../api";
  import { vault } from "../../../vault.svelte";
  import { dueBucket, formatDue, isoDate, priorityClass, priorityLabels, statusClass, statusLabels, today } from "../../../tasks";

  type Show = "running" | "closed" | "all";

  let projects = $state<ProjectSummary[]>([]);
  let loaded = $state(false);
  let show = $state<Show>("running");

  $effect(() => {
    void vault.revision;
    api
      .projects()
      .then((p) => (projects = p))
      .catch(() => (projects = []))
      .finally(() => (loaded = true));
  });

  const now = $derived.by(() => {
    void vault.revision;
    return today();
  });

  const statusOrder: ProjectStatus[] = ["active", "waiting", "idea", "done", "cancelled"];
  const closed = (s: ProjectStatus) => s === "done" || s === "cancelled";

  const visible = $derived(
    projects
      .filter((p) => (show === "all" ? true : show === "closed" ? closed(p.status) : !closed(p.status)))
      .sort((a, b) => {
        const s = statusOrder.indexOf(a.status) - statusOrder.indexOf(b.status);
        if (s) return s;
        const ad = a.due ?? "9999";
        const bd = b.due ?? "9999";
        return ad < bd ? -1 : ad > bd ? 1 : a.title.localeCompare(b.title, "de");
      }),
  );

  const runningCount = $derived(projects.filter((p) => !closed(p.status)).length);

  function dueTone(due: string, status: ProjectStatus): string {
    if (closed(status)) return "text-zinc-500";
    const b = dueBucket(due, now);
    return b === "overdue" ? "text-red-400" : b === "today" ? "text-violet-300" : "text-zinc-400";
  }

  /** Creates "Neues Projekt.md" (in `Projekte/` if that folder exists) with the project frontmatter. */
  async function createProject() {
    const folder = vault.entries.find((e) => e.isDir && /^projekte?$/i.test(e.path))?.path;
    const prefix = folder ? `${folder}/` : "";
    const taken = new Set(vault.entries.map((e) => e.path.toLowerCase()));
    let name = "Neues Projekt";
    for (let n = 2; taken.has(`${prefix}${name}.md`.toLowerCase()); n++) name = `Neues Projekt ${n}`;
    const path = `${prefix}${name}.md`;
    const content = `---\ntype: projekt\nstatus: aktiv\nstart: ${isoDate(new Date())}\ndue:\npriority:\narea:\n---\n# ${name}\n\n## Ziel\n\n\n## Aufgaben\n\n- [ ] \n`;
    try {
      await vault.flush();
      await api.createFile(path, content);
      await vault.refresh();
      await vault.openFile(path);
    } catch (e) {
      vault.error = String(e);
    }
  }

  const segment = (active: boolean) =>
    `rounded px-2 py-0.5 ${active ? "bg-zinc-700 text-zinc-100" : "text-zinc-400 hover:text-zinc-200"}`;
</script>

<div class="flex items-center justify-between gap-2 px-3 pb-2 text-xs">
  <div class="flex rounded-md bg-zinc-800/60 p-0.5" role="group" aria-label="Anzeigen">
    <button class={segment(show === "running")} onclick={() => (show = "running")}>Laufend {runningCount}</button>
    <button class={segment(show === "closed")} onclick={() => (show = "closed")}>Abgeschlossen</button>
    <button class={segment(show === "all")} onclick={() => (show = "all")}>Alle</button>
  </div>
  <button
    class="flex h-6 w-6 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
    title="Neues Projekt"
    aria-label="Neues Projekt"
    onclick={createProject}
  >
    <Plus size={16} />
  </button>
</div>

<div class="flex-1 space-y-1 overflow-y-auto px-2 pb-4">
  {#each visible as p (p.path)}
    {@const total = p.openTasks + p.doneTasks}
    <button
      class="block w-full rounded-md px-2 py-2 text-left hover:bg-zinc-800/70 {vault.openPath === p.path ? 'bg-zinc-800/70' : ''}"
      onclick={() => vault.openFile(p.path)}
    >
      <div class="flex items-center gap-2">
        <span class="min-w-0 flex-1 truncate text-sm text-zinc-200">{p.title}</span>
        <span class="shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium {statusClass[p.status]}">{statusLabels[p.status]}</span>
      </div>
      <div class="mt-1 flex flex-wrap gap-x-2 text-xs text-zinc-500">
        {#if p.area}<span>{p.area}</span>{/if}
        {#if p.priority}<span class={priorityClass[p.priority]}>{priorityLabels[p.priority]}</span>{/if}
        {#if p.due}<span class={dueTone(p.due, p.status)} title="Fällig am {p.due}">fällig {formatDue(p.due, now)}</span>{/if}
        {#if p.nextDue && !closed(p.status)}
          <span class={dueTone(p.nextDue, p.status)} title="Nächste offene Aufgabe">nächste Aufgabe {formatDue(p.nextDue, now)}</span>
        {/if}
      </div>
      {#if total}
        <div class="mt-1.5 flex items-center gap-2">
          <div class="h-1 flex-1 overflow-hidden rounded-full bg-zinc-800">
            <div class="h-full rounded-full bg-violet-500" style:width="{(p.doneTasks / total) * 100}%"></div>
          </div>
          <span class="text-[10px] text-zinc-500">{p.doneTasks}/{total}</span>
        </div>
      {/if}
    </button>
  {:else}
    {#if loaded}
      <div class="space-y-2 px-1 text-sm text-zinc-500">
        {#if projects.length}
          <p>Keine Projekte in dieser Ansicht.</p>
        {:else}
          <p>Noch keine Projekte. Ein Projekt ist eine Notiz mit diesem Frontmatter:</p>
          <pre class="rounded bg-zinc-950 p-2 text-xs text-zinc-300">---
type: projekt
status: aktiv
due: 2026-12-01
---</pre>
          <p class="text-xs">Status: aktiv, wartend, idee, erledigt, abgebrochen. Die Checkboxen in der Notiz sind die Aufgaben des Projekts.</p>
          <button class="text-xs text-violet-300 hover:underline" onclick={createProject}>Neues Projekt anlegen</button>
        {/if}
      </div>
    {/if}
  {/each}
</div>
