// State and actions of the git plugin: the state of the repository, the last
// error and the commands that the panel, the commands and the history share.
import { api, type GitBranch, type GitStatus } from "../../../api";
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
  /** The diff that is shown over the app, if any. */
  diff = $state<{ title: string; text: string } | null>(null);

  private timer: ReturnType<typeof setInterval> | undefined;

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

  /** Loads the state of the repository and the list of branches. */
  async refresh() {
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
      this.error = "Die Commit-Nachricht darf nicht leer sein.";
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
      const text = await api.gitDiff(path, staged, commit);
      this.diff = { title: path, text };
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
    this.timer = setInterval(() => void this.refresh(), seconds * 1000);
  }

  /** Stops the automatic refresh of the state. */
  stop() {
    if (this.timer !== undefined) {
      clearInterval(this.timer);
      this.timer = undefined;
    }
  }

  /** Forgets everything, e.g. when the vault is closed. */
  reset() {
    this.stop();
    this.status = null;
    this.branches = [];
    this.error = null;
    this.busy = false;
    this.message = "";
    this.diff = null;
  }
}

export const gitState = new GitState();
