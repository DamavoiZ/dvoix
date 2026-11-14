// Vault settings from `.dvoix/app.json` and `.dvoix/appearance.json`.
import { readJson, writeJson } from "./config";

export interface AppSettings {
  /** Limit the editor text to a comfortable width instead of the full pane. */
  readableLineLength: boolean;
  /** Browser spell checking in the editor. */
  spellcheck: boolean;
  /** Line numbers in the markdown editor. */
  lineNumbers: boolean;
  /** How a note is shown: reading view, live preview or the markdown source. */
  viewMode: ViewMode;
}

/** The three ways a note can be shown, like in Obsidian. */
export type ViewMode = "read" | "edit" | "source";

export type ThemeId = "dark" | "midnight" | "slate" | "light";
export type AccentColor = "violet" | "sky" | "emerald" | "rose" | "amber";
export type EditorFont = "sans" | "serif" | "mono";
export type InterfaceFont = "sans" | "system" | "mono";

export interface AppearanceSettings {
  /** Color theme preset. */
  theme: ThemeId;
  /** Accent highlight color preset. */
  accentColor: AccentColor;
  /** Font family used in the editor and reading view. */
  editorFont: EditorFont;
  /** Font family used across the app interface. */
  interfaceFont: InterfaceFont;
  /** Editor font size in px. */
  fontSize: number;
  /** Base font size of the interface in px (scales all rem-based sizes; the editor has its own). */
  interfaceFontSize: number;
}

export const APP_DEFAULTS: AppSettings = {
  readableLineLength: true,
  spellcheck: false,
  lineNumbers: false,
  viewMode: "edit",
};
export const APPEARANCE_DEFAULTS: AppearanceSettings = {
  theme: "dark",
  accentColor: "violet",
  editorFont: "sans",
  interfaceFont: "sans",
  fontSize: 15,
  interfaceFontSize: 16,
};

class SettingsStore {
  app = $state<AppSettings>({ ...APP_DEFAULTS });
  appearance = $state<AppearanceSettings>({ ...APPEARANCE_DEFAULTS });

  async load() {
    // The two files do not depend on each other: read both at once.
    [this.app, this.appearance] = await Promise.all([
      readJson("app.json", APP_DEFAULTS),
      readJson("appearance.json", APPEARANCE_DEFAULTS),
    ]);
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
