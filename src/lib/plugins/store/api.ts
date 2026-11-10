// The API a store plugin gets in `onload(dvoix)`. It is the `PluginContext` of
// core plugins, adapted for code that is not compiled with the app:
//
// - UI is a `mount(el, props)` function drawing into a plain element instead
//   of a Svelte component, icons are SVG text.
// - Ids of panels, views, file types, ... are prefixed with `<plugin-id>:`,
//   so a plugin can't replace something of the app or of another plugin.
// - Vault and editor access only work if the manifest asks for them
//   (`permissions`). This is not a sandbox: a plugin runs inside the app like
//   in Obsidian, the permissions say what it means to do, are shown before
//   installing and keep honest plugins to what they declared.
import * as cmState from "@codemirror/state";
import * as cmView from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { api, type PluginPermission, type StoreManifest, type VaultEntry } from "../../api";
import { vault } from "../../vault.svelte";
import type { CommandSpec, Dispose, PluginContext } from "../types";
import { iconComponent, mountComponent } from "./component";

/** What `mount` may return: nothing, a cleanup function or a handle. */
export type Mounted = { update?(props: any): void; destroy?(): void };
export type MountResult = void | undefined | (() => void) | Mounted;
export type MountFn<P = Record<string, never>> = (el: HTMLElement, props: P) => MountResult;

export interface FileViewProps {
  path: string;
  content: string;
  onchange: (content: string) => void;
  onopen: (path: string) => void;
}

/** The object a store plugin's `onload` receives. */
export interface DvoixPluginApi {
  readonly manifest: StoreManifest;
  addPanel(spec: { id: string; label: string; icon?: string; header?: boolean; order?: number; side?: "left" | "right"; mount: MountFn }): Dispose;
  addRibbonAction(spec: { id: string; label: string; icon?: string; order?: number; action: () => void }): Dispose;
  addCommand(spec: CommandSpec): Dispose;
  /** The right edge of the window, e.g. a sidebar with panels of the right side. */
  addRightSidebar(spec: { id: string; mount: MountFn; toggle?: MountFn }): Dispose;
  addFileFooter(spec: { id: string; order?: number; mount: MountFn<{ path: string }> }): Dispose;
  addPaneHeader(spec: { id: string; order?: number; mount: MountFn }): Dispose;
  addOverlay(spec: { id: string; mount: MountFn }): Dispose;
  /** A view in a tab; open it with `openView(id)`. */
  registerView(spec: { id: string; label: string; icon?: string; mount: MountFn }): Dispose;
  registerFileType(spec: {
    id: string;
    label: string;
    extensions: string[];
    icon?: string;
    template?: (name: string) => string;
    mount: MountFn<FileViewProps>;
  }): Dispose;
  /** Needs the `editor` permission. Build extensions with `lib.codemirror`. */
  registerEditorExtension(extension: Extension): Dispose;
  addSettingTab(spec: { mount: MountFn }): Dispose;
  register(fn: Dispose): void;
  loadData<T extends object>(defaults: T): Promise<T>;
  saveData(data: unknown): void;

  vault: {
    /** Needs `vault:read`. All files and folders, vault-relative. */
    list(): VaultEntry[];
    /** Needs `vault:read`. */
    read(path: string): Promise<string>;
    /** Needs `vault:write`. Creates the file if it does not exist. */
    write(path: string, content: string): Promise<void>;
    openFile(path: string): Promise<void>;
  };
  openView(id: string): Promise<void>;
  /** The app's own CodeMirror, so editor extensions fit the editor. */
  lib: { codemirror: { state: typeof cmState; view: typeof cmView } };
}

export const PERMISSION_LABELS: Record<PluginPermission, string> = {
  "vault:read": "Read files in the vault",
  "vault:write": "Change and create files in the vault",
  editor: "Extend the Markdown editor (can see the text of the open note)",
};

export function createApi(ctx: PluginContext, manifest: StoreManifest): DvoixPluginApi {
  const pid = manifest.id;
  const own = (id: string) => `${pid}:${id}`;
  const need = (p: PluginPermission) => {
    if (!manifest.permissions.includes(p)) {
      throw new Error(`The plugin "${manifest.name}" uses the permission "${p}" but does not declare it in its manifest`);
    }
  };

  return {
    manifest,
    addPanel: (s) =>
      ctx.addPanel({ ...s, id: own(s.id), icon: iconComponent(s.icon), component: mountComponent(s.mount) }),
    addRightSidebar: (s) =>
      ctx.addRightSidebar({
        id: own(s.id),
        component: mountComponent(s.mount),
        toggle: s.toggle ? mountComponent(s.toggle) : undefined,
      }),
    addRibbonAction: (s) => ctx.addRibbonAction({ ...s, id: own(s.id), icon: iconComponent(s.icon) }),
    addCommand: (s) => ctx.addCommand(s),
    addFileFooter: (s) => ctx.addFileFooter({ id: own(s.id), order: s.order, component: mountComponent(s.mount) }),
    addPaneHeader: (s) => ctx.addPaneHeader({ id: own(s.id), order: s.order, component: mountComponent(s.mount) }),
    addOverlay: (s) => ctx.addOverlay({ id: own(s.id), component: mountComponent(s.mount) }),
    registerView: (s) =>
      ctx.registerView({ id: own(s.id), label: s.label, icon: iconComponent(s.icon), component: mountComponent(s.mount) }),
    registerFileType: (s) =>
      ctx.registerFileType({
        id: own(s.id),
        label: s.label,
        extensions: s.extensions,
        icon: iconComponent(s.icon),
        template: s.template,
        view: mountComponent(s.mount),
      }),
    registerEditorExtension: (ext) => {
      need("editor");
      return ctx.registerEditorExtension(ext);
    },
    addSettingTab: (s) => ctx.addSettingTab({ component: mountComponent(s.mount) }),
    register: (fn) => ctx.register(fn),
    loadData: (d) => ctx.loadData(d),
    saveData: (d) => ctx.saveData(d),
    vault: {
      list: () => {
        need("vault:read");
        return vault.entries.map((e) => ({ ...e }));
      },
      read: async (path) => {
        need("vault:read");
        return api.readFile(path);
      },
      write: async (path, content) => {
        need("vault:write");
        const exists = vault.entries.some((e) => e.path === path && !e.isDir);
        await (exists ? api.writeFile(path, content) : api.createFile(path, content));
      },
      openFile: (path) => vault.openFile(path),
    },
    openView: (id) => vault.openViewTab(own(id)),
    lib: { codemirror: { state: cmState, view: cmView } },
  };
}
