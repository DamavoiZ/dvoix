// State and actions of the git plugin: the state of the repository, the last
// error and the commands that the panel, the commands and the history share.
import { api, type GitBranch, type GitFileDiff, type GitStatus } from "../../../api";
import { gitSettings } from "./settings";

/** The message of a failed git command. */
function reason(e: unknown): string {
  if (typeof e === "string") return e;
  const text = String(e);
  return text.startsWith("Error: ") ? text.slice(7) : text;
}

class GitState {
  /** The state of the repository, `null` while it is not loaded. */
  status = $state<GitStatus | null>(null);
  /** The branches of the repository. */
  branches = $state<GitBranch[]>([]);
  /** The last error, in the words of git. */
  error = $state<string | null>(null);
  /** True while a git command runs. */
  busy = $state(false);
  /** The message for the next commit. */
  message = $state("");
  /** The diff that is shown over the app, if any: both versions of the file. */
  diff = $state<({ title: string } & GitFileDiff) | null>(null);

  private timer: ReturnType<typeof setInterval> | undefined;
  private refreshTimer: ReturnType<typeof setTimeout> | undefined;
  /** The refresh that is running right now, so a second one joins it instead of racing it. */
  private refreshing: Promise<void> | null = null;
  /** A refresh was asked for while one was running: run once more when it ends. */
  private refreshAgain = false;
  private onVisible: (() => void) | undefined;
  /** Panels that show the state right now; the interval rests while none does. */
  private viewers = 0;

  /** The repository of the vault, `null` when there is none. */
  readonly repo = $derived(this.status?.repo ?? null);
  /** True when something is staged. */
  readonly hasStaged = $derived((this.status?.staged.length ?? 0) > 0);
  /** True when there is anything to commit at all. */
  readonly hasChanges = $derived(
    (this.status?.staged.length ?? 0) +
      (this.status?.unstaged.length ?? 0) +
      (this.status?.untracked.length ?? 0) >
      0,
  );

  /** A panel that shows the state mounted: catch up, then keep the interval running. */
  viewOpened() {
    this.viewers++;
    if (this.viewers === 1) void this.refresh();
  }

  /** The panel went away: the interval may rest now. */
  viewClosed() {
    this.viewers = Math.max(0, this.viewers - 1);
  }

  /** True while polling makes sense: a panel shows the state and the window is looked at. */
  private get watched(): boolean {
    return this.viewers > 0 && !document.hidden;
  }

  /** Loads the state of the repository and the list of branches, shortly after the last request. */
  scheduleRefresh(delay = 300) {
    clearTimeout(this.refreshTimer);
    this.refreshTimer = setTimeout(() => void this.refresh(), delay);
  }

  /**
   * Loads the state of the repository and the list of branches. Only one
   * load runs at a time: requests that come in while it runs are collected
   * into one more load afterwards, so a burst of changes does not stack
   * `git status` calls.
   */
  refresh(): Promise<void> {
    if (this.refreshing) {
      this.refreshAgain = true;
      return this.refreshing;
    }
    this.refreshing = this.loadStatus().finally(() => {
      this.refreshing = null;
      if (this.refreshAgain) {
        this.refreshAgain = false;
        void this.refresh();
      }
    });
    return this.refreshing;
  }

  private async loadStatus() {
    try {
      this.status = await api.gitStatus();
      this.error = null;
      this.branches = this.status.repo ? await api.gitBranches().catch(() => this.branches) : [];
    } catch (e) {
      this.status = null;
      this.error = reason(e);
    }
  }

  /** Runs a git command, shows its error and loads the state again. */
  private async run(fn: () => Promise<void>) {
    this.busy = true;
    try {
      await fn();
      this.error = null;
    } catch (e) {
      this.error = reason(e);
    } finally {
      this.busy = false;
      await this.refresh();
    }
  }

  stage(paths: string[]) {
    return this.run(async () => {
      await api.gitStage(paths);
    });
  }

  unstage(paths: string[]) {
    return this.run(async () => {
      await api.gitUnstage(paths);
    });
  }

  stageAll() {
    return this.run(async () => {
      await api.gitStageAll();
    });
  }

  /** Throws the changes of a file away. Untracked files are deleted. */
  discard(path: string, untracked: boolean) {
    return this.run(async () => {
      await api.gitDiscard(path, untracked);
    });
  }

  /** Creates a commit. Staged changes go in, the rest only when nothing is staged. */
  async commit() {
    const message = this.message.trim();
    if (!message) {
      this.error = "The commit message must not be empty.";
      return;
    }
    const stageAll = gitSettings.value.stageAllWhenNothingStaged && !this.hasStaged;
    await this.run(async () => {
      await api.gitCommit(message, stageAll);
      this.message = "";
    });
    if (gitSettings.value.pushAfterCommit && !this.error) await this.push();
  }

  push() {
    return this.run(async () => {
      await api.gitPush();
    });
  }

  pull() {
    return this.run(async () => {
      await api.gitPull();
    });
  }

  checkout(name: string) {
    return this.run(async () => {
      await api.gitCheckout(name);
    });
  }

  createBranch(name: string) {
    return this.run(async () => {
      await api.gitCreateBranch(name);
    });
  }

  /** Makes the vault folder a repository of its own. */
  init() {
    return this.run(async () => {
      await api.gitInit();
    });
  }

  /** Shows the changes of a file over the app. */
  async showDiff(path: string, staged: boolean, commit: string | null = null) {
    try {
      const sides = await api.gitDiff(path, staged, commit);
      this.diff = { title: path, ...sides };
      this.error = null;
    } catch (e) {
      this.error = reason(e);
    }
  }

  /** Closes the diff. */
  closeDiff() {
    this.diff = null;
  }

  /** Starts the automatic refresh of the state. */
  start() {
    this.stop();
    if (!gitSettings.value.autoRefresh) return;
    const seconds = Math.max(5, gitSettings.value.refreshSeconds);
    // Only while a panel shows the state and the window is looked at;
    // coming back (or opening the panel) catches up right away.
    this.timer = setInterval(() => {
      if (this.watched) void this.refresh();
    }, seconds * 1000);
    this.onVisible = () => {
      if (this.watched) void this.refresh();
    };
    document.addEventListener("visibilitychange", this.onVisible);
  }

  /** Stops the automatic refresh of the state. */
  stop() {
    if (this.timer !== undefined) {
      clearInterval(this.timer);
      this.timer = undefined;
    }
    clearTimeout(this.refreshTimer);
    this.refreshTimer = undefined;
    this.refreshAgain = false;
    if (this.onVisible) {
      document.removeEventListener("visibilitychange", this.onVisible);
      this.onVisible = undefined;
    }
  }

  /** Forgets everything, e.g. when the vault is closed. */
  reset() {
    this.stop();
    this.viewers = 0;
    this.status = null;
    this.branches = [];
    this.error = null;
    this.busy = false;
    this.message = "";
    this.diff = null;
  }
}

export const gitState = new GitState();
