//! Access to the vault folder. All paths exchanged with the outside world are
//! relative to the vault root and use `/` as separator, on every platform.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use walkdir::WalkDir;

use crate::{Error, Result, APP_DIR};

/// Folder (inside the vault) that deleted files are moved to, like in Obsidian.
pub const TRASH_DIR: &str = ".trash";

#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VaultEntry {
    /// Path relative to the vault root, `/`-separated.
    pub path: String,
    pub name: String,
    /// Lower-case extension without the dot, empty for folders and files without one.
    pub extension: String,
    pub is_dir: bool,
    pub size: u64,
    /// Modification time in milliseconds since the Unix epoch.
    pub modified: u64,
}

impl Vault {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        if !root.is_dir() {
            return Err(Error::VaultNotFound(root));
        }
        let root = root.canonicalize()?;
        fs::create_dir_all(root.join(APP_DIR))?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn app_dir(&self) -> PathBuf {
        self.root.join(APP_DIR)
    }

    pub fn name(&self) -> String {
        self.root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Vault".into())
    }

    /// Resolves a vault-relative path to an absolute one, refusing anything
    /// that would escape the vault.
    pub fn resolve(&self, rel: &str) -> Result<PathBuf> {
        let rel_path = Path::new(rel);
        let mut out = self.root.clone();
        for c in rel_path.components() {
            match c {
                Component::Normal(part) => out.push(part),
                Component::CurDir => {}
                _ => return Err(Error::OutsideVault(rel.to_string())),
            }
        }
        Ok(out)
    }

    /// Turns an absolute path back into a vault-relative, `/`-separated one.
    pub fn relative(&self, abs: &Path) -> Option<String> {
        let rel = abs.strip_prefix(&self.root).ok()?;
        let parts: Vec<_> = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect();
        if parts.is_empty() {
            return None;
        }
        Some(parts.join("/"))
    }

    /// True for paths dvoix should never show or index (hidden folders like
    /// `.dvoix`, `.git`, `.trash`, `.obsidian`).
    pub fn is_ignored(rel: &str) -> bool {
        rel.split('/').any(|part| part.starts_with('.'))
    }

    /// All files and folders in the vault (hidden ones excluded), sorted by path.
    pub fn list(&self) -> Result<Vec<VaultEntry>> {
        let mut entries = Vec::new();
        let walker = WalkDir::new(&self.root)
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'));
        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if let Some(rel) = self.relative(entry.path()) {
                if let Ok(e) = self.entry_for(&rel, entry.path()) {
                    entries.push(e);
                }
            }
        }
        // Plain path order; the UI builds the tree and decides how to group it.
        entries.sort_by_key(|e| e.path.to_lowercase());
        Ok(entries)
    }

    /// Only the files of the vault (no folders).
    pub fn files(&self) -> Result<Vec<VaultEntry>> {
        Ok(self.list()?.into_iter().filter(|e| !e.is_dir).collect())
    }

    fn entry_for(&self, rel: &str, abs: &Path) -> Result<VaultEntry> {
        let meta = fs::metadata(abs)?;
        let name = abs.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let extension = if meta.is_dir() {
            String::new()
        } else {
            abs.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
        };
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Ok(VaultEntry {
            path: rel.to_string(),
            name,
            extension,
            is_dir: meta.is_dir(),
            size: if meta.is_dir() { 0 } else { meta.len() },
            modified,
        })
    }

    pub fn entry(&self, rel: &str) -> Result<VaultEntry> {
        self.entry_for(rel, &self.resolve(rel)?)
    }

    pub fn read(&self, rel: &str) -> Result<String> {
        Ok(fs::read_to_string(self.resolve(rel)?)?)
    }

    /// Writes atomically: content goes to a temp file first which is then
    /// renamed over the target, so a crash never leaves a half-written note.
    pub fn write(&self, rel: &str, content: &str) -> Result<VaultEntry> {
        let path = self.resolve(rel)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let tmp = path.with_file_name(format!(".{file_name}.dvoix-tmp"));
        fs::write(&tmp, content)?;
        fs::rename(&tmp, &path)?;
        self.entry(rel)
    }

    /// Creates a new file and fails if it already exists.
    pub fn create(&self, rel: &str, content: &str) -> Result<VaultEntry> {
        if self.resolve(rel)?.exists() {
            return Err(Error::AlreadyExists(rel.to_string()));
        }
        self.write(rel, content)
    }

    pub fn create_dir(&self, rel: &str) -> Result<VaultEntry> {
        let path = self.resolve(rel)?;
        if path.exists() {
            return Err(Error::AlreadyExists(rel.to_string()));
        }
        fs::create_dir_all(&path)?;
        self.entry(rel)
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<VaultEntry> {
        let src = self.resolve(from)?;
        let dst = self.resolve(to)?;
        if dst.exists() {
            return Err(Error::AlreadyExists(to.to_string()));
        }
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(src, dst)?;
        self.entry(to)
    }

    /// Moves a file or folder into the vault's `.trash` folder instead of
    /// deleting it for good.
    pub fn trash(&self, rel: &str) -> Result<()> {
        let src = self.resolve(rel)?;
        let trash = self.root.join(TRASH_DIR);
        fs::create_dir_all(&trash)?;
        let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut dst = trash.join(&name);
        let mut n = 1;
        while dst.exists() {
            dst = trash.join(format!("{n} {name}"));
            n += 1;
        }
        fs::rename(src, dst)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_paths_outside_vault() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        assert!(vault.resolve("../secret.md").is_err());
        assert!(vault.resolve("/etc/passwd").is_err());
        assert!(vault.resolve("notes/ok.md").is_ok());
    }

    #[test]
    fn write_list_rename_trash() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        vault.write("projects/alpha.md", "# Alpha").unwrap();
        vault.write("inbox.md", "hi").unwrap();

        let paths: Vec<_> = vault.list().unwrap().into_iter().map(|e| e.path).collect();
        assert_eq!(paths, vec!["inbox.md", "projects", "projects/alpha.md"]);
        assert!(!paths.iter().any(|p| p.starts_with(APP_DIR)));

        vault.rename("inbox.md", "projects/inbox.md").unwrap();
        assert_eq!(vault.read("projects/inbox.md").unwrap(), "hi");
        assert!(vault.create("projects/inbox.md", "x").is_err());

        vault.trash("projects/inbox.md").unwrap();
        assert!(vault.read("projects/inbox.md").is_err());
        assert!(dir.path().join(TRASH_DIR).join("inbox.md").exists());
    }
}
