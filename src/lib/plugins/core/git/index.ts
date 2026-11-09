import { GitBranch, GitCommitHorizontal } from "@lucide/svelte";
import type { Plugin } from "../../types";
import { vault } from "../../../vault.svelte";
import GitDiff from "./GitDiff.svelte";
import GitSettings from "./GitSettings.svelte";
import HistoryView from "./HistoryView.svelte";
import SourceControlPanel from "./SourceControlPanel.svelte";
import { gitSettings } from "./settings";
import { gitState } from "./state.svelte";

/** Id of the view with the history of the repository. */
export const HISTORY_VIEW = "git-history";

export const gitPlugin: Plugin = {
  manifest: {
    id: "git",
    name: "Git",
    description: "Version the vault with Git: stage changes, commit, switch branches and view the history.",
    version: "1.0.0",
  },
  icon: GitBranch,
  async onload(ctx) {
    await gitSettings.load(ctx);
    ctx.addPanel({ id: "git", label: "Git", icon: GitBranch, component: SourceControlPanel, order: 55 });
    ctx.registerView({ id: HISTORY_VIEW, label: "History", icon: GitCommitHorizontal, component: HistoryView });
    ctx.addOverlay({ id: "git-diff", component: GitDiff });
    ctx.addSettingTab({ component: GitSettings });

    ctx.addCommand({
      id: "commit",
      name: "Commit changes",
      hotkeys: [{ mod: true, key: "Enter" }],
      when: () => !!vault.info,
      run: () => void gitState.commit(),
    });
    ctx.addCommand({
      id: "refresh",
      name: "Refresh Git status",
      when: () => !!vault.info,
      run: () => void gitState.refresh(),
    });
    ctx.addCommand({
      id: "push",
      name: "Push",
      when: () => !!vault.info,
      run: () => void gitState.push(),
    });
    ctx.addCommand({
      id: "pull",
      name: "Pull",
      when: () => !!vault.info,
      run: () => void gitState.pull(),
    });
    ctx.addCommand({
      id: "history",
      name: "Open history",
      hotkeys: [{ mod: true, shift: true, key: "h" }],
      when: () => !!vault.info,
      run: () => void vault.openViewTab(HISTORY_VIEW, { newTab: !!vault.openPath }),
    });

    // The state is loaded again every few seconds, so changes from other
    // programs show up too. Everything the plugin starts, it stops again.
    gitState.reset();
    gitState.start();
    ctx.register(() => gitState.stop());
    ctx.register(gitSettings.onChange(() => gitState.start()));
  },
};
