# AGENTS.md

Fixed rules for all AI agents (Claude, Codex, Copilot, …) working in this repo. This file is the main source; `CLAUDE.md` only points here. Detailed rules on individual topics are in [`.agents/`](.agents/). Before a change, read the file for the affected topic.

## What dvoix is

A desktop app for notes, projects, tasks and tracking, similar to Obsidian. All data lives as ordinary files in a folder on disk (the **vault**). Basic note and editor functions are **core features** that are always on; optional functions are **core plugins** that can be turned off. A vault's settings live inside the vault under `.dvoix/`.

## The dvoix repos

dvoix is spread across several repositories. This section is also in the `AGENTS.md` of every repo. Whoever changes it, changes it in all of them.

| Repo | Purpose | What belongs there |
| --- | --- | --- |
| [`dvoi-dev/dvoix`](https://github.com/dvoi-dev/dvoix) | the app | Rust core, Tauri shell, Svelte frontend, core plugins, plugin API, plugin store in the app, developer guide for store plugins (`docs/plugins/`), developer README |
| [`dvoi-dev/dvoix-website`](https://github.com/dvoi-dev/dvoix-website) | product page and docs | product page at `/`, user and plugin developer docs at `/docs`, screenshots for the page, Docker image for the server |
| [`dvoi-dev/dvoix-plugins`](https://github.com/dvoi-dev/dvoix-plugins) | list of the plugin store | only `registry.json` and its check (`scripts/validate.mjs`). No plugin code |
| `dvoi-dev/dvoix-<name>` | one official store plugin each | code, `manifest.json`, releases with `manifest.json`, `main.js`, `styles.css` |

### Where does what go?

| Task | Repo |
| --- | --- |
| Basic note and editor function that is always there (explorer, search, tags, backlinks, tabs, views, …) | `dvoix`, as a core feature under `src/lib/features/` |
| Optional function that ships with the app (tasks, canvas, graph, …) | `dvoix`, as a core plugin under `src/lib/plugins/core/` |
| Extra function not everyone wants (kanban, projects, Git, tracking, …) | its own repo `dvoi-dev/dvoix-<name>` as a store plugin, plus an entry in `dvoix-plugins` |
| Extend the plugin API (`PluginContext`, permissions) | `dvoix`, plus `docs/plugins/README.md` there and the `plugin-api.md` page on the website |
| Explain how to use dvoix | `dvoix-website`, page in `src/content/docs/` |
| Offer or remove a plugin in the store | `dvoix-plugins`, `registry.json` |
| Bug in a store plugin | in the plugin's repo, not in the app |

### What is connected across repo boundaries

- **Format of the plugin list:** defined in `crates/dvoix-core/src/store.rs` (`parse_registry`) of the app, example in `docs/plugins/registry.example.json`, checked by `scripts/validate.mjs` in `dvoix-plugins`. If the format changes, all three places change.
- **Address of the plugin list:** the app reads `https://raw.githubusercontent.com/dvoi-dev/dvoix-plugins/main/registry.json`. Never change the repo name, the branch `main` or the file name, or no installed app finds plugins anymore.
- **Official:** `"channel": "official"` only applies to repos of `dvoi-dev`. The app and the check in `dvoix-plugins` both enforce this.
- **Plugin API:** `src/lib/plugins/store/api.ts` in the app, `docs/plugins/README.md` in the app and `src/content/docs/plugin-api.md` on the website describe the same API and must match.
- **Features of the app:** what the website docs say about the app must match the code in `dvoix`. If the app changes in a way users see, the docs are updated.

### Changes across several repos

- One PR per repo. The PRs link to each other in their descriptions.
- Merge order: first the app (`dvoix`), then `dvoix-plugins` and `dvoix-website`. A doc or plugin list must not promise anything the app does not yet support on `main`.
- For all repos the same applies: code, comments and commit messages in English, everything users see in English too; always the newest stable versions of Node.js, dependencies and GitHub Actions; never push directly to `main`.

## The seven basic rules

1. **The files are the only source of truth.** Everything else (`.dvoix/index.sqlite`, in-memory state) is cache and must be rebuildable from the files at any time. Never keep data only in the index, in `localStorage` or in the app.
2. **Basic functions are core features, everything else is a plugin.** Functions almost every note needs (file explorer, search, tags, backlinks, link suggestions, clickable checkboxes, quick switcher, tabs, views) are core features under `src/lib/features/<id>/`: always on, no on/off switch. Optional functions are core plugins under `src/lib/plugins/core/<id>/` (can be turned off) or store plugins. Both hook in only through the `PluginContext`. No special cases for a feature in `App.svelte` or in the components under `src/lib/components/`.
3. **Vault settings belong in `<vault>/.dvoix/`.** Only "which vault was opened last" lives in the operating system's configuration folder (`src-tauri/src/settings.rs`).
4. **Writing is atomic and minimal.** Files are written through `Vault::write` (temp file, then rename). If a function changes a note (e.g. ticks off a task), it changes only the affected line and never reformats the whole file.
5. **Paths are vault-relative and go through `Vault::resolve`.** No access outside the vault, no absolute paths from the frontend.
6. **Always the newest versions.** Node.js (current version, not an older LTS), Rust stable, all npm and Cargo dependencies and all GitHub Actions run on their newest version. Add new dependencies in their newest version, bump older ones when you touch them, and keep the CI, the README and `package.json` (`engines`) in line. Older versions only with a justification in the PR, e.g. when the newest is demonstrably broken.
7. **Everything is in English.** Code, identifiers, comments, commit messages, UI texts, error messages, plugin names and descriptions, tests, documentation and these rules. Do not add German text anywhere. The only exception is German alias values that the parsers must keep accepting (e.g. `!hoch`, `type: projekt`), and those are marked as aliases.

## Stack

| Layer | Technology |
| --- | --- |
| App shell | Tauri 2 (`src-tauri/`), `ureq` for the plugin store downloads, `webkit2gtk` (Linux only) for webview settings Tauri does not offer |
| Core | Rust crate `crates/dvoix-core/` without Tauri: `notify`, `pulldown-cmark`, `serde_yaml_ng`, `rusqlite` with FTS5, `walkdir` |
| Frontend | Svelte 5 (runes), TypeScript, Vite, Tailwind CSS 4, icons from `@lucide/svelte`, `force-graph` for the graph view |
| Editor | CodeMirror 6 |

## Layout

```
crates/dvoix-core/src/   logic without UI: vault.rs, markdown.rs, tasks.rs, store.rs, index.rs, watcher.rs
src-tauri/src/           Tauri shell: lib.rs (setup, handler list), commands.rs (all commands), http.rs (downloads for the store), settings.rs
src/lib/api.ts           typed wrappers for every Rust command
src/lib/config.ts        read/write JSON in <vault>/.dvoix/ (deferred, batched)
src/lib/*.svelte.ts      app state (vault, ui, settings, window) with runes
src/lib/filetypes/       file type registry, Markdown editor, fallback view
src/lib/features/        one folder per core feature (always on), list in features/index.ts
src/lib/plugins/         plugin API (types.ts), management (manager.svelte.ts), settings (pluginSettings.svelte.ts)
src/lib/plugins/core/    one folder per core plugin (can be turned off), list in core/index.ts
src/lib/plugins/store/   plugin store: load/install store plugins, their API (api.ts)
docs/plugins/            guide and example for store plugins, example plugin list
src/lib/components/      app frame: ribbon, side panel, right sidebar, file pane, settings, context menu
```

## Commands

```sh
npm install              # once
npm run check            # svelte-check: Svelte + TypeScript
npm run build            # build the frontend (Vite)
cargo test --workspace   # Rust tests (core + Tauri crate)
npm run tauri dev        # start the app (needs Tauri system dependencies)
```

Before every commit, `npm run check`, `npm run build` and `cargo test --workspace` must pass. These exact three run in the CI (`.github/workflows/ci.yml`). On Linux the Rust build needs `libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev libxdo-dev libssl-dev`. If a check cannot run locally, say so explicitly instead of claiming success.

## Conventions

- **Language:** code, identifiers, comments and commit messages in English. Everything users see (UI texts, plugin names and descriptions, error messages from commands, README) in English as well.
- **Comments** explain the why, not the what. Every file starts with a short comment on what it is for (see the existing files).
- **Svelte 5 with runes** (`$state`, `$derived`, `$effect`, `$props`). No `export let`, no Svelte 4 stores.
- **Styling** with Tailwind classes and the CSS variables from `src/app.css`. Do not hard-code new colors.
- **No new dependencies** without a good reason. If there is one, justify it in the PR.
- **Small, focused changes.** One PR, one topic. No reformatting of unrelated files.
- **Rust:** errors via `dvoix_core::Error` and `Result`, no `unwrap()` outside tests. New logic in the core gets tests in the same module (`#[cfg(test)] mod tests`, vault via `tempfile::tempdir()`).
- **Keep the README current:** new feature, new file type, new file in `.dvoix/` or new plugin API → add a section to the README.

## Topic rules in `.agents/`

| File | When to read |
| --- | --- |
| [`.agents/plugins.md`](.agents/plugins.md) | new core feature or plugin, changing one, plugin settings |
| [`.agents/filetypes.md`](.agents/filetypes.md) | new file type (e.g. `.board`, `.track`) |
| [`.agents/rust-core.md`](.agents/rust-core.md) | changes in the Rust core, new Tauri commands, index |
| [`.agents/frontend.md`](.agents/frontend.md) | Svelte components, app state, editor |
| [`.agents/vault-data.md`](.agents/vault-data.md) | file formats, frontmatter, `.dvoix/`, task syntax |
| [`.agents/workflow.md`](.agents/workflow.md) | branches, commits, PRs, what is checked before finishing |
