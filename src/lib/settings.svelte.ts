// Vault settings from `.dvoix/app.json` and `.dvoix/appearance.json`.
import { readJson, writeJson } from "./config";

export interface AppSettings {
  /** Limit the editor text to a comfortable width instead of the full pane. */
  readableLineLength: boolean;
  /** Browser spell checking in the editor. */
  spellcheck: boolean;
  /** Line numbers in the markdown editor. */
  lineNumbers: boolean;
}

export interface AppearanceSettings {
  /** Editor font size in px. */
  fontSize: number;
  /** Base font size of the interface in px (scales all rem-based sizes; the editor has its own). */
  interfaceFontSize: number;
}

export const APP_DEFAULTS: AppSettings = { readableLineLength: true, spellcheck: false, lineNumbers: false };
export const APPEARANCE_DEFAULTS: AppearanceSettings = { fontSize: 15, interfaceFontSize: 16 };

class SettingsStore {
  app = $state<AppSettings>({ ...APP_DEFAULTS });
  appearance = $state<AppearanceSettings>({ ...APPEARANCE_DEFAULTS });

  async load() {
    this.app = await readJson("app.json", APP_DEFAULTS);
    this.appearance = await readJson("appearance.json", APPEARANCE_DEFAULTS);
  }

  reset() {
    this.app = { ...APP_DEFAULTS };
    this.appearance = { ...APPEARANCE_DEFAULTS };
  }

  saveApp() {
    writeJson("app.json", $state.snapshot(this.app));
  }

  saveAppearance() {
    writeJson("appearance.json", $state.snapshot(this.appearance));
  }
}

export const settings = new SettingsStore();
