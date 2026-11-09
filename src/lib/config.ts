// Reading and writing JSON files in the vault's `.dvoix/` folder, like Obsidian's
// `.obsidian/`. Everything that belongs to a vault lives there:
//
//   .dvoix/app.json                 editor settings
//   .dvoix/appearance.json          font size and other looks
//   .dvoix/core-plugins.json        which core plugins are on ({ "tasks": true, ... })
//   .dvoix/workspace.json           open tabs, side panel, expanded folders
//   .dvoix/plugins/<id>/data.json   settings of one plugin
//   .dvoix/plugins/<id>/manifest.json, main.js, styles.css   a plugin from the store
//   .dvoix/store.json               store plugins: on/off, source, community switch
//   .dvoix/index.sqlite             search index (cache, can be deleted)
import { api } from "./api";

const WRITE_DELAY_MS = 300;

const timers = new Map<string, ReturnType<typeof setTimeout>>();
const pending = new Map<string, string>();

/** Reads `.dvoix/<path>` as JSON, filling in missing keys from `defaults`. */
export async function readJson<T extends object>(path: string, defaults: T): Promise<T> {
  const raw = await readJsonRaw(path);
  return raw && typeof raw === "object" && !Array.isArray(raw) ? { ...defaults, ...(raw as Partial<T>) } : { ...defaults };
}

/** Reads `.dvoix/<path>` as JSON; undefined if it is missing or broken. */
export async function readJsonRaw(path: string): Promise<unknown> {
  try {
    const text = await api.readConfig(path);
    return text === null ? undefined : JSON.parse(text);
  } catch {
    return undefined;
  }
}

/** Writes `.dvoix/<path>` shortly after the last call (many changes in a row become one write). */
export function writeJson(path: string, data: unknown): void {
  pending.set(path, JSON.stringify(data, null, 2) + "\n");
  clearTimeout(timers.get(path));
  timers.set(
    path,
    setTimeout(() => void flushPath(path), WRITE_DELAY_MS),
  );
}

async function flushPath(path: string) {
  clearTimeout(timers.get(path));
  timers.delete(path);
  const text = pending.get(path);
  if (text === undefined) return;
  pending.delete(path);
  await api.writeConfig(path, text).catch((e) => console.error(`.dvoix/${path}:`, e));
}

/** Writes everything that is still waiting (before closing the vault or the window). */
export async function flushConfig(): Promise<void> {
  await Promise.all([...pending.keys()].map(flushPath));
}
