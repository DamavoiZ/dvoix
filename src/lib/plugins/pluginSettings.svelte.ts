// Reactive settings of one plugin, stored in `.dvoix/plugins/<id>/data.json`.
//
//   const settings = pluginSettings({ showDone: false });
//   onload: await settings.load(ctx)
//   in a component: settings.value.showDone = true; settings.save();
import type { PluginContext } from "./types";

export interface PluginSettings<T extends object> {
  readonly value: T;
  load(ctx: PluginContext): Promise<void>;
  save(): void;
  /** Called after every `save()`, e.g. to rebuild editor extensions. */
  onChange(fn: (value: T) => void): () => void;
}

export function pluginSettings<T extends object>(defaults: T): PluginSettings<T> {
  let value = $state<T>({ ...defaults });
  let ctx: PluginContext | undefined;
  const listeners = new Set<(value: T) => void>();
  return {
    get value() {
      return value;
    },
    async load(c) {
      ctx = c;
      value = await c.loadData(defaults);
    },
    save() {
      const snap = $state.snapshot(value) as T;
      ctx?.saveData(snap);
      for (const fn of listeners) fn(snap);
    },
    onChange(fn) {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
  };
}
