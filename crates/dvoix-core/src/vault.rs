//! Access to the vault folder. All paths exchanged with the outside world are
//! relative to the vault root and use `/` as separator, on every platform.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::{APP_DIR, Error, Result};

/// Folder (inside the vault) that deleted files are moved to, like in Obsidian.
pub const TRASH_DIR: &str = ".trash";

/// Upper bound for `Vault::list_hidden`: a hidden folder like `node_modules`
/// or `.git` can hold tens of thousands of files, and the explorer would
/// freeze on all of them. What lies beyond is simply not listed.
pub const MAX_HIDDEN_ENTRIES: usize = 10_000;

/// The entry of a path with the metadata that is already known about it.
fn entry_of(rel: &str, meta: &fs::Metadata) -> VaultEntry {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    let extension = if meta.is_dir() {
        String::new()
    } else {
        match Path::new(rel).extension().and_then(|e| e.to_str()) {
            // Most extensions are already lower-case ASCII; only then is no
            // second copy needed. Non-ASCII always goes through
            // `to_lowercase`, which knows more characters than ASCII does.
            Some(e) if e.is_ascii() && !e.bytes().any(|b| b.is_ascii_uppercase()) => {
                e.to_string()
            }
            Some(e) => e.to_lowercase(),
            None => String::new(),
        }
    };
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    VaultEntry {
        path: rel.to_string(),
        name: name.to_string(),
        extension,
        is_dir: meta.is_dir(),
        size: if meta.is_dir() { 0 } else { meta.len() },
        modified,
    }
}

/// True for names of hidden files and folders (`.git`, `.dvoix`, ...). Looks
/// at the bytes, so no copy of the name is made for every entry of a walk.
#[cfg(unix)]
fn is_dot_name(name: &std::ffi::OsStr) -> bool {
    use std::os::unix::ffi::OsStrExt;
    name.as_bytes().first() == Some(&b'.')
}

#[cfg(not(unix))]
fn is_dot_name(name: &std::ffi::OsStr) -> bool {
    name.as_encoded_bytes().first() == Some(&b'.')
}

/// The key the listings are sorted by: the path in lower case, built once
/// per entry instead of once per comparison. Plain ASCII paths (almost all)
/// only need the copy itself, without the Unicode machinery.
fn sort_key(entry: &VaultEntry) -> String {
    if entry.path.is_ascii() {
        entry.path.to_ascii_lowercase()
    } else {
        entry.path.to_lowercase()
    }
}

/// Which hidden entries `Vault::list_hidden` returns. Set by the
/// "Hidden files" plugin from its settings.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HiddenFilter {
    /// Also dot-files (`.gitignore`), not only dot-folders and their content.
    pub include_files: bool,
    /// Also dvoix's own `.dvoix/` folder.
    pub include_app_dir: bool,
    /// Names that stay hidden anyway, wherever they occur (e.g. `.git`).
    pub exclude: Vec<String>,
}

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
        let mut out = String::with_capacity(rel.as_os_str().len());
        for part in rel.components() {
            if !out.is_empty() {
                out.push('/');
            }
            out.push_str(&part.as_os_str().to_string_lossy());
        }
        if out.is_empty() {
            return None;
        }
        Some(out)
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
            .filter_entry(|e| !is_dot_name(e.file_name()));
        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            // The metadata of the walk is used, so nothing has to be asked
            // from the file system a second time.
            let Some(rel) = self.relative(entry.path()) else {
                continue;
            };
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            entries.push(entry_of(&rel, &meta));
        }
        // Plain path order; the UI builds the tree and decides how to group it.
        entries.sort_by_cached_key(sort_key);
        Ok(entries)
    }

    /// Only the files below the folder `rel`, sorted by path. The folder
    /// itself is not in the list. Nothing if `rel` is not a folder of the
    /// vault, so a caller does not have to check.
    pub fn files_below(&self, rel: &str) -> Result<Vec<VaultEntry>> {
        let dir = self.resolve(rel)?;
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        let walker = WalkDir::new(&dir)
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| !is_dot_name(e.file_name()));
        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let Some(path) = self.relative(entry.path()) else {
                continue;
            };
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            entries.push(entry_of(&path, &meta));
        }
        entries.sort_by_cached_key(sort_key);
        Ok(entries)
    }

    /// The hidden entries that `list` leaves out: dot-folders with everything
    /// in them and (with `include_files`) dot-files, sorted by path. Only for
    /// showing them; they are never indexed. Temp files of `write` are never
    /// returned, and the list stops at `MAX_HIDDEN_ENTRIES`.
    pub fn list_hidden(&self, filter: &HiddenFilter) -> Result<Vec<VaultEntry>> {
        let mut entries = Vec::new();
        let walker = WalkDir::new(&self.root)
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name();
                if filter.exclude.iter().any(|x| x.as_str() == name) {
                    return false;
                }
                let name = name.to_string_lossy();
                if name.ends_with(".dvoix-tmp") {
                    return false;
                }
                !(e.depth() == 1 && name == APP_DIR && !filter.include_app_dir)
            });
        for entry in walker {
            let Ok(entry) = entry else { continue };
            let Some(rel) = self.relative(entry.path()) else {
                continue;
            };
            if !Self::is_ignored(&rel) {
                continue;
            }
            // A dot-file outside a hidden folder only shows with `include_files`.
            let in_hidden_dir = rel
                .rsplit_once('/')
                .is_some_and(|(parent, _)| Self::is_ignored(parent));
            if !entry.file_type().is_dir() && !in_hidden_dir && !filter.include_files {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            entries.push(entry_of(&rel, &meta));
            if entries.len() >= MAX_HIDDEN_ENTRIES {
                break;
            }
        }
        entries.sort_by_cached_key(sort_key);
        Ok(entries)
    }

    /// Only the files of the vault (no folders).
    pub fn files(&self) -> Result<Vec<VaultEntry>> {
        Ok(self.list()?.into_iter().filter(|e| !e.is_dir).collect())
    }

    fn entry_for(&self, rel: &str, abs: &Path) -> Result<VaultEntry> {
        let meta = fs::metadata(abs)?;
        Ok(entry_of(rel, &meta))
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
        self.write_bytes(rel, content.as_bytes())
    }

    /// Reads any file as bytes (images and other attachments).
    pub fn read_bytes(&self, rel: &str) -> Result<Vec<u8>> {
        Ok(fs::read(self.resolve(rel)?)?)
    }

    /// Writes bytes atomically, the same way as `write`.
    pub fn write_bytes(&self, rel: &str, content: &[u8]) -> Result<VaultEntry> {
        let path = self.resolve(rel)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let tmp = path.with_file_name(format!(".{file_name}.dvoix-tmp"));
        fs::write(&tmp, content)?;
        fs::rename(&tmp, &path)?;
        self.entry(rel)
    }

    /// Creates a new binary file (an attachment) and fails if it already exists.
    pub fn create_bytes(&self, rel: &str, content: &[u8]) -> Result<VaultEntry> {
        if self.resolve(rel)?.exists() {
            return Err(Error::AlreadyExists(rel.to_string()));
        }
        self.write_bytes(rel, content)
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

    /// Resolves a path inside the vault's `.dvoix/` config folder, e.g.
    /// `app.json` or `plugins/tasks/data.json`. Only plain names are allowed:
    /// no `..`, no absolute paths, no hidden parts.
    pub fn config_path(&self, rel: &str) -> Result<PathBuf> {
        let mut out = self.app_dir();
        let mut parts = 0;
        for c in Path::new(rel).components() {
            match c {
                Component::Normal(part) if !part.to_string_lossy().starts_with('.') => {
                    out.push(part);
                    parts += 1;
                }
                _ => return Err(Error::OutsideVault(rel.to_string())),
            }
        }
        if parts == 0 {
            return Err(Error::OutsideVault(rel.to_string()));
        }
        Ok(out)
    }

    /// Reads a config file from `.dvoix/`; `None` if it does not exist yet.
    pub fn read_config(&self, rel: &str) -> Result<Option<String>> {
        match fs::read_to_string(self.config_path(rel)?) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Writes a config file in `.dvoix/` atomically (temp file, then rename).
    pub fn write_config(&self, rel: &str, content: &str) -> Result<()> {
        let path = self.config_path(rel)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let tmp = path.with_file_name(format!(".{file_name}.dvoix-tmp"));
        fs::write(&tmp, content)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Moves a file or folder into the vault's `.trash` folder instead of
    /// deleting it for good.
    pub fn trash(&self, rel: &str) -> Result<()> {
        let src = self.resolve(rel)?;
        let trash = self.root.join(TRASH_DIR);
        fs::create_dir_all(&trash)?;
        let name = src
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
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
    fn config_files_live_in_app_dir_and_stay_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        assert_eq!(vault.read_config("app.json").unwrap(), None);
        vault.write_config("plugins/tasks/data.json", "{}").unwrap();
        assert_eq!(
            vault
                .read_config("plugins/tasks/data.json")
                .unwrap()
                .as_deref(),
            Some("{}")
        );
        assert!(
            dir.path()
                .join(APP_DIR)
                .join("plugins/tasks/data.json")
                .exists()
        );
        assert!(vault.list().unwrap().is_empty());

        assert!(vault.write_config("../escape.json", "x").is_err());
        assert!(vault.write_config("/etc/x.json", "x").is_err());
        assert!(vault.write_config("plugins/.hidden", "x").is_err());
        assert!(vault.write_config("", "x").is_err());
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

    #[test]
    fn binary_files_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        let png = [0x89, b'P', b'N', b'G', 0, 0xff, 0x10];
        vault.create_bytes("assets/image.png", &png).unwrap();
        assert_eq!(vault.read_bytes("assets/image.png").unwrap(), png);
        assert!(vault.create_bytes("assets/image.png", &png).is_err());
        assert!(vault.create_bytes("../escape.png", &png).is_err());
    }

    #[test]
    fn lists_the_files_below_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        vault.write("work/one.md", "1").unwrap();
        vault.write("work/deep/two.md", "2").unwrap();
        vault.write("other/three.md", "3").unwrap();
        vault.write("top.md", "4").unwrap();

        let paths: Vec<_> = vault
            .files_below("work")
            .unwrap()
            .iter()
            .map(|e| e.path.clone())
            .collect();
        assert_eq!(paths, vec!["work/deep/two.md", "work/one.md"]);

        // A folder that does not exist gives nothing, not an error.
        assert!(vault.files_below("nothing").unwrap().is_empty());
        // A file is not a folder.
        assert!(vault.files_below("top.md").unwrap().is_empty());
    }

    /// The listing stays in lower-cased path order, whatever case the names
    /// were written in, and the extension is always lower-case.
    #[test]
    fn lists_in_lowercase_path_order_with_lowercase_extensions() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        vault.write("Zebra.MD", "z").unwrap();
        vault.write("apple.md", "a").unwrap();
        vault.write("Mango.Txt", "m").unwrap();

        let entries = vault.list().unwrap();
        let paths: Vec<_> = entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["apple.md", "Mango.Txt", "Zebra.MD"]);
        let extensions: Vec<_> = entries.iter().map(|e| e.extension.as_str()).collect();
        assert_eq!(extensions, vec!["md", "txt", "md"]);

        // `files` keeps the same order, without the folders.
        let files: Vec<_> = vault.files().unwrap().into_iter().map(|e| e.path).collect();
        assert_eq!(files, vec!["apple.md", "Mango.Txt", "Zebra.MD"]);
    }

    #[test]
    fn lists_hidden_entries_by_filter() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        vault.write("note.md", "x").unwrap();
        vault.write(".obsidian/app.json", "{}").unwrap();
        vault.write(".git/HEAD", "ref").unwrap();
        vault.write("sub/.gitignore", "*.tmp").unwrap();
        vault.write_config("app.json", "{}").unwrap();
        fs::write(dir.path().join(".obsidian/.app.json.dvoix-tmp"), "x").unwrap();

        let paths = |f: &HiddenFilter| -> Vec<String> {
            vault
                .list_hidden(f)
                .unwrap()
                .into_iter()
                .map(|e| e.path)
                .collect()
        };
        let mut filter = HiddenFilter {
            exclude: vec![".git".into()],
            ..Default::default()
        };
        assert_eq!(paths(&filter), vec![".obsidian", ".obsidian/app.json"]);

        filter.include_files = true;
        filter.include_app_dir = true;
        filter.exclude.clear();
        assert_eq!(
            paths(&filter),
            vec![
                ".dvoix",
                ".dvoix/app.json",
                ".git",
                ".git/HEAD",
                ".obsidian",
                ".obsidian/app.json",
                "sub/.gitignore",
            ]
        );
        // The normal listing is unchanged.
        let visible: Vec<_> = vault.list().unwrap().into_iter().map(|e| e.path).collect();
        assert_eq!(visible, vec!["note.md", "sub"]);
    }
}
