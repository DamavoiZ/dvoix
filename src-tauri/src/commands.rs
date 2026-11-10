//! Commands callable from the frontend via `invoke("name", { ...args })`.
//! The TypeScript side of this lives in `src/lib/api.ts`.
//!
//! Every command is `#[tauri::command(async)]`: plain sync commands run on the
//! main thread, and opening or re-indexing a large vault there freezes the
//! window (on Windows it stays white until the work is done).

use dvoix_core::{
    GraphData, HiddenFilter, NoteSummary, ProjectSummary, RenderedNote, SearchHit, TaskItem,
    VaultEntry, index::TagCount, markdown, tasks,
};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{AppState, VaultHandle, Workspace, http, settings};

use dvoix_core::store::{self, InstalledPlugin, PluginFiles, PluginManifest, RegistryEntry};

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

/// Runs `f` with the open vault. The lock of the workspace is only held to
/// get the vault; `f` reads and writes files without holding it, so a slow
/// listing does not hold up the other commands.
pub(crate) fn with_ws<T>(
    state: &AppState,
    f: impl FnOnce(&VaultHandle) -> CmdResult<T>,
) -> CmdResult<T> {
    let handle = {
        let guard = state.workspace.lock().map_err(err)?;
        let ws = guard.as_ref().ok_or("No vault is open")?;
        ws.handle.clone()
    };
    f(&handle)
}

/// Re-indexes a file right away so search/backlinks are current even before
/// the watcher reports the change.
fn reindex(ws: &VaultHandle, path: &str) -> CmdResult<()> {
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
        name: ws.handle.vault.name(),
        path: ws.handle.vault.root().to_string_lossy().into_owned(),
    };
    state.processes.kill_all();
    // The old vault is dropped outside the lock, so its watcher can finish.
    let old = state.workspace.lock().map_err(err)?.replace(ws);
    drop(old);

    let mut s = settings::load(&app);
    s.last_vault = Some(info.path.clone());
    settings::save(&app, &s);
    Ok(info)
}

#[tauri::command(async)]
pub fn close_vault(app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    state.processes.kill_all();
    // The workspace is dropped outside the lock, so its watcher can finish.
    let old = state.workspace.lock().map_err(err)?.take();
    drop(old);
    let mut s = settings::load(&app);
    s.last_vault = None;
    settings::save(&app, &s);
    Ok(())
}

#[tauri::command(async)]
pub fn list_entries(state: State<AppState>) -> CmdResult<Vec<VaultEntry>> {
    with_ws(&state, |ws| ws.vault.list().map_err(err))
}

/// Dot-folders and dot-files for the "Hidden files" plugin. They are
/// only listed, never indexed.
#[tauri::command(async)]
pub fn list_hidden_entries(
    state: State<AppState>,
    filter: HiddenFilter,
) -> CmdResult<Vec<VaultEntry>> {
    with_ws(&state, |ws| ws.vault.list_hidden(&filter).map_err(err))
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

/// Reads a file as raw bytes (images in notes). The bytes go to the frontend
/// as an `ArrayBuffer`, not as a JSON array of numbers.
#[tauri::command(async)]
pub fn read_binary(state: State<AppState>, path: String) -> CmdResult<tauri::ipc::Response> {
    with_ws(&state, |ws| {
        ws.vault
            .read_bytes(&path)
            .map(tauri::ipc::Response::new)
            .map_err(err)
    })
}

/// Creates an attachment (a pasted or dropped file). The bytes are the raw
/// request body; the vault path comes URI-encoded in the `path` header,
/// because a header can only carry ASCII.
#[tauri::command(async)]
pub fn create_binary(
    state: State<AppState>,
    request: tauri::ipc::Request,
) -> CmdResult<VaultEntry> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("Expected the file as raw bytes".into());
    };
    let path = request
        .headers()
        .get("path")
        .and_then(|v| v.to_str().ok())
        .ok_or("Missing path")?;
    let path = percent_decode(path)?;
    with_ws(&state, |ws| {
        ws.vault.create_bytes(&path, bytes).map_err(err)
    })
}

/// Decodes `encodeURIComponent` output.
fn percent_decode(text: &str) -> CmdResult<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3).ok_or("Bad path encoding")?;
            out.push(u8::from_str_radix(hex, 16).map_err(err)?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(err)
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
        if entry.is_dir {
            // Only the notes of the folder get their new path; the rest of
            // the vault stays as it is.
            index.move_folder(&ws.vault, &from, &to).map_err(err)?;
        } else {
            index.remove(&from).map_err(err)?;
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

/// The two versions of one file that its diff compares.
#[tauri::command(async)]
pub fn git_diff(
    state: State<AppState>,
    path: String,
    staged: bool,
    commit: Option<String>,
) -> CmdResult<git::FileDiff> {
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

// -------------------------------------------------------------- plugin store
// Downloads run before `with_ws`, so the vault is not locked while waiting
// for the network.

/// The plugin list from the registry repository.
#[tauri::command(async)]
pub fn store_registry() -> CmdResult<Vec<RegistryEntry>> {
    let text = http::get_text(store::REGISTRY_URL)?.ok_or("The plugin list was not found")?;
    store::parse_registry(&text).map_err(err)
}

/// The manifest of the newest release of a plugin, to show its version,
/// permissions and whether an update is there.
#[tauri::command(async)]
pub fn store_latest_manifest(repo: String) -> CmdResult<PluginManifest> {
    fetch_manifest(&repo).map(|(m, _)| m)
}

fn fetch_manifest(repo: &str) -> CmdResult<(PluginManifest, String)> {
    let url = store::latest_manifest_url(repo).map_err(err)?;
    let text = http::get_text(&url)?
        .ok_or_else(|| format!("{repo} has no release with a manifest.json"))?;
    let manifest = store::parse_manifest(&text).map_err(err)?;
    Ok((manifest, text))
}

/// Installs or updates a plugin from the newest release of `repo`. The files
/// come from the release tagged with the manifest's version, so all three
/// belong to the same release.
#[tauri::command(async)]
pub fn store_install(
    state: State<AppState>,
    repo: String,
    id: String,
) -> CmdResult<PluginManifest> {
    let (manifest, manifest_text) = fetch_manifest(&repo)?;
    let get = |file: &str| -> CmdResult<Option<String>> {
        http::get_text(&store::release_url(&repo, &manifest.version, file).map_err(err)?)
    };
    let main = get(store::MAIN_FILE)?
        .ok_or_else(|| format!("Release {} of {repo} is missing main.js", manifest.version))?;
    let styles = get(store::STYLES_FILE)?;
    let files = PluginFiles {
        manifest: manifest_text,
        main,
        styles,
    };
    with_ws(&state, |ws| {
        store::install(&ws.vault, &id, &files).map_err(err)
    })
}

/// Removes a plugin's code; its settings (`data.json`) stay.
#[tauri::command(async)]
pub fn store_uninstall(state: State<AppState>, id: String) -> CmdResult<()> {
    with_ws(&state, |ws| store::uninstall(&ws.vault, &id).map_err(err))
}

/// Store plugins in `.dvoix/plugins/`.
#[tauri::command(async)]
pub fn store_installed(state: State<AppState>) -> CmdResult<Vec<InstalledPlugin>> {
    with_ws(&state, |ws| store::installed(&ws.vault).map_err(err))
}

#[cfg(test)]
mod tests {
    use super::percent_decode;

    #[test]
    fn decodes_uri_components() {
        assert_eq!(
            percent_decode("9%20-%20System%2Fbild%C3%A4.png").unwrap(),
            "9 - System/bildä.png"
        );
        assert!(percent_decode("%zz").is_err());
        assert!(percent_decode("%4").is_err());
    }
}

/// Starts a program for a store plugin with the `process` permission, in the
/// vault folder. Output and exit arrive through `on_event`.
#[tauri::command(async)]
pub fn process_spawn(
    state: State<AppState>,
    command: String,
    args: Vec<String>,
    stdin: Option<String>,
    env: Option<std::collections::HashMap<String, String>>,
    on_event: tauri::ipc::Channel<crate::process::ProcessEvent>,
) -> CmdResult<u32> {
    let processes = std::sync::Arc::clone(&state.processes);
    with_ws(&state, |ws| {
        let spec = crate::process::SpawnSpec {
            command,
            args,
            stdin,
            env: env.unwrap_or_default(),
        };
        processes.spawn(ws.vault.root(), spec, on_event)
    })
}

#[tauri::command(async)]
pub fn process_kill(state: State<AppState>, id: u32) {
    state.processes.kill(id);
}
