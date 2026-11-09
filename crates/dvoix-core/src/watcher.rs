//! Watches the vault folder so edits made outside dvoix (another editor, a
//! sync tool, git) show up in the app right away.

use std::path::Path;

use notify::event::{ModifyKind, RenameMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;

use crate::{Result, Vault};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Created,
    Modified,
    Removed,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VaultChange {
    pub kind: ChangeKind,
    /// Vault-relative path.
    pub path: String,
    /// Inside a hidden folder or a dot-file (see `Vault::is_ignored`). Such
    /// changes are only reported for the "Hidden files" plugin and are
    /// never indexed.
    pub hidden: bool,
}

/// Hidden places whose changes are never reported: `.git` churns through
/// hundreds of object files on every commit and `.dvoix` is written by dvoix
/// itself all the time, so reporting them would flood the UI for nothing.
fn is_noisy(rel: &str) -> bool {
    let top = rel.split('/').next().unwrap_or_default();
    top == ".git" || top == crate::APP_DIR || rel.ends_with(".dvoix-tmp")
}

/// Keeps the OS watcher alive; dropping it stops watching.
pub struct VaultWatcher {
    _inner: RecommendedWatcher,
}

impl VaultWatcher {
    /// Calls `on_change` for every change of a file or folder. Changes in
    /// hidden places are marked `hidden`; those in `.git`, `.dvoix` and temp
    /// files are dropped (see `is_noisy`).
    pub fn start<F>(vault: &Vault, on_change: F) -> Result<Self>
    where
        F: Fn(VaultChange) + Send + 'static,
    {
        let vault_for_events = vault.clone();
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                let Ok(event) = res else { return };
                for change in to_changes(&vault_for_events, &event) {
                    on_change(change);
                }
            })?;
        watcher.watch(vault.root(), RecursiveMode::Recursive)?;
        Ok(Self { _inner: watcher })
    }
}

fn to_changes(vault: &Vault, event: &notify::Event) -> Vec<VaultChange> {
    let kind_for = |path: &Path| match event.kind {
        EventKind::Create(_) => Some(ChangeKind::Created),
        EventKind::Remove(_) => Some(ChangeKind::Removed),
        // Renames arrive as "from"/"to" pairs (or "any" on some platforms):
        // treat them as remove + create, decided by whether the path still exists.
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => Some(ChangeKind::Removed),
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => Some(ChangeKind::Created),
        EventKind::Modify(ModifyKind::Name(_)) => Some(if path.exists() {
            ChangeKind::Created
        } else {
            ChangeKind::Removed
        }),
        EventKind::Modify(ModifyKind::Metadata(_)) => None,
        EventKind::Modify(_) => Some(ChangeKind::Modified),
        _ => None,
    };

    event
        .paths
        .iter()
        .filter_map(|p| {
            let rel = vault.relative(p)?;
            if is_noisy(&rel) {
                return None;
            }
            Some(VaultChange {
                kind: kind_for(p)?,
                hidden: Vault::is_ignored(&rel),
                path: rel,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn reports_external_writes_but_not_app_files() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        let (tx, rx) = mpsc::channel();
        let _watcher = VaultWatcher::start(&vault, move |c| {
            let _ = tx.send(c);
        })
        .unwrap();

        std::fs::write(dir.path().join(".dvoix").join("cache"), "x").unwrap();
        std::fs::write(dir.path().join("new.md"), "# New").unwrap();

        let change = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("no change reported");
        assert_eq!(change.path, "new.md");
        assert!(!change.hidden);
    }

    #[test]
    fn marks_hidden_changes_and_drops_noisy_ones() {
        assert!(is_noisy(".git/objects/ab/cdef"));
        assert!(is_noisy(".dvoix/workspace.json"));
        assert!(is_noisy("notes/.a.md.dvoix-tmp"));
        assert!(!is_noisy(".obsidian/app.json"));
        assert!(!is_noisy("sub/.git-notes.md"));
    }
}
