//! Commands callable from the frontend via `invoke("name", { ...args })`.
//! The TypeScript side of this lives in `src/lib/api.ts`.
//!
//! Every command is `#[tauri::command(async)]`: plain sync commands run on the
//! main thread, and opening or re-indexing a large vault there freezes the
//! window (on Windows it stays white until the work is done).

use dvoix_core::{
    GraphData, NoteSummary, ProjectSummary, RenderedNote, SearchHit, TaskItem, VaultEntry,
    index::TagCount, markdown, tasks,
};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{AppState, Workspace, settings};

use dvoix_core::git::{self, Branch, Commit, GitStatus};

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

/// Renders a note as HTML for the reading view. It only depends on the text of
/// the note, so it needs no vault.
#[tauri::command(async)]
pub fn render_markdown(content: String) -> RenderedNote {
    markdown::render(&content)
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

// ---------------------------------------------------------------------- git

/// The state of the repository the vault is in: what is staged, what is
/// changed and what is new. A vault without a repository is not an error.
#[tauri::command(async)]
pub fn git_status(state: State<AppState>) -> CmdResult<GitStatus> {
    with_ws(&state, |ws| git::status(&ws.vault).map_err(err))
}

/// Stages the given files.
#[tauri::command(async)]
pub fn git_stage(state: State<AppState>, paths: Vec<String>) -> CmdResult<()> {
    with_ws(&state, |ws| git::stage(&ws.vault, &paths).map_err(err))
}

/// Moves the given files back out of the index.
#[tauri::command(async)]
pub fn git_unstage(state: State<AppState>, paths: Vec<String>) -> CmdResult<()> {
    with_ws(&state, |ws| git::unstage(&ws.vault, &paths).map_err(err))
}

/// Stages every change of the vault.
#[tauri::command(async)]
pub fn git_stage_all(state: State<AppState>) -> CmdResult<()> {
    with_ws(&state, |ws| git::stage_all(&ws.vault).map_err(err))
}

/// Throws the changes of one file away.
#[tauri::command(async)]
pub fn git_discard(state: State<AppState>, path: String, untracked: bool) -> CmdResult<()> {
    with_ws(&state, |ws| {
        git::discard(&ws.vault, &path, untracked).map_err(err)
    })
}

/// Creates a commit, with the short id of the new commit as result.
#[tauri::command(async)]
pub fn git_commit(state: State<AppState>, message: String, stage_all: bool) -> CmdResult<String> {
    with_ws(&state, |ws| {
        git::commit(&ws.vault, &message, stage_all).map_err(err)
    })
}

/// The last commits, newest first.
#[tauri::command(async)]
pub fn git_log(state: State<AppState>, limit: u32) -> CmdResult<Vec<Commit>> {
    with_ws(&state, |ws| git::log(&ws.vault, limit).map_err(err))
}

/// The files a commit changed.
#[tauri::command(async)]
pub fn git_commit_files(state: State<AppState>, id: String) -> CmdResult<Vec<String>> {
    with_ws(&state, |ws| git::commit_files(&ws.vault, &id).map_err(err))
}

/// The local branches and the ones on the remotes.
#[tauri::command(async)]
pub fn git_branches(state: State<AppState>) -> CmdResult<Vec<Branch>> {
    with_ws(&state, |ws| git::branches(&ws.vault).map_err(err))
}

/// Checks out another branch.
#[tauri::command(async)]
pub fn git_checkout(state: State<AppState>, name: String) -> CmdResult<()> {
    with_ws(&state, |ws| git::checkout(&ws.vault, &name).map_err(err))
}

/// Creates a branch and checks it out.
#[tauri::command(async)]
pub fn git_create_branch(state: State<AppState>, name: String) -> CmdResult<()> {
    with_ws(&state, |ws| {
        git::create_branch(&ws.vault, &name).map_err(err)
    })
}

/// Pushes the current branch to its remote.
#[tauri::command(async)]
pub fn git_push(state: State<AppState>) -> CmdResult<String> {
    with_ws(&state, |ws| git::push(&ws.vault).map_err(err))
}

/// Fetches the remote branch and merges it into the current one.
#[tauri::command(async)]
pub fn git_pull(state: State<AppState>) -> CmdResult<String> {
    with_ws(&state, |ws| git::pull(&ws.vault).map_err(err))
}

/// Makes the vault folder a repository of its own.
#[tauri::command(async)]
pub fn git_init(state: State<AppState>) -> CmdResult<()> {
    with_ws(&state, |ws| git::init(&ws.vault).map_err(err))
}

/// The changes of one file as a diff.
#[tauri::command(async)]
pub fn git_diff(
    state: State<AppState>,
    path: String,
    staged: bool,
    commit: Option<String>,
) -> CmdResult<String> {
    with_ws(&state, |ws| {
        git::diff(&ws.vault, &path, staged, commit.as_deref()).map_err(err)
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
