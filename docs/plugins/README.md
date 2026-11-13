# Writing plugins for the plugin store

dvoix has two kinds of plugins:

- **Core features** are built into the app and always on (file explorer, search, tags, backlinks, tabs, …). They live in `src/lib/features/`.
- **Core plugins** are built into the app (tasks, canvas, graph, …) and can be turned off in the settings under "Core plugins". They live in `src/lib/plugins/core/`.
- **Store plugins** are installed when needed via **Settings → Plugin store**. They then live in the vault under `.dvoix/plugins/<id>/`. There are **official** plugins (maintained by dvoix, always installable) and **community plugins** (by others, only after the switch "Allow community plugins").

This page describes store plugins. A complete example is in [`hello/`](hello/).

## Structure

A plugin is a GitHub repository with releases. Each release has these files (assets):

| File | Required | Content |
| --- | --- | --- |
| `manifest.json` | yes | name, version, permissions (see below) |
| `main.js` | yes | the code as an ES module, all in one file |
| `styles.css` | no | styles, loaded as long as the plugin is on |

The **tag of the release is the version** from `manifest.json`, without a `v` in front (e.g. `1.2.0`). The app first fetches `manifest.json` from the newest release (`releases/latest`), then `main.js` and `styles.css` from the release with this tag.

### manifest.json

```json
{
  "id": "hello",
  "name": "Hello World",
  "version": "1.0.0",
  "description": "One sentence about what the plugin does.",
  "author": "your-name",
  "minAppVersion": "0.1.0",
  "permissions": ["vault:read"]
}
```

- `id`: lowercase, `a-z0-9-`, never change it. It is the folder name in `.dvoix/plugins/<id>/` and must match the `id` in the plugin directory. IDs of core features and core plugins (`tabs`, `tasks`, `graph`, …) are reserved.
- `version`: `Major.Minor.Patch`, numbers only.
- `minAppVersion`: the oldest dvoix version the plugin runs on (optional).
- `permissions`: what the plugin needs from the API:

| Permission | allows |
| --- | --- |
| `vault:read` | `dvoix.vault.list()`, `dvoix.vault.read(path)` |
| `vault:write` | `dvoix.vault.write(path, content)` |
| `editor` | `dvoix.registerEditorExtension(...)` |
| `process` | `dvoix.process.spawn(...)`: run programs on the computer, started in the vault folder |

Permissions are shown in the store before anyone installs. If a plugin calls something it did not declare, the API throws an error. This is not a sandbox: like in Obsidian, a plugin runs inside the app and could bypass the API. That is why community plugins sit behind a switch with a warning.

### main.js

```js
export default {
  async onload(dvoix) {
    dvoix.addCommand({ id: "hello", name: "Say hello", run: () => alert("Hello!") });
  },
  onunload() {},
};
```

`onload` runs when the plugin is switched on (or when the vault is opened), `onunload` when it is switched off. The app removes everything that was registered through `dvoix` by itself when switching off. Your own timers or listeners are handed over with `dvoix.register(() => ...)`.

`main.js` is a single ES module without imports. If you use libraries or TypeScript, bundle beforehand (e.g. with esbuild or Vite in library mode, format `es`).

## The API (`dvoix`)

Views are `mount(el, props)` functions: they draw into an empty element and optionally return a cleanup function or `{ update(props), destroy() }`. Icons are SVG text (e.g. copied from lucide.dev). All IDs are only valid within your own plugin.

| Call | What for |
| --- | --- |
| `addPanel({ id, label, icon, mount })` | panel in the sidebar, with an icon in the icon bar; with `side: "right"` it stands in the right sidebar |
| `addRightSidebar({ id, mount, toggle })` | the right edge of the window, e.g. a sidebar with panels of the right side; `toggle` is an optional button in the title row that shows and hides the sidebar |
| `addRibbonAction({ id, label, icon, action })` | button in the icon bar |
| `addCommand({ id, name, hotkeys, run })` | command with an optional keyboard shortcut |
| `addFileFooter({ id, mount })` | something below the open file; `props.path` is the file |
| `addPaneHeader({ id, mount })` | something above the open file |
| `addOverlay({ id, mount })` | element on the top layer (dialogs) |
| `registerView({ id, label, icon, mount })` | own view in a tab, open with `dvoix.openView(id)` |
| `registerFileType({ id, label, extensions, icon, template, mount })` | new file type; `props` has `path`, `content`, `onchange(text)`, `onopen(path)`, `update(props)` is called when the file changes on disk |
| `registerEditorExtension(ext)` | CodeMirror extension, built with `dvoix.lib.codemirror.state` / `.view` (same instance as the editor); `dvoix.lib.codemirror.merge` is `@codemirror/merge` for diff views |
| `addSettingTab({ mount })` | own page in the settings |
| `loadData(defaults)` / `saveData(data)` | settings in `.dvoix/plugins/<id>/data.json` |
| `vault.list()`, `vault.read(path)`, `vault.write(path, text)`, `vault.openFile(path)` | files, paths relative to the vault |
| `vault.onChange(fn)` | needs `vault:read`; `fn(paths)` gets the vault-relative paths of every batch of changes on disk (own saves, sync, git, other editors); returns the unsubscribe |
| `register(fn)` | `fn` runs when the plugin is switched off |
| `workspace.activeFile()` | vault-relative path of the file in the active tab, `null` for views and empty tabs |
| `process.spawn({ command, args, stdin, env, onStdout, onStderr, onExit })` | needs `process`; see below |

### Running programs (`process`)

```js
const proc = dvoix.process.spawn({
  command: "claude",            // looked up on PATH (on Windows also .cmd/.exe via PATHEXT), or a path
  args: ["-p", "--output-format", "stream-json", "--verbose"],
  stdin: "Summarize my notes",  // written to stdin, then stdin is closed
  onStdout: (chunk) => {},      // text, in chunks of any size
  onStderr: (chunk) => {},
  onExit: (code) => {},         // null if killed or if it could not be started
});
proc.kill();
```

The program always runs in the vault folder, with the rights of the user, so it can reach more than the vault. If it can't be started (e.g. not installed), the reason arrives on `onStderr` and `onExit(null)` follows. Running programs are killed when the plugin is switched off and when the vault is closed. On Windows no console window opens. Prefer stdin over arguments for free text: `cmd.exe` changes quotes and newlines in the arguments of `.cmd` files.

Colors: the CSS variables of Tailwind (e.g. `var(--color-zinc-300)`, `var(--color-violet-500)`) match the rest of the app. Give classes in `styles.css` their own prefix; the file applies to the whole app.

## Testing locally

Without the store: put the three files into `<vault>/.dvoix/plugins/<id>/`, reopen the vault and turn the plugin on under **Plugin store → Installed**. Plugins copied by hand count as community plugins, so turn on "Allow community plugins" for them.

## Getting into the store

The list of plugins is in `registry.json` in [`dvoi-dev/dvoix-plugins`](https://github.com/dvoi-dev/dvoix-plugins) (example: [`registry.example.json`](registry.example.json)):

```json
{
  "version": 1,
  "plugins": [
    {
      "id": "hello",
      "name": "Hello World",
      "author": "damavoi",
      "description": "One sentence about what the plugin does.",
      "repo": "dvoi-dev/dvoix-plugin-hello",
      "channel": "official"
    }
  ]
}
```

A community plugin is added via pull request with `"channel": "community"`. `"official"` only applies to repositories of `dvoi-dev`; for all others the app shows the plugin as a community plugin, even if the list says something else.
