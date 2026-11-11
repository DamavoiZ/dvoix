//! Tauri shell around `dvoix-core`: owns the open vault, wires the file watcher
//! to the index and the UI, and exposes everything as commands to the frontend.

mod commands;
mod http;
mod process;
mod settings;

use std::sync::{Arc, Mutex};

use dvoix_core::{ChangeBatcher, Index, Vault, VaultChange, VaultWatcher};
use tauri::{AppHandle, Emitter, Manager};

/// Event sent to the frontend whenever something in the vault changed on disk.
/// The payload is always an array of `VaultChange`, also for a single change:
/// a burst arrives as one event instead of one webview dispatch per file.
pub const VAULT_CHANGED_EVENT: &str = "vault://changed";

/// The parts of the open vault a command needs.
///
/// They are taken out of the lock of the workspace, so no command holds that
/// lock while it reads or writes files. A slow listing of the vault does not
/// hold up reading a file any more.
#[derive(Clone)]
pub struct VaultHandle {
    pub vault: Vault,
    pub index: Arc<Mutex<Index>>,
}

/// Everything that belongs to the currently open vault.
pub struct Workspace {
    pub handle: VaultHandle,
    _watcher: VaultWatcher,
    _batcher: ChangeBatcher,
}

#[derive(Default)]
pub struct AppState {
    pub workspace: Mutex<Option<Workspace>>,
    /// Programs started by store plugins; killed when the vault closes.
    pub processes: Arc<process::Processes>,
}

impl Workspace {
    /// Opens a vault, brings its index in line with the files and starts
    /// watching it.
    pub fn open(app: &AppHandle, path: &str) -> dvoix_core::Result<Self> {
        let vault = Vault::open(path)?;
        let mut index = Index::for_vault(&vault)?;
        // Only the notes that changed since the last time are read again.
        index.sync(&vault)?;
        let index = Arc::new(Mutex::new(index));

        // The changes of the watcher are collected and handed over in
        // batches, so a burst of changes costs one pass over the index
        // instead of one per change.
        let batcher = ChangeBatcher::start({
            let index = Arc::clone(&index);
            let vault = vault.clone();
            let app = app.clone();
            move |changes: Vec<VaultChange>| {
                {
                    let Ok(mut index) = index.lock() else {
                        return;
                    };
                    if let Err(e) = index.apply_changes(&vault, &changes) {
                        // One change that does not work must not lose the others.
                        eprintln!("dvoix: the index could not be updated: {e}");
                        for change in &changes {
                            let _ = index.apply_changes(&vault, std::slice::from_ref(change));
                        }
                    }
                }
                // The app is told about the changes now, not when they came
                // in: the index knows about them, so the panels read the new
                // state and not the one from before. The whole batch travels
                // as one event, so a burst costs one serialization and one
                // refresh cycle in the app instead of one per change.
                let _ = app.emit(VAULT_CHANGED_EVENT, &changes);
            }
        })?;

        let watcher = {
            let app = app.clone();
            let sender = batcher.sender();
            VaultWatcher::start(&vault, move |change| {
                if change.hidden {
                    // Hidden files are not indexed, so the app is told right
                    // away: there is nothing to wait for. The payload stays
                    // an array, like the one of a batch.
                    let _ = app.emit(VAULT_CHANGED_EVENT, std::slice::from_ref(&change));
                } else {
                    sender.send(change);
                }
            })?
        };

        Ok(Self {
            handle: VaultHandle { vault, index },
            _watcher: watcher,
            _batcher: batcher,
        })
    }
}

/// WebKitGTK jumps line by line on every wheel step unless smooth scrolling is
/// switched on, which made scrolling on Linux feel rough next to WebView2 on
/// Windows. Wry leaves the setting alone, so it is set here.
#[cfg(target_os = "linux")]
fn smooth_scrolling(app: &tauri::App) {
    use webkit2gtk::{SettingsExt, WebViewExt};
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.with_webview(|webview| {
        if let Some(settings) = WebViewExt::settings(&webview.inner()) {
            settings.set_enable_smooth_scrolling(true);
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(AppState::default());
            #[cfg(target_os = "linux")]
            smooth_scrolling(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::last_vault,
            commands::open_vault,
            commands::close_vault,
            commands::list_entries,
            commands::list_hidden_entries,
            commands::read_file,
            commands::write_file,
            commands::render_markdown,
            commands::create_file,
            commands::read_binary,
            commands::create_binary,
            commands::create_folder,
            commands::rename_entry,
            commands::trash_entry,
            commands::search,
            commands::tags,
            commands::notes_with_tag,
            commands::notes_where,
            commands::backlinks,
            commands::graph,
            commands::tasks,
            commands::projects,
            commands::set_task_done,
            commands::read_config,
            commands::write_config,
            commands::rebuild_index,
            commands::git_status,
            commands::git_stage,
            commands::git_unstage,
            commands::git_stage_all,
            commands::git_discard,
            commands::git_commit,
            commands::git_log,
            commands::git_commit_files,
            commands::git_branches,
            commands::git_checkout,
            commands::git_create_branch,
            commands::git_push,
            commands::git_pull,
            commands::git_init,
            commands::git_diff,
            commands::store_registry,
            commands::store_latest_manifest,
            commands::store_install,
            commands::store_uninstall,
            commands::store_installed,
            commands::process_spawn,
            commands::process_kill,
        ])
        .run(tauri::generate_context!())
        .expect("error while running dvoix");
}

#[cfg(test)]
mod tests {
    use super::*;
    use dvoix_core::Vault;

    /// A workspace without a window, for the tests.
    fn workspace(vault: &Vault) -> Workspace {
        let mut index = Index::for_vault(vault).unwrap();
        index.sync(vault).unwrap();
        let index = Arc::new(Mutex::new(index));
        let batcher = ChangeBatcher::start(|_| {}).unwrap();
        let watcher = VaultWatcher::start(vault, |_| {}).unwrap();
        Workspace {
            handle: VaultHandle {
                vault: vault.clone(),
                index,
            },
            _watcher: watcher,
            _batcher: batcher,
        }
    }

    /// A command must not hold the lock of the workspace while it works, so
    /// a slow listing of the vault does not hold up the other commands.
    #[test]
    fn a_command_does_not_hold_the_workspace_lock() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        vault.write("note.md", "# Note").unwrap();
        let state = AppState {
            workspace: Mutex::new(Some(workspace(&vault))),
            processes: Default::default(),
        };

        commands::with_ws(&state, |ws| {
            // The lock is free while the command works.
            let guard = state.workspace.lock().unwrap();
            assert!(guard.is_some());
            drop(guard);
            ws.vault
                .read("note.md")
                .map(|_| ())
                .map_err(|e| e.to_string())
        })
        .unwrap();
    }

    /// Without a vault the commands fail with a clear message.
    #[test]
    fn a_command_without_a_vault_fails() {
        let state = AppState::default();
        let result = commands::with_ws(&state, |_| Ok(()));
        assert_eq!(result.unwrap_err(), "No vault is open");
    }
}
