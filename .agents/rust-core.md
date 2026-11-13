# Rust core and Tauri commands

## Split

- `crates/dvoix-core/` holds the logic and does not know Tauri. Everything that can be tested without a window belongs here.
- `src-tauri/` is only the shell: it holds the open vault (`AppState`/`Workspace` in `lib.rs`), connects watcher and index, and provides commands (`commands.rs`). No domain logic there.

## Rules in the core

- Every file access goes through `Vault` (`vault.rs`). Paths come in vault-relative with `/` and are checked with `Vault::resolve`. Never build paths yourself.
- Write only through `Vault::write` (atomic), or `create`, `rename`, `trash`. Deleting means moving to `.trash/`, never deleting for good.
- `.dvoix/` files through `read_config`/`write_config`/`config_path`, not through `write`.
- Folders and files starting with `.` are never indexed and never appear in `Vault::list` (`Vault::is_ignored`, `index::is_indexable`). This stays so. Only the "Hidden files" plugin may show them, via `Vault::list_hidden`. The watcher reports them with `hidden: true` and drops `.git`/`.dvoix` entirely.
- Errors via `crate::Error` and `crate::Result`. No `unwrap()`/`expect()` outside tests and the app start.
- Changes to files are minimal: `tasks::set_done` changes exactly one line. New functions that change notes work the same way.
- New logic gets tests in the same module (`#[cfg(test)] mod tests`). For filesystem tests use `tempfile::tempdir()`.

## Index (`index.rs`)

- `index.sqlite` is only a cache. It must always be fully rebuildable from the files with `rebuild`.
- If the schema changes, raise `SCHEMA_VERSION`. The migration then drops the tables and rebuilds. Do not store data in the index that does not come from files.
- Frontmatter lands in the index as JSON and can be queried with `notes_where(key, value)`.

## New Tauri command

1. Function in `src-tauri/src/commands.rs` with `#[tauri::command]`, return type `CmdResult<T>`, vault access through `with_ws`. After write access, call `reindex(ws, &path)`.
2. Add it to `tauri::generate_handler![...]` in `src-tauri/src/lib.rs`.
3. Add a typed wrapper in `src/lib/api.ts`. Rust structs use `#[serde(rename_all = "camelCase")]`; the TS interfaces mirror them exactly.
4. Error messages for users in English (like "No vault is open").
5. If the command needs new permissions, extend `src-tauri/capabilities/default.json` as narrowly as possible.

## Checks

```sh
cargo test --workspace
cargo clippy --workspace   # should pass without new warnings
```
