// The plugin API. Every feature of dvoix (file explorer, search, tasks, canvas, ...)
// is a plugin built on this, like the core plugins of Obsidian. The same API is
// meant for community plugins later on: a plugin only talks to the app through
// the `PluginContext` it gets in `onload`, and everything it registers there is
// removed again automatically when it is turned off.
import type { Extension } from "@codemirror/state";
import type { Component } from "svelte";
import type { FileType } from "../filetypes/types";

/** Describes a plugin. For community plugins this would be their `manifest.json`. */
export interface PluginManifest {
  /** Stable id, also the folder name of its data: `.dvoix/plugins/<id>/`. Lower-case, `a-z0-9-`. */
  id: string;
  /** Name shown in the settings, e.g. "Aufgaben". */
  name: string;
  /** One sentence on what it does. */
  description: string;
  version: string;
  author?: string;
  /** On in a fresh vault. Defaults to true for core plugins. */
  enabledByDefault?: boolean;
}

export interface Plugin {
  manifest: PluginManifest;
  /** Icon in the plugin list. */
  icon?: Component;
  /** Called when the plugin is turned on (or the vault is opened with it on). */
  onload(ctx: PluginContext): void | Promise<void>;
  /** Optional extra cleanup. Everything registered through `ctx` is removed anyway. */
  onunload?(): void | Promise<void>;
}

/** A panel in the left sidebar with its icon in the ribbon. */
export interface PanelSpec {
  /** Unique id; also stored in `workspace.json` as the open panel. */
  id: string;
  /** Title in the ribbon tooltip and in the panel header. */
  label: string;
  icon: Component;
  component: Component;
  /** Show the label as a header above the panel (default true). */
  header?: boolean;
  /** Position in the ribbon, lower comes first (default 100). */
  order?: number;
}

/** A button in the ribbon that does something instead of opening a panel. */
export interface RibbonActionSpec {
  id: string;
  label: string;
  icon: Component;
  action: () => void;
  order?: number;
}

/** Ctrl on Windows/Linux, Cmd on macOS, plus a key as in `KeyboardEvent.key` (case-insensitive). */
export interface Hotkey {
  mod?: boolean;
  shift?: boolean;
  alt?: boolean;
  key: string;
}

export interface CommandSpec {
  /** Unique within the plugin; the full id is `<plugin-id>:<id>`. */
  id: string;
  /** Name shown to users, e.g. "Neuer Tab". */
  name: string;
  hotkeys?: Hotkey[];
  /** Also run while a dialog (quick switcher, settings) is open. */
  inModal?: boolean;
  /** Only when a vault is open and this returns true. */
  when?: () => boolean;
  run: () => void | Promise<void>;
}

/** Something shown below the open file, e.g. backlinks. Receives `{ path }` as props. */
export interface FileFooterSpec {
  id: string;
  component: Component<{ path: string }>;
  order?: number;
}

/** Something shown above the open file, e.g. the tab bar. */
export interface PaneHeaderSpec {
  id: string;
  component: Component;
  order?: number;
}

/** A component mounted once at the top level of the app, e.g. a dialog. */
export interface OverlaySpec {
  id: string;
  component: Component;
}

/** The plugin's page in the settings dialog. */
export interface SettingTabSpec {
  component: Component;
}

export type Dispose = () => void;

export interface PluginContext {
  readonly manifest: PluginManifest;

  addPanel(panel: PanelSpec): Dispose;
  addRibbonAction(action: RibbonActionSpec): Dispose;
  addCommand(command: CommandSpec): Dispose;
  /** A new file type: extension, view and (optionally) a template for "create". */
  registerFileType(type: FileType): Dispose;
  /** CodeMirror extensions for the markdown editor. */
  registerEditorExtension(extension: Extension): Dispose;
  addFileFooter(footer: FileFooterSpec): Dispose;
  addPaneHeader(header: PaneHeaderSpec): Dispose;
  addOverlay(overlay: OverlaySpec): Dispose;
  addSettingTab(tab: SettingTabSpec): Dispose;
  /** Runs `fn` when the plugin is turned off. */
  register(fn: Dispose): void;

  /** Reads `.dvoix/plugins/<id>/data.json`, missing keys filled from `defaults`. */
  loadData<T extends object>(defaults: T): Promise<T>;
  /** Writes `.dvoix/plugins/<id>/data.json`. */
  saveData(data: unknown): void;
}
