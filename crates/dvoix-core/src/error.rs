use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("index error: {0}")]
    Index(#[from] rusqlite::Error),
    #[error("watcher error: {0}")]
    Watch(#[from] notify::Error),
    #[error("vault folder does not exist: {0}")]
    VaultNotFound(PathBuf),
    #[error("path is outside of the vault: {0}")]
    OutsideVault(String),
    #[error("path already exists: {0}")]
    AlreadyExists(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
