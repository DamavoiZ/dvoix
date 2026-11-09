//! Core logic of dvoix.
//!
//! The vault (a plain folder on disk) is the single source of truth. Everything
//! else in this crate, the index in particular, is a cache that can be thrown
//! away and rebuilt from the files at any time.
//!
//! This crate has no dependency on Tauri so it can be tested and reused on its own.

pub mod error;
pub mod git;
pub mod index;
pub mod markdown;
pub mod project;
pub mod store;
pub mod tasks;
pub mod vault;
pub mod watcher;

pub use error::{Error, Result};
pub use index::{GraphData, Index, NoteSummary, ProjectSummary, SearchHit, TagCount, TaskItem};
pub use markdown::{ParsedNote, RenderedNote};
pub use project::{ProjectMeta, ProjectStatus};
pub use tasks::{Priority, Task};
pub use vault::{HiddenFilter, Vault, VaultEntry};
pub use watcher::{ChangeKind, VaultChange, VaultWatcher};

/// Name of the hidden folder inside a vault where dvoix keeps caches and settings.
pub const APP_DIR: &str = ".dvoix";
