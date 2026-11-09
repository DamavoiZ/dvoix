<script lang="ts">
  // All checkbox tasks of the vault, grouped by due date or by note.
  import { Flag } from "@lucide/svelte";
  import { api, type TaskItem } from "../../../api";
  import { vault } from "../../../vault.svelte";
  import {
    bucketLabels,
    compareTasks,
    dueBucket,
    formatDue,
    openTask,
    priorityClass,
    priorityLabels,
    today,
    toggleTask,
    type DueBucket,
  } from "../../../tasks";

  import { taskSettings, type TaskGroup, type TaskShow } from "./settings";

  let tasks = $state<TaskItem[]>([]);
  let loaded = $state(false);
  // Filter and grouping are remembered in `.dvoix/plugins/tasks/data.json`.
  const show = $derived(taskSettings.value.show);
  const group = $derived(taskSettings.value.group);
  function setShow(v: TaskShow) {
    taskSettings.value.show = v;
    taskSettings.save();
  }
  function setGroup(v: TaskGroup) {
    taskSettings.value.group = v;
    taskSettings.save();
  }
  let query = $state("");

  $effect(() => {
    void vault.revision;
    api
      .tasks()
      .then((t) => (tasks = t))
      .catch(() => (tasks = []))
      .finally(() => (loaded = true));
  });

  const now = $derived.by(() => {
    void vault.revision; // re-evaluate "today" now and then, e.g. after midnight
    return today();
  });

  const visible = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return tasks.filter((t) => {
      if (show === "open" && t.done) return false;
      if (show === "done" && !t.done) return false;
      if (!q) return true;
      return (
        t.text.toLowerCase().includes(q) || t.noteTitle.toLowerCase().includes(q) || t.tags.some((tag) => tag.includes(q.replace(/^#/, "")))
      );
    });
  });

  interface Section {
    key: string;
    label: string;
    tone?: string;
    path?: string;
    items: TaskItem[];
  }

  const sections = $derived.by((): Section[] => {
    if (group === "note") {
      const byNote = new Map<string, Section>();
      for (const t of visible) {
        let s = byNote.get(t.path);
        if (!s) byNote.set(t.path, (s = { key: t.path, label: t.noteTitle, path: t.path, items: [] }));
        s.items.push(t);
      }
      return [...byNote.values()];
    }
    // Finished tasks are not "overdue": they get their own section at the end.
    const order: (DueBucket | "done")[] = ["overdue", "today", "week", "later", "none", "done"];
    const buckets = new Map<DueBucket | "done", TaskItem[]>(order.map((b) => [b, []]));
    for (const t of visible) buckets.get(t.done ? "done" : dueBucket(t.due, now))!.push(t);
    return order
      .filter((b) => buckets.get(b)!.length)
      .map((b) => ({
        key: b,
        label: b === "done" ? "Done" : bucketLabels[b],
        tone: b === "overdue" ? "text-red-400" : b === "today" ? "text-violet-300" : undefined,
        items: buckets.get(b)!.sort(compareTasks),
      }));
  });

  const openCount = $derived(tasks.filter((t) => !t.done).length);

  function dueClass(t: TaskItem): string {
    if (t.done || !t.due) return "text-zinc-500";
    const b = dueBucket(t.due, now);
    return b === "overdue" ? "text-red-400" : b === "today" ? "text-violet-300" : "text-zinc-400";
  }

  const segment = (active: boolean) =>
    `rounded px-2 py-0.5 ${active ? "bg-zinc-700 text-zinc-100" : "text-zinc-400 hover:text-zinc-200"}`;
</script>

<div class="flex flex-col gap-2 px-3 pb-2 text-xs">
  <input
    class="w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1 text-sm outline-none placeholder:text-zinc-600 focus:border-violet-500"
    placeholder="Filter tasks …"
    bind:value={query}
  />
  <div class="flex items-center justify-between gap-2">
    <div class="flex rounded-md bg-zinc-800/60 p-0.5" role="group" aria-label="Show">
      <button class={segment(show === "open")} onclick={() => setShow("open")}>Open {openCount}</button>
      <button class={segment(show === "done")} onclick={() => setShow("done")}>Done</button>
      <button class={segment(show === "all")} onclick={() => setShow("all")}>All</button>
    </div>
    <div class="flex rounded-md bg-zinc-800/60 p-0.5" role="group" aria-label="Group">
      <button class={segment(group === "due")} title="Group by due date" onclick={() => setGroup("due")}>Date</button>
      <button class={segment(group === "note")} title="Group by note" onclick={() => setGroup("note")}>Note</button>
    </div>
  </div>
</div>

<div class="flex-1 overflow-y-auto px-1 pb-4">
  {#each sections as section (section.key)}
    <section class="mb-2">
      <h3 class="flex items-center justify-between px-2 pt-2 pb-1 text-xs font-medium {section.tone ?? 'text-zinc-400'}">
        {#if section.path}
          <button class="truncate hover:underline" onclick={() => vault.openFile(section.path!)}>{section.label}</button>
        {:else}
          <span>{section.label}</span>
        {/if}
        <span class="text-zinc-600">{section.items.length}</span>
      </h3>
      <ul>
        {#each section.items as task (`${task.path}:${task.line}`)}
          <li
            class="group flex items-start gap-2 rounded px-2 py-1 hover:bg-zinc-800/70"
            style:padding-left={group === "note" ? `${0.5 + Math.min(task.indent, 12) * 0.25}rem` : undefined}
          >
            <input
              type="checkbox"
              class="mt-1 shrink-0 cursor-pointer accent-violet-500"
              checked={task.done}
              aria-label={task.done ? "Mark as open" : "Mark as done"}
              onclick={(e) => {
                e.preventDefault();
                void toggleTask(task);
              }}
            />
            <button class="min-w-0 flex-1 text-left" onclick={() => openTask(task)}>
              <span class="block text-sm break-words {task.done ? 'text-zinc-500 line-through' : 'text-zinc-200'}">
                {task.text || "(no text)"}
              </span>
              {#if task.due || task.priority || group === "due"}
                <span class="mt-0.5 flex flex-wrap items-center gap-x-2 text-xs text-zinc-500">
                  {#if task.priority}
                    <span class="inline-flex items-center gap-0.5 {priorityClass[task.priority]}" title="Priority {priorityLabels[task.priority]}">
                      <Flag size={11} strokeWidth={2} />{priorityLabels[task.priority]}
                    </span>
                  {/if}
                  {#if task.due}
                    <span class={dueClass(task)} title={task.due}>{formatDue(task.due, now)}</span>
                  {/if}
                  {#if group === "due"}
                    <span class="truncate">{task.noteTitle}</span>
                  {/if}
                </span>
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    </section>
  {:else}
    {#if loaded}
      <div class="space-y-2 px-3 text-sm text-zinc-500">
        {#if tasks.length && query}
          <p>No task matches "{query}".</p>
        {:else if tasks.length && show === "open"}
          <p>All done. 🎉</p>
        {:else if tasks.length}
          <p>Nothing here.</p>
        {:else}
          <p>No tasks yet. Write in any note:</p>
          <pre class="rounded bg-zinc-950 p-2 text-xs text-zinc-300">- [ ] Write the offer 📅 2026-12-01
- [ ] Call back due:2026-11-20 !high
- [x] Done</pre>
          <p class="text-xs">Priority: <code>!high</code>, <code>!medium</code>, <code>!low</code> (or ⏫ 🔼 🔽).</p>
        {/if}
      </div>
    {/if}
  {/each}
</div>
