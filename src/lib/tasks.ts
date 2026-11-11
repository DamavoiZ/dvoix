// Shared helpers for the task and project panels: dates, labels, toggling.
import { api, type Priority, type ProjectStatus, type TaskItem } from "./api";
import { vault } from "./vault.svelte";
import { reveal } from "./reveal.svelte";

/** Today as `YYYY-MM-DD` in local time (task dates have no time zone). */
export function today(): string {
  const d = new Date();
  return isoDate(d);
}

export function isoDate(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export function addDays(iso: string, days: number): string {
  const [y, m, d] = iso.split("-").map(Number);
  return isoDate(new Date(y, m - 1, d + days));
}

/** The day a week ahead, per "today": panels ask for every row, so it is kept. */
let weekFrom = "";
let weekEnd = "";
function weekAhead(now: string): string {
  if (now !== weekFrom) {
    weekFrom = now;
    weekEnd = addDays(now, 7);
  }
  return weekEnd;
}

export type DueBucket = "overdue" | "today" | "week" | "later" | "none";

export function dueBucket(due: string | null, now = today()): DueBucket {
  if (!due) return "none";
  if (due < now) return "overdue";
  if (due === now) return "today";
  if (due <= weekAhead(now)) return "week";
  return "later";
}

export const bucketLabels: Record<DueBucket, string> = {
  overdue: "Overdue",
  today: "Today",
  week: "Next 7 days",
  later: "Later",
  none: "No date",
};

/** One formatter per shape; building one per row costs more than the formatting. */
const dateShort = new Intl.DateTimeFormat("en-US", { day: "numeric", month: "short" });
const dateLong = new Intl.DateTimeFormat("en-US", { day: "numeric", month: "short", year: "numeric" });

/** The neighbours of "today", kept per day: panels ask for every row. */
let dueDay = "";
let dueTomorrow = "";
let dueYesterday = "";

/** Short date: "today", "tomorrow", "yesterday" or "3 Dec" (with year if not this year). */
export function formatDue(due: string, now = today()): string {
  if (due === now) return "today";
  if (now !== dueDay) {
    dueDay = now;
    dueTomorrow = addDays(now, 1);
    dueYesterday = addDays(now, -1);
  }
  if (due === dueTomorrow) return "tomorrow";
  if (due === dueYesterday) return "yesterday";
  const [y, m, d] = due.split("-").map(Number);
  const date = new Date(y, m - 1, d);
  const sameYear = due.slice(0, 4) === now.slice(0, 4);
  return (sameYear ? dateShort : dateLong).format(date);
}

export const priorityRank: Record<Priority, number> = { high: 0, medium: 1, low: 2 };

export const priorityLabels: Record<Priority, string> = { high: "High", medium: "Medium", low: "Low" };

export const priorityClass: Record<Priority, string> = {
  high: "text-red-400",
  medium: "text-amber-400",
  low: "text-sky-400",
};

export const statusLabels: Record<ProjectStatus, string> = {
  active: "Active",
  waiting: "Waiting",
  idea: "Idea",
  done: "Done",
  cancelled: "Cancelled",
};

export const statusClass: Record<ProjectStatus, string> = {
  active: "bg-emerald-500/15 text-emerald-300",
  waiting: "bg-amber-500/15 text-amber-300",
  idea: "bg-sky-500/15 text-sky-300",
  done: "bg-zinc-700/60 text-zinc-400",
  cancelled: "bg-zinc-800 text-zinc-500 line-through",
};

/** Sort: overdue/soonest first, undated last, then by priority. */
export function compareTasks(a: TaskItem, b: TaskItem): number {
  const ad = a.due ?? "9999-99-99";
  const bd = b.due ?? "9999-99-99";
  if (ad !== bd) return ad < bd ? -1 : 1;
  const ap = a.priority ? priorityRank[a.priority] : 3;
  const bp = b.priority ? priorityRank[b.priority] : 3;
  return ap - bp;
}

/**
 * Sets a task's checkbox to the state `done` in its file. Pending edits are
 * saved first so nothing is lost. True when the file was written; the watcher
 * reports the change and the panels refresh from it, so the vault is not
 * listed again here. The target state is passed explicitly: the caller may
 * flip its row optimistically before the write, so `task.done` is no longer
 * a safe source for it.
 */
export async function toggleTask(task: TaskItem, done: boolean): Promise<boolean> {
  try {
    await vault.flush();
    const entry = await api.setTaskDone(task.path, task.line, task.text, done);
    // Our own write: its watcher echo must not read the file back into the
    // editor or make a panel undo the tick before the reload arrives.
    vault.noteSelfWrite(entry);
    return true;
  } catch (e) {
    vault.error = String(e);
    return false;
  }
}

/** Opens the note of a task and scrolls the editor to its line. */
export async function openTask(task: { path: string; line: number }): Promise<void> {
  await vault.openFile(task.path);
  reveal.show(task.path, task.line);
}
