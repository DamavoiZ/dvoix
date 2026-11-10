# dvoix

A desktop app for notes, project management, tracking, and whatever comes later. Like Obsidian, all data lives as ordinary files in a folder on your computer (the **vault**). The files are the only source of truth; everything else is a cache that can be rebuilt at any time.

![dvoix](docs/screenshot.png)

## What already works

- Pick a vault folder; the one you opened last is loaded again at startup
- Sidebar like Obsidian: a narrow icon bar on the far left (Files, Search, Tags, Tasks, Projects), next to it the panel, resizable by dragging and collapsible by clicking the active icon
- Custom title bar on Windows and Linux: no bright system frame; the tab bar and the top row of the sidebar move the window (double-click maximizes); minimize, maximize and close sit on the right next to the tabs. Narrow, dark scrollbars everywhere in the app
- The file explorer sorts naturally: `2 - Notes` comes before `10 - Archive`
- File explorer with a folder tree: expand and collapse folders (remembered), right-click menu to create, rename and delete (deleted items go to `.trash/` in the vault), drag and drop files into folders
- Tabs: clicking in the file explorer opens in the active tab, Ctrl/Cmd + click or middle click opens in a new one; drag tabs to reorder, close them with middle click or ×. Open tabs are remembered per vault
- Quick switcher (Ctrl/Cmd + O) with fuzzy search over all files: `mtg plan` finds `Projects/Meeting-Planning`. Enter opens, Ctrl + Enter opens in a new tab, and if the note does not exist it is created
- Keyboard shortcuts: Ctrl + T new tab, Ctrl + W close tab, Ctrl + Tab / Ctrl + Shift + Tab (or Ctrl + Page Down/Up) switch tabs, Ctrl + Enter commit changes, Ctrl + Shift + H open history
- Markdown editor (CodeMirror 6) with YAML frontmatter, `[[wikilinks]]` (Ctrl/Cmd + click opens or creates the note) and `#tags`
- Properties (frontmatter) that you can edit like in Obsidian, in both the reading and the editing view: values by type (text, number, date, yes/no as a checkbox, lists and tags as chips, `[[links]]` clickable), rename keys, add and remove entries. Only the affected YAML line changes; comments and formatting stay. Values that span several lines (`|`, nested) are read-only and are edited in the source view. When the cursor is in the frontmatter, the editing view shows the YAML
- Tables like in Obsidian: in the editing view you type directly into the cells; the cell with the cursor shows its Markdown. Tab / Shift + Tab jumps to the next / previous cell, Enter to the cell below (in the last row a new one is created), the arrow keys move between cells and out of the table, Esc leaves it. Right-click inserts or deletes rows and columns; the "+" on the right and at the bottom appends a column or row. A typed `|` is saved as `\|` so it does not split the cell. To select several cells: drag over them with the mouse, Shift + click, Shift + arrow keys, or Ctrl/Cmd + A in an empty cell; whole rows and columns via the handles on the left and top (they appear on hover). Selected cells can be cleared (Del), copied, cut and pasted (including blocks from Excel or Google Sheets; the table grows when needed), made bold or italic (Ctrl/Cmd + B / I), and aligned via the right-click menu. Rows and columns are moved by dragging their handle; selected cells by dragging them to another place (the cells there swap places). Only what actually changes changes in the file
- Link suggestions: after `[[` the editor suggests matching notes and files (fuzzy search as in the quick switcher); Enter or Tab inserts the link including `]]`
- Three views like in Obsidian: **Read** (the note as a rendered page with the properties from the frontmatter), **Edit** (live preview: Markdown syntax is only visible on the line with the cursor, `## ` makes a heading immediately, tables stay tables while editing, see below) and **Source** (the raw Markdown). Switch with the buttons at the top right of the note, with Ctrl + E between Read and Edit, and Ctrl + Shift + E between Source and Edit. The view a note opens in is set in the settings.
- Automatic saving, written atomically (first a temporary file, then rename)
- Changes from outside (VS Code, sync tools, git) show up in the app immediately
- Full-text search, tag overview and backlinks ("Linked from")
- Tasks: ordinary Markdown checkboxes in any note, optionally with a due date and priority. In the editor they are always clickable; the task overview in the sidebar collects all tasks of the vault (grouped by due date or by note)
- Projects: notes with `type: project` in the frontmatter, with status, due date and progress from their checkboxes, in a dedicated project list
- Graph view (Ctrl/Cmd + G or the network icon in the icon bar) in its own tab: notes as dots, connected by links, by their folders (folders as their own nodes) and/or by shared tags (tags as nodes or as direct lines). Adjustable: filters (`projects #idea -archive`), single notes, attachments and missing notes on/off, colors by folder or tag, size by number of connections, arrows, line width, the zoom level from which titles appear, and the layout forces. Click opens the note in a new tab; hovering highlights the neighbors
- Basic functions are always on: file explorer, search, tags, backlinks, link suggestions, clickable checkboxes, quick switcher, tabs and views. Optional functions are core plugins that can each be turned off in the settings (gear icon at the bottom of the icon bar, or Ctrl/Cmd + ,): tasks, projects, canvas, graph, Git and hidden files (folders such as `.obsidian`, off at first)
- Plugin store in the settings: search, install, update and remove additional plugins. Official plugins work immediately; community plugins only after a separate switch with a warning (see "Plugin store" below)
- Settings: editor (line length, line numbers, spellcheck), appearance (font sizes), a list of all keyboard shortcuts, and separate options per feature or plugin
- All settings of a vault live inside the vault under `.dvoix/` (see below), so they move along when you copy or sync the folder
- Git: the vault is versioned with the system's `git` program. A panel in the sidebar shows what changed (staged, changed, untracked); you can stage single files, discard changes, commit (Ctrl/Cmd + Enter), switch and create branches, and push and pull. A separate tab shows the history with the files per commit and the diff
- First custom file type: `.canvas` in the open [JSON Canvas format](https://jsoncanvas.org) (move cards, double-click creates a card)

## Tech stack

| Layer | Technology |
| --- | --- |
| App shell | [Tauri 2](https://v2.tauri.app), `ureq` (downloads for the plugin store) |
| Core (Rust) | `notify` (file watcher), `pulldown-cmark` + `serde_yaml_ng` (Markdown & frontmatter), `rusqlite` with FTS5 (index & search), `git` (the system program, for the Git plugin) |
| Frontend | Svelte 5, TypeScript, Vite, Tailwind CSS 4, [force-graph](https://github.com/vasturiano/force-graph) (graph view on canvas) |
| Editor | CodeMirror 6 |

## Layout

```
crates/dvoix-core/      Rust core without Tauri, testable on its own
  src/vault.rs          reading/writing in the vault, paths always relative and checked
  src/markdown.rs       frontmatter, title, tags, links, headings, search text, HTML for the reading view
  src/tasks.rs          read and toggle checkbox tasks
  src/project.rs        frontmatter schema for projects
  src/git.rs            Git commands for the vault (via the system's git program)
  src/store.rs          plugin store: plugin list, manifest.json, installing into .dvoix/plugins/
  src/index.rs          SQLite index in <vault>/.dvoix/index.sqlite (cache only)
  src/watcher.rs        reports changes on disk
src-tauri/              Tauri app: holds the open vault, connects watcher, index and UI
  src/commands.rs       all commands the frontend can call
  src/http.rs           downloads for the plugin store (https only, GitHub only)
src/                    Svelte frontend
  lib/api.ts            typed calls to the Rust commands
  lib/vault.svelte.ts   app state: open vault, open file, saving
  lib/config.ts         read and write JSON files in <vault>/.dvoix/
  lib/settings.svelte.ts  editor and appearance settings
  lib/ui.svelte.ts      workspace: active panel, width, expanded folders, open tabs
  lib/window.svelte.ts  custom title bar: moving the window, window buttons
  lib/filetypes/        file type registry and the built-in Markdown editor
  lib/plugins/          plugin API (types.ts) and plugin management (manager.svelte.ts)
  lib/features/         the core features (always on), one folder each
  lib/plugins/core/     the core plugins (can be turned off), one folder each
  lib/plugins/store/    plugin store: load and install plugins, their API (api.ts)
docs/plugins/           guide for store plugins, example plugin, example plugin list
  lib/components/       app frame: ribbon, side panel, file view, settings
```

## The `.dvoix/` folder

Like `.obsidian/` in Obsidian. Everything that belongs to the vault lives here; only "which vault was opened last" is stored in the operating system's configuration folder.

```
.dvoix/
  app.json                  editor settings (line length, default view)
  appearance.json           font sizes
  core-plugins.json         which plugins are on: { "tasks": true, "canvas": false, ... }
  workspace.json            open tabs, active panel, width, expanded folders
  plugins/<id>/data.json    settings of a plugin (e.g. plugins/tasks/data.json)
  plugins/<id>/manifest.json, main.js, styles.css   a plugin from the store
  store.json                store plugins: on/off, where they come from, community plugins allowed yes/no
  index.sqlite              search index, cache only
```

If a file is missing, the default values apply. You can delete `index.sqlite` at any time; it is rebuilt. Folders whose names start with `.` are not indexed and not shown by default (see "Hidden files" below). The layout that earlier versions kept in the app's browser storage is migrated to `workspace.json` automatically the first time a vault is opened.

## Plugins

Every function hooks into the app through a small API, the same for core features (`src/lib/features/`, always on) and plugins (`src/lib/plugins/core/` and the store, can be turned off). Everything a plugin registers is removed automatically when it is turned off. Through the API you can:

| Call | What for |
| --- | --- |
| `ctx.addPanel(...)` | panel in the sidebar, with an icon in the icon bar |
| `ctx.addRibbonAction(...)` | button in the icon bar that runs something |
| `ctx.addCommand(...)` | command with an optional keyboard shortcut |
| `ctx.registerFileType(...)` | new file type with its own view |
| `ctx.registerEditorExtension(...)` | CodeMirror extension for the Markdown editor |
| `ctx.addFileFooter(...)` / `ctx.addPaneHeader(...)` | something below or above the open file (backlinks, tab bar) |
| `ctx.addOverlay(...)` | dialog on the top layer (quick switcher) |
| `ctx.addExplorerEntries(...)` | extra entries only in the file explorer (hidden folders) |
| `ctx.registerView(...)` | own view that runs like a file in a tab (graph); open with `vault.openViewTab(id)` |
| `ctx.addSettingTab(...)` | own page in the settings |
| `ctx.loadData()` / `ctx.saveData()` | own data in `.dvoix/plugins/<id>/data.json` |

### Hidden files

The "Hidden files" plugin is off at first. When on, it shows folders and files whose names start with a dot (e.g. `.obsidian`, `.trash`, `.gitignore`), slightly faded, in the file explorer. There you can open, edit, rename and delete them like anything else. They never appear in search, tags, backlinks, the graph or the quick switcher, and notes inside them are not indexed.

Settings (in `.dvoix/plugins/hidden-files/data.json`): show yes/no (also toggleable with the command "Show/hide hidden files"), whether single dot files appear too, whether the app's own `.dvoix` folder appears (off), and which names stay hidden anyway (default: `.git`). At most 10,000 hidden entries are listed per vault. The file watcher does not report changes in `.git` and `.dvoix`, so that a commit does not flood the app with events; their contents in the explorer are reloaded the next time you toggle the setting or create or delete a file.

### Plugin store

Besides the built-in core plugins there are plugins you install only when needed: **Settings → Plugin store**. Under "Browse" is the plugin list with search; a click shows details, version and permissions and installs the plugin. Under "Installed" you can turn them on and off, update them ("Check for updates") and uninstall them. Uninstalling keeps the plugin's settings (`data.json`).

- **Official plugins** are maintained by dvoix itself. They can always be installed.
- **Community plugins** are written by others. They only work once "Allow community plugins" is on, and a warning appears before that. Turning the switch off again unloads them immediately.

The plugin list is a `registry.json` in the repository [`dvoi-dev/dvoix-plugins`](https://github.com/dvoi-dev/dvoix-plugins); the plugins themselves come from the GitHub releases of their own repositories. Downloads only happen over https from GitHub. The app only shows "official" for repositories of `dvoi-dev`, regardless of what the list says.

Each plugin states in its manifest what it needs (`vault:read`, `vault:write`, `editor`). This is shown in the store before installing, and the API refuses anything that is not declared. It is not a sandbox: like in Obsidian, plugins run inside the app and could bypass the API. That is why official and community plugins are kept apart.

How to write a plugin for the store is described in [`docs/plugins/README.md`](docs/plugins/README.md), with an example plugin in `docs/plugins/hello/`.

## Adding a new feature

A new optional function is a new plugin; only basic note and editor functions that every vault needs become a core feature in `src/lib/features/` (same structure, listed in `src/lib/features/index.ts`, no on/off switch). Extras that not everyone wants go to the plugin store instead (see `docs/plugins/README.md`). Example of a core plugin: a Kanban board as `.board`:

1. Create the folder `src/lib/plugins/core/board/` with a `BoardView.svelte`. It receives `content` (the file content as text) and calls `onchange(newContent)` when something changes. The app takes care of saving.
2. Next to it, `index.ts`:
   ```ts
   export const boardPlugin: Plugin = {
     manifest: { id: "board", name: "Kanban", description: "Boards as .board files.", version: "1.0.0" },
     icon: Kanban, // any icon from @lucide/svelte
     onload(ctx) {
       ctx.registerFileType({
         id: "board",
         label: "Board",
         extensions: ["board"],
         icon: Kanban,
         view: BoardView,
         template: () => JSON.stringify({ columns: [] }, null, 2),
       });
     },
   };
   ```
3. Add it to the `corePlugins` list in `src/lib/plugins/core/index.ts`.

This makes the plugin appear in the settings for turning it on and off, "Create Board" in the toolbar and in the explorer's right-click menu, and `.board` files open in the new view. If the type should also be searchable, `crates/dvoix-core/src/index.rs` needs an extraction for the extension (see `is_indexable`).

For projects and tracking, frontmatter is the building block: `status: active`, `due: 2026-12-01` etc. end up in the index and can be queried with `notes_where("status", "active")`.

## Tasks and projects

A task is a checkbox in any note. The due date and priority sit directly in the line; the emoji syntax is the same as in the Obsidian Tasks plugin:

```markdown
- [ ] Write the offer 📅 2026-12-01 ⏫ #client
- [ ] Call back due:2026-11-20 !high
  - [x] Subtask
```

| | Syntax |
| --- | --- |
| Due date | `📅 2026-12-01` or `due:2026-12-01` |
| Priority | `!high` `!medium` `!low` (German `!hoch` `!mittel` `!niedrig` also work) or ⏫ 🔼 🔽 |

A project is a note with this frontmatter; its tasks are the checkboxes in it:

```yaml
---
type: project          # or: projekt (German alias)
status: active         # idea | active | waiting | done | cancelled
due: 2026-12-01        # optional
start: 2026-11-01      # optional
priority: high         # optional: high | medium | low
area: Work             # optional, free-form
---
```

German values (`aktiv`, `wartend`, `erledigt`, `hoch`, ...) are still accepted, so existing vaults keep working. Ticking a task in the task overview changes exactly that one line in the file.

## Git

The Git plugin versions the vault through the `git` program, which must be installed on the computer (e.g. Git for Windows, macOS or Linux). The core has no extra Rust component for this: `crates/dvoix-core/src/git.rs` calls `git` and translates its output. It is a core plugin like the others and can be turned off in the settings.

The panel in the sidebar shows what changed:

| Group | Meaning |
| --- | --- |
| Staged | changes that go into the next commit |
| Changed | changes to the file that are not staged yet |
| Untracked | files that git does not know yet |

Files can be staged one by one, unstaged, discarded (untracked files are deleted in that case) and viewed as a diff. The commit takes the staged changes; if nothing is staged, all changes go in (can be turned off in the settings). Branches can be switched and created in the panel, plus push and pull.

The history (Ctrl/Cmd + Shift + H) shows the latest commits; expanded, each commit lists its changed files, and a click on one shows the diff.

The diff shows the old version on the left and the new one on the right; both scroll together. Changed lines are highlighted in color (red removed, green new), and the changed characters within them are stronger. Long unchanged sections are collapsed and can be opened with a click. At the top right you can switch to "Inline": then everything is in one column, removed lines above the new ones. The plugin remembers this choice in `.dvoix/plugins/git/data.json`.

Files that dvoix itself keeps rewriting stay out of Git: the search index (`.dvoix/index.sqlite`), the open tabs (`.dvoix/workspace.json`) and the temporary files while saving (`*.dvoix-tmp`). They do not appear in the list of changes and are not included by "stage all". The other settings in `.dvoix/` are versioned as usual. Notes with Windows line endings (CRLF) keep them when saved, so that not every line changes.

If the vault sits in a folder of a larger repository, files outside the vault stay invisible: only what is in the vault is staged and committed. It is also important that `user.name` and `user.email` are set; otherwise git reports at commit time that it does not know the author.

## Development

Requirements: [Rust](https://rustup.rs) (current stable, chosen automatically through `rust-toolchain.toml`), Node.js 26 (see `.nvmrc`) and the [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/) for your operating system.

```sh
npm install
npm run tauri dev      # start the app with hot reload
npm run tauri build    # build the installer
```

After a pull that brings new dependencies, run `npm install` once. `npm run dev`, `npm run check` and `npm run build` check first whether all packages from `package.json` are installed, and say what is missing – instead of failing later in Vite with "Failed to resolve import …".

To check:

```sh
npm run check          # Svelte/TypeScript
cargo test --workspace # Rust core
```

## Next steps (ideas)

- Shared views (two tabs side by side)
- Kanban board (`.board`) and tracking (`.track`) as official plugins in the store
- `tantivy` instead of FTS5, if search needs to do more

## License

dvoix is available under the [MIT License](LICENSE-MIT) or the [Apache License 2.0](LICENSE-APACHE), at the user's choice.

Contributions you submit to this project are licensed under the same two licenses without any additional terms, unless you explicitly state otherwise.
