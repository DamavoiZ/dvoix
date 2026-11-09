//! Commands callable from the frontend via `invoke("name", { ...args })`.
//! The TypeScript side of this lives in `src/lib/api.ts`.
//!
//! Every command is `#[tauri::command(async)]`: plain sync commands run on the
//! main thread, and opening or re-indexing a large vault there freezes the
//! window (on Windows it stays white until the work is done).

use dvoix_core::{
    GraphData, NoteSummary, ProjectSummary, SearchHit, TaskItem, VaultEntry, index::TagCount, tasks,
};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{AppState, Workspace, settings};

type CmdResult<T> = Result<T, String>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultInfo {
    pub name: String,
    pub path: String,
}

fn err(e: impl ToString) -> String {
    e.to_string()
}

/// Runs `f` with the open workspace, or fails if no vault is open.
fn with_ws<T>(state: &State<AppState>, f: impl FnOnce(&Workspace) -> CmdResult<T>) -> CmdResult<T> {
    let guard = state.workspace.lock().map_err(err)?;
    let ws = guard.as_ref().ok_or("Kein Vault geöffnet")?;
    f(ws)
}

/// Re-indexes a file right away so search/backlinks are current even before
/// the watcher reports the change.
fn reindex(ws: &Workspace, path: &str) -> CmdResult<()> {
    ws.index
        .lock()
        .map_err(err)?
        .update(&ws.vault, path)
        .map_err(err)
}

#[tauri::command(async)]
pub fn last_vault(app: AppHandle) -> Option<String> {
    settings::load(&app).last_vault
}

#[tauri::command(async)]
pub fn open_vault(app: AppHandle, state: State<AppState>, path: String) -> CmdResult<VaultInfo> {
    let ws = Workspace::open(&app, &path).map_err(err)?;
    let info = VaultInfo {
        name: ws.vault.name(),
        path: ws.vault.root().to_string_lossy().into_owned(),
    };
    *state.workspace.lock().map_err(err)? = Some(ws);

    let mut s = settings::load(&app);
    s.last_vault = Some(info.path.clone());
    settings::save(&app, &s);
    Ok(info)
}

#[tauri::command(async)]
pub fn close_vault(app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    *state.workspace.lock().map_err(err)? = None;
    let mut s = settings::load(&app);
    s.last_vault = None;
    settings::save(&app, &s);
    Ok(())
}

#[tauri::command(async)]
pub fn list_entries(state: State<AppState>) -> CmdResult<Vec<VaultEntry>> {
    with_ws(&state, |ws| ws.vault.list().map_err(err))
}

#[tauri::command(async)]
pub fn read_file(state: State<AppState>, path: String) -> CmdResult<String> {
    with_ws(&state, |ws| ws.vault.read(&path).map_err(err))
}

#[tauri::command(async)]
pub fn write_file(state: State<AppState>, path: String, content: String) -> CmdResult<VaultEntry> {
    with_ws(&state, |ws| {
        let entry = ws.vault.write(&path, &content).map_err(err)?;
        reindex(ws, &path)?;
        Ok(entry)
    })
}

#[tauri::command(async)]
pub fn create_file(state: State<AppState>, path: String, content: String) -> CmdResult<VaultEntry> {
    with_ws(&state, |ws| {
        let entry = ws.vault.create(&path, &content).map_err(err)?;
        reindex(ws, &path)?;
        Ok(entry)
    })
}

#[tauri::command(async)]
pub fn create_folder(state: State<AppState>, path: String) -> CmdResult<VaultEntry> {
    with_ws(&state, |ws| ws.vault.create_dir(&path).map_err(err))
}

#[tauri::command(async)]
pub fn rename_entry(state: State<AppState>, from: String, to: String) -> CmdResult<VaultEntry> {
    with_ws(&state, |ws| {
        let entry = ws.vault.rename(&from, &to).map_err(err)?;
        let mut index = ws.index.lock().map_err(err)?;
        index.remove(&from).map_err(err)?;
        if entry.is_dir {
            index.rebuild(&ws.vault).map_err(err)?;
        } else {
            index.update(&ws.vault, &to).map_err(err)?;
        }
        Ok(entry)
    })
}

#[tauri::command(async)]
pub fn trash_entry(state: State<AppState>, path: String) -> CmdResult<()> {
    with_ws(&state, |ws| {
        ws.vault.trash(&path).map_err(err)?;
        ws.index.lock().map_err(err)?.remove(&path).map_err(err)
    })
}

#[tauri::command(async)]
pub fn search(state: State<AppState>, query: String) -> CmdResult<Vec<SearchHit>> {
    with_ws(&state, |ws| {
        ws.index
            .lock()
            .map_err(err)?
            .search(&query, 50)
            .map_err(err)
    })
}

#[tauri::command(async)]
pub fn tags(state: State<AppState>) -> CmdResult<Vec<TagCount>> {
    with_ws(&state, |ws| {
        ws.index.lock().map_err(err)?.tags().map_err(err)
    })
}

#[tauri::command(async)]
pub fn notes_with_tag(state: State<AppState>, tag: String) -> CmdResult<Vec<NoteSummary>> {
    with_ws(&state, |ws| {
        ws.index
            .lock()
            .map_err(err)?
            .notes_with_tag(&tag)
            .map_err(err)
    })
}

#[tauri::command(async)]
pub fn notes_where(
    state: State<AppState>,
    key: String,
    value: String,
) -> CmdResult<Vec<NoteSummary>> {
    with_ws(&state, |ws| {
        ws.index
            .lock()
            .map_err(err)?
            .notes_where(&key, &value)
            .map_err(err)
    })
}

#[tauri::command(async)]
pub fn backlinks(state: State<AppState>, path: String) -> CmdResult<Vec<NoteSummary>> {
    with_ws(&state, |ws| {
        ws.index.lock().map_err(err)?.backlinks(&path).map_err(err)
    })
}

#[tauri::command(async)]
pub fn graph(state: State<AppState>) -> CmdResult<GraphData> {
    with_ws(&state, |ws| {
        ws.index.lock().map_err(err)?.graph().map_err(err)
    })
}

#[tauri::command(async)]
pub fn tasks(state: State<AppState>) -> CmdResult<Vec<TaskItem>> {
    with_ws(&state, |ws| {
        ws.index.lock().map_err(err)?.tasks().map_err(err)
    })
}

#[tauri::command(async)]
pub fn projects(state: State<AppState>) -> CmdResult<Vec<ProjectSummary>> {
    with_ws(&state, |ws| {
        ws.index.lock().map_err(err)?.projects().map_err(err)
    })
}

/// Checks or unchecks a task in its file. `text` must still match, so a task
/// is never toggled on the wrong line after the file changed.
#[tauri::command(async)]
pub fn set_task_done(
    state: State<AppState>,
    path: String,
    line: u32,
    text: String,
    done: bool,
) -> CmdResult<VaultEntry> {
    with_ws(&state, |ws| {
        let entry = tasks::set_done_in_vault(&ws.vault, &path, line, &text, done).map_err(err)?;
        reindex(ws, &path)?;
        Ok(entry)
    })
}

/// Reads a file from the vault's `.dvoix/` folder (settings, workspace,
/// plugin data). Returns `None` if it does not exist yet.
#[tauri::command(async)]
pub fn read_config(state: State<AppState>, path: String) -> CmdResult<Option<String>> {
    with_ws(&state, |ws| ws.vault.read_config(&path).map_err(err))
}

/// Writes a file into the vault's `.dvoix/` folder.
#[tauri::command(async)]
pub fn write_config(state: State<AppState>, path: String, content: String) -> CmdResult<()> {
    with_ws(&state, |ws| {
        ws.vault.write_config(&path, &content).map_err(err)
    })
}

#[tauri::command(async)]
pub fn rebuild_index(state: State<AppState>) -> CmdResult<usize> {
    with_ws(&state, |ws| {
        ws.index
            .lock()
            .map_err(err)?
            .rebuild(&ws.vault)
            .map_err(err)
    })
}
