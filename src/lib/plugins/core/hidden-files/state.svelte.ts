// The hidden entries the explorer shows, kept up to date while the plugin is on.
import { api, onVaultChanged, type VaultEntry } from "../../../api";
import { hiddenSettings } from "./settings";

const REFRESH_DELAY_MS = 300;

let entries = $state.raw<VaultEntry[]>([]);
let timer: ReturnType<typeof setTimeout> | undefined;
/** Drops answers that arrive after a newer request or after the plugin was turned off. */
let generation = 0;

export const hidden = {
  get entries() {
    return entries;
  },
};

export async function refresh() {
  clearTimeout(timer);
  const run = ++generation;
  const s = hiddenSettings.value;
  if (!s.visible) {
    entries = [];
    return;
  }
  try {
    const list = await api.listHiddenEntries({ includeFiles: s.includeFiles, includeAppDir: s.includeAppDir, exclude: [...s.exclude] });
    if (run === generation) entries = list;
  } catch (e) {
    console.error("hidden-files: listing failed", e);
  }
}

function scheduleRefresh() {
  clearTimeout(timer);
  timer = setTimeout(() => void refresh(), REFRESH_DELAY_MS);
}

/**
 * Re-lists on changes in hidden places and when files appear or vanish (a
 * renamed folder moves the dot-files in it). Plain edits of notes don't need it.
 * Changes in `.git` and `.dvoix` are not reported at all (see `watcher.rs`).
 */
export async function watch(): Promise<() => void> {
  const unlisten = await onVaultChanged((c) => {
    if (c.hidden || c.kind !== "modified") scheduleRefresh();
  });
  return () => {
    unlisten();
    clearTimeout(timer);
    generation++;
    entries = [];
  };
}
