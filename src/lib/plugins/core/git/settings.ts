import { pluginSettings } from "../../pluginSettings.svelte";

export const gitSettings = pluginSettings({
  /** The state of the repository is loaded again every few seconds. */
  autoRefresh: true,
  /** How often that happens, in seconds. */
  refreshSeconds: 30,
  /** Stage every change when committing and nothing is staged yet. */
  stageAllWhenNothingStaged: true,
  /** Show files that git does not know yet. */
  showUntracked: true,
  /** Push the branch after every commit. */
  pushAfterCommit: false,
  /** How the diff is laid out: old and new side by side, or in one column. */
  diffLayout: "split" as "split" | "inline",
});
