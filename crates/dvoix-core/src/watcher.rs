//! Watches the vault folder so edits made outside dvoix (another editor, a
//! sync tool, git) show up in the app right away.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::event::{ModifyKind, RenameMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;

use crate::{Result, Vault, VaultEntry};

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
    /// The entry of the path, as far as it could be read: its size and the
    /// time it was changed. `None` when the file is gone or could not be
    /// read. It lets the app correct its list of the vault without listing
    /// the whole vault again.
    pub entry: Option<VaultEntry>,
}

/// How long a batch waits for more changes before it is handed over.
pub const BATCH_QUIET_MS: u64 = 250;

/// The longest a batch is held back. A sync tool that writes without pause
/// would otherwise keep the batch open forever, and neither the index nor
/// the app would ever see the changes.
pub const BATCH_MAX_MS: u64 = 1_000;

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

/// Collects the changes of the vault and hands them over in batches.
///
/// A git checkout or a sync tool writes hundreds of files at once. Handling
/// every change on its own would mean hundreds of passes over the index, so
/// the changes are collected until the vault is quiet for a moment and are
/// then handed over together.
pub struct ChangeBatcher {
    sender: ChangeSender,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

/// The way to give a change to the batcher. It can be copied, so the watcher
/// and the app can both hand changes over.
#[derive(Clone)]
pub struct ChangeSender {
    sender: Sender<VaultChange>,
}

impl ChangeSender {
    /// Adds a change to the batch that is being collected.
    pub fn send(&self, change: VaultChange) {
        // The receiver is gone when the batcher is dropped; nothing to do.
        let _ = self.sender.send(change);
    }
}

impl ChangeBatcher {
    /// Starts the thread that collects the changes. It calls `on_batch` with
    /// all the changes that came in while the vault was quiet.
    pub fn start<F>(on_batch: F) -> Result<Self>
    where
        F: Fn(Vec<VaultChange>) + Send + 'static,
    {
        let (sender, receiver) = std::sync::mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let handle = std::thread::spawn({
            let stop = Arc::clone(&stop);
            move || collect(receiver, stop, on_batch)
        });
        Ok(Self {
            sender: ChangeSender { sender },
            stop,
            handle: Some(handle),
        })
    }

    /// A handle to give changes to the batch.
    pub fn sender(&self) -> ChangeSender {
        self.sender.clone()
    }
}

impl Drop for ChangeBatcher {
    fn drop(&mut self) {
        // The thread has to stop, even when somebody still holds a sender.
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Waits for changes and hands them over in batches, as long as changes
/// keep coming.
fn collect<F>(receiver: Receiver<VaultChange>, stop: Arc<AtomicBool>, on_batch: F)
where
    F: Fn(Vec<VaultChange>),
{
    let quiet = Duration::from_millis(BATCH_QUIET_MS);
    let mut batch: Vec<VaultChange> = Vec::new();
    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        match receiver.recv_timeout(quiet) {
            Ok(change) => batch.push(change),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if batch.is_empty() {
            continue;
        }
        // Keep collecting while changes keep coming, so a burst becomes
        // one batch, but hand it over after `BATCH_MAX_MS` at the latest.
        let deadline = Instant::now() + Duration::from_millis(BATCH_MAX_MS);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match receiver.recv_timeout(quiet.min(left)) {
                Ok(change) => batch.push(change),
                Err(_) => break,
            }
        }
        on_batch(std::mem::take(&mut batch));
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
            let hidden = Vault::is_ignored(&rel);
            // The entry of the path, so the app can correct its list of the
            // vault without listing the whole vault again. Hidden paths are
            // not part of that list, so they are not read.
            let entry = if hidden { None } else { vault.entry(&rel).ok() };
            Some(VaultChange {
                kind: kind_for(p)?,
                hidden,
                path: rel,
                entry,
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
    fn a_change_carries_the_entry_of_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        std::fs::write(dir.path().join("note.md"), "# Note").unwrap();

        let change = VaultChange {
            kind: ChangeKind::Modified,
            path: "note.md".into(),
            hidden: false,
            entry: vault.entry("note.md").ok(),
        };

        let entry = change.entry.expect("the entry of the file");
        assert_eq!(entry.path, "note.md");
        assert_eq!(entry.name, "note.md");
        assert_eq!(entry.extension, "md");
        assert_eq!(entry.size, 6);
        assert!(!entry.is_dir);
        assert!(entry.modified > 0);
    }

    #[test]
    fn marks_hidden_changes_and_drops_noisy_ones() {
        assert!(is_noisy(".git/objects/ab/cdef"));
        assert!(is_noisy(".dvoix/workspace.json"));
        assert!(is_noisy("notes/.a.md.dvoix-tmp"));
        assert!(!is_noisy(".obsidian/app.json"));
        assert!(!is_noisy("sub/.git-notes.md"));
    }

    #[test]
    fn the_batcher_hands_over_several_changes_at_once() {
        let (tx, rx) = std::sync::mpsc::channel();
        let batcher = ChangeBatcher::start(move |changes| {
            let _ = tx.send(changes);
        })
        .unwrap();
        let sender = batcher.sender();
        for i in 0..5 {
            sender.send(VaultChange {
                kind: ChangeKind::Modified,
                path: format!("note-{i}.md"),
                hidden: false,
                entry: None,
            });
        }
        let batch = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the changes did not arrive");
        assert_eq!(batch.len(), 5);
        assert_eq!(batch[0].path, "note-0.md");
        assert_eq!(batch[4].path, "note-4.md");
    }

    /// Changes that never pause still reach the app, in batches.
    #[test]
    fn a_steady_stream_of_changes_is_handed_over_in_time() {
        let (tx, rx) = std::sync::mpsc::channel();
        let batcher = ChangeBatcher::start(move |changes| {
            let _ = tx.send(changes);
        })
        .unwrap();
        let sender = batcher.sender();
        let start = Instant::now();
        // A change every 50 ms for three seconds, so the vault is never quiet.
        let feeder = std::thread::spawn(move || {
            while start.elapsed() < Duration::from_secs(3) {
                sender.send(VaultChange {
                    kind: ChangeKind::Modified,
                    path: "note.md".into(),
                    hidden: false,
                    entry: None,
                });
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        let first = rx.recv_timeout(Duration::from_secs(2)).expect("no batch while changes kept coming");
        assert!(!first.is_empty());
        feeder.join().unwrap();
    }
}
