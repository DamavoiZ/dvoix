//! Tauri shell around `dvoix-core`: owns the open vault, wires the file watcher
//! to the index and the UI, and exposes everything as commands to the frontend.

mod commands;
mod settings;

use std::sync::{Arc, Mutex};

use dvoix_core::{ChangeKind, Index, Vault, VaultWatcher};
use tauri::{AppHandle, Emitter, Manager};

/// Event sent to the frontend whenever something in the vault changed on disk.
pub const VAULT_CHANGED_EVENT: &str = "vault://changed";

/// Everything that belongs to the currently open vault.
pub struct Workspace {
    pub vault: Vault,
    pub index: Arc<Mutex<Index>>,
    _watcher: VaultWatcher,
}

#[derive(Default)]
pub struct AppState {
    pub workspace: Mutex<Option<Workspace>>,
}

impl Workspace {
    /// Opens a vault, rebuilds its index and starts watching it.
    pub fn open(app: &AppHandle, path: &str) -> dvoix_core::Result<Self> {
        let vault = Vault::open(path)?;
        let mut index = Index::for_vault(&vault)?;
        index.rebuild(&vault)?;
        let index = Arc::new(Mutex::new(index));

        let watcher = {
            let app = app.clone();
            let vault = vault.clone();
            let index = Arc::clone(&index);
            VaultWatcher::start(&vault.clone(), move |change| {
                if let Ok(mut index) = index.lock() {
                    let _ = match change.kind {
                        ChangeKind::Removed => index.remove(&change.path),
                        _ => index.update(&vault, &change.path),
                    };
                }
                let _ = app.emit(VAULT_CHANGED_EVENT, &change);
            })?
        };

        Ok(Self { vault, index, _watcher: watcher })
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(AppState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::last_vault,
            commands::open_vault,
            commands::close_vault,
            commands::list_entries,
            commands::read_file,
            commands::write_file,
            commands::create_file,
            commands::create_folder,
            commands::rename_entry,
            commands::trash_entry,
            commands::search,
            commands::tags,
            commands::notes_with_tag,
            commands::notes_where,
            commands::backlinks,
            commands::tasks,
            commands::projects,
            commands::set_task_done,
            commands::read_config,
            commands::write_config,
            commands::rebuild_index,
        ])
        .run(tauri::generate_context!())
        .expect("error while running dvoix");
}
