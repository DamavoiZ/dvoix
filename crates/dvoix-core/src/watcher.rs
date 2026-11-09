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
}

/// Keeps the OS watcher alive; dropping it stops watching.
pub struct VaultWatcher {
    _inner: RecommendedWatcher,
}

impl VaultWatcher {
    /// Calls `on_change` for every change of a visible file or folder.
    /// Hidden paths (`.dvoix`, `.git`, temp files, ...) are filtered out.
    pub fn start<F>(vault: &Vault, on_change: F) -> Result<Self>
    where
        F: Fn(VaultChange) + Send + 'static,
    {
        let vault_for_events = vault.clone();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
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
        EventKind::Modify(ModifyKind::Name(_)) => {
            Some(if path.exists() { ChangeKind::Created } else { ChangeKind::Removed })
        }
        EventKind::Modify(ModifyKind::Metadata(_)) => None,
        EventKind::Modify(_) => Some(ChangeKind::Modified),
        _ => None,
    };

    event
        .paths
        .iter()
        .filter_map(|p| {
            let rel = vault.relative(p)?;
            if Vault::is_ignored(&rel) {
                return None;
            }
            Some(VaultChange { kind: kind_for(p)?, path: rel })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn reports_external_writes_but_not_hidden_files() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        let (tx, rx) = mpsc::channel();
        let _watcher = VaultWatcher::start(&vault, move |c| {
            let _ = tx.send(c);
        })
        .unwrap();

        std::fs::write(dir.path().join(".dvoix").join("cache"), "x").unwrap();
        std::fs::write(dir.path().join("neu.md"), "# Neu").unwrap();

        let change = rx.recv_timeout(Duration::from_secs(5)).expect("no change reported");
        assert_eq!(change.path, "neu.md");
    }
}
