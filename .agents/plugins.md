# Plugins

Every feature of dvoix is a plugin. The API is in `src/lib/plugins/types.ts`, the management in `src/lib/plugins/manager.svelte.ts`. Store plugins (official and community) build on the same API, so the same limits apply to core plugins.

## Core plugin or store plugin?

- **Core plugin** (`src/lib/plugins/core/`): basic functions almost everyone needs and that are closely tied to the app (explorer, search, tasks, projects, tabs, Git, …). Shipped with the app; can be turned off.
- **Store plugin**: everything extra that not everyone wants (e.g. kanban, tracking). Its own repository with releases, an entry in `registry.json` of `DamavoiZ/dvoix-plugins`. Guide in `docs/plugins/README.md`.

## Plugin store (`src/lib/plugins/store/`)

- `store.svelte.ts` loads store plugins from `.dvoix/plugins/<id>/` (`main.js` as an ES module via a blob URL) and manages `.dvoix/store.json`. The manager knows them through `plugins.loadExtra`/`unloadExtra`, not through `corePlugins`.
- `api.ts` is the API for store plugins: the `PluginContext`, but with `mount(el, props)` instead of Svelte components, SVG text as icon, IDs prefixed with `<plugin-id>:`, and permissions (`vault:read`, `vault:write`, `editor`). If the `PluginContext` is extended, update it here and in `docs/plugins/README.md`.
- New permission: add it to `PERMISSIONS` in `crates/dvoix-core/src/store.rs`, to the type `PluginPermission` in `src/lib/api.ts`, to `PERMISSION_LABELS` and to the docs.
- Downloads only through `src-tauri/src/http.rs` (https, GitHub only), URLs only from `store::release_url`/`latest_manifest_url`.
- Permissions are not a sandbox. Never pretend they are one, neither in the UI nor in the docs.

## Rules

- One plugin = one folder `src/lib/plugins/core/<id>/` with an `index.ts` that exports a `Plugin` (`export const <name>Plugin: Plugin = { ... }`).
- Register it in `corePlugins` in `src/lib/plugins/core/index.ts`. The order there is the loading and display order.
- `manifest.id`: lowercase, `a-z0-9-`, stable. It is the folder name in `.dvoix/plugins/<id>/` and the key in `core-plugins.json`. Never rename an existing id, or users lose their settings.
- `manifest.name` and `description` in English; the description is one sentence.
- A plugin talks to the app **only** through the `ctx` from `onload`: `addPanel`, `addRibbonAction`, `addCommand`, `registerFileType`, `registerEditorExtension`, `addFileFooter`, `addPaneHeader`, `addOverlay`, `registerView`, `addExplorerEntries`, `addSettingTab`, `register`, `loadData`, `saveData`.
- Everything registered through `ctx` is removed automatically by the app when the plugin is turned off. What a plugin starts itself (listeners, timers, subscriptions) must be handed to `ctx.register(dispose)`. After switching off, nothing of the plugin may remain.
- A plugin does not import anything from another plugin's folder. What several plugins need goes to `src/lib/` (e.g. `src/lib/tasks.ts`, `src/lib/fuzzy.ts`).
- If a plugin needs something the API cannot do, extend the API in `types.ts` + `manager.svelte.ts` (with automatic cleanup) and add it to the plugin table in the README. No workarounds around the API.
- Every plugin must be switchable on and off without breaking the app or affecting other plugins. Test both.

## Plugin settings

With `pluginSettings` from `src/lib/plugins/pluginSettings.svelte.ts`, stored in `.dvoix/plugins/<id>/data.json`:

```ts
// settings.ts
export const mySettings = pluginSettings({ showDone: false });

// index.ts, in onload
await mySettings.load(ctx);
ctx.addSettingTab({ component: MySettings });

// in a component
mySettings.value.showDone = true;
mySettings.save();
```

Every key has a default value; missing keys are filled in on load. Only add new keys; never rename or reinterpret existing ones. The settings page is built on `src/lib/components/settings/Setting.svelte` and `Toggle.svelte`. Template: `core/tasks/` or `core/file-explorer/`.

## Commands and keyboard shortcuts

`ctx.addCommand({ id, name, hotkeys, run })`. The full id is `<plugin-id>:<id>`. `mod: true` means Ctrl on Windows/Linux and Cmd on macOS. Before adding a new shortcut, check that it is not taken yet (`grep -rn "hotkeys" src/lib/plugins`). New shortcuts go into the README.
