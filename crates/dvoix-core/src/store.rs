//! Plugins from the plugin store: the format of the plugin list (the registry),
//! the `manifest.json` of a plugin and its files in `.dvoix/plugins/<id>/`.
//!
//! Downloading is done by the app shell; this module only checks what came in
//! and writes it, so all of it can be tested without network.
//!
//! An installed plugin is a folder `.dvoix/plugins/<id>/` with
//! `manifest.json`, `main.js` and optionally `styles.css`. The folder may also
//! hold `data.json` (the plugin's settings), which uninstalling keeps, so a
//! reinstalled plugin finds its settings again. Core plugins only ever have
//! `data.json` there, so a folder without `manifest.json` is not a store plugin.

use std::fs;

use serde::{Deserialize, Serialize};

use crate::{Error, Result, Vault};

/// Where the app reads the plugin list from.
pub const REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/DamavoiZ/dvoix-plugins/main/registry.json";

/// Only plugins in repositories of this GitHub account can be official. A
/// registry entry that says `official` for any other repository is shown as
/// community plugin, so a mistake in the list can't hand out the label.
pub const OFFICIAL_OWNER: &str = "DamavoiZ";

pub const MANIFEST_FILE: &str = "manifest.json";
pub const MAIN_FILE: &str = "main.js";
pub const STYLES_FILE: &str = "styles.css";

/// Upper limit for a downloaded file, so a broken release can't fill the disk.
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// What a plugin may use of the API (see `src/lib/plugins/store/api.ts`).
pub const PERMISSIONS: &[&str] = &["vault:read", "vault:write", "editor"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Official,
    Community,
}

/// One plugin in the registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryEntry {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    /// GitHub repository `owner/name`; the plugin's files come from its releases.
    pub repo: String,
    pub channel: Channel,
}

#[derive(Deserialize)]
struct RegistryFile {
    plugins: Vec<serde_json::Value>,
}

/// The `manifest.json` of a store plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    /// `major.minor.patch`; also the tag of the GitHub release with the files.
    pub version: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Oldest dvoix version the plugin runs on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_app_version: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// A plugin found in `.dvoix/plugins/<id>/`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPlugin {
    pub manifest: PluginManifest,
    pub has_styles: bool,
}

/// The files of one plugin release.
pub struct PluginFiles {
    pub manifest: String,
    pub main: String,
    pub styles: Option<String>,
}

fn invalid(msg: impl Into<String>) -> Error {
    Error::Plugin(msg.into())
}

/// Plugin ids are folder names and settings keys: `a-z0-9-`, not empty.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('-')
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `owner/name` with the characters GitHub allows. Keeps anything else out of
/// the download URLs.
pub fn valid_repo(repo: &str) -> bool {
    let mut parts = repo.split('/');
    let ok = |s: Option<&str>| {
        s.is_some_and(|s| {
            !s.is_empty()
                && s != "."
                && s != ".."
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        })
    };
    ok(parts.next()) && ok(parts.next()) && parts.next().is_none()
}

/// `1.2.3`, numbers only.
pub fn valid_version(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 9 && p.chars().all(|c| c.is_ascii_digit()))
}

/// Reads the registry. Broken or duplicate entries are skipped instead of
/// failing the whole list, and `official` only sticks for [`OFFICIAL_OWNER`].
pub fn parse_registry(text: &str) -> Result<Vec<RegistryEntry>> {
    let file: RegistryFile = serde_json::from_str(text)
        .map_err(|e| invalid(format!("Plugin-Liste ist kein gültiges JSON: {e}")))?;
    let mut out: Vec<RegistryEntry> = Vec::new();
    for value in file.plugins {
        let Ok(mut entry) = serde_json::from_value::<RegistryEntry>(value) else {
            continue;
        };
        if !valid_id(&entry.id) || !valid_repo(&entry.repo) || out.iter().any(|e| e.id == entry.id)
        {
            continue;
        }
        let owner = entry.repo.split('/').next().unwrap_or_default();
        if !owner.eq_ignore_ascii_case(OFFICIAL_OWNER) {
            entry.channel = Channel::Community;
        }
        out.push(entry);
    }
    Ok(out)
}

/// Reads and checks a `manifest.json`.
pub fn parse_manifest(text: &str) -> Result<PluginManifest> {
    let m: PluginManifest = serde_json::from_str(text)
        .map_err(|e| invalid(format!("manifest.json ist ungültig: {e}")))?;
    if !valid_id(&m.id) {
        return Err(invalid(format!("Ungültige Plugin-ID: {}", m.id)));
    }
    if !valid_version(&m.version) {
        return Err(invalid(format!("Ungültige Version: {}", m.version)));
    }
    if let Some(min) = &m.min_app_version
        && !valid_version(min)
    {
        return Err(invalid(format!("Ungültige minAppVersion: {min}")));
    }
    if let Some(p) = m
        .permissions
        .iter()
        .find(|p| !PERMISSIONS.contains(&p.as_str()))
    {
        return Err(invalid(format!("Unbekannte Berechtigung: {p}")));
    }
    Ok(m)
}

/// Download URL of a file of a release (`tag` = the version).
pub fn release_url(repo: &str, tag: &str, file: &str) -> Result<String> {
    if !valid_repo(repo) {
        return Err(invalid(format!("Ungültiges Repository: {repo}")));
    }
    if !valid_version(tag) {
        return Err(invalid(format!("Ungültige Version: {tag}")));
    }
    Ok(format!(
        "https://github.com/{repo}/releases/download/{tag}/{file}"
    ))
}

/// URL of the `manifest.json` of the newest release.
pub fn latest_manifest_url(repo: &str) -> Result<String> {
    if !valid_repo(repo) {
        return Err(invalid(format!("Ungültiges Repository: {repo}")));
    }
    Ok(format!(
        "https://github.com/{repo}/releases/latest/download/{MANIFEST_FILE}"
    ))
}

fn file(id: &str, name: &str) -> String {
    format!("plugins/{id}/{name}")
}

/// All store plugins in the vault, sorted by name. Folders with a broken
/// manifest are skipped (they can't be loaded anyway).
pub fn installed(vault: &Vault) -> Result<Vec<InstalledPlugin>> {
    let dir = vault.config_path("plugins")?;
    let mut out = Vec::new();
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    for entry in entries.flatten() {
        let id = entry.file_name().to_string_lossy().into_owned();
        if !valid_id(&id) || !entry.path().is_dir() {
            continue;
        }
        let Some(text) = vault.read_config(&file(&id, MANIFEST_FILE))? else {
            continue;
        };
        let Ok(manifest) = parse_manifest(&text) else {
            continue;
        };
        // The folder name is what settings and data refer to; a manifest
        // claiming another id would load under the wrong name.
        if manifest.id != id {
            continue;
        }
        let has_styles = vault.config_path(&file(&id, STYLES_FILE))?.is_file();
        out.push(InstalledPlugin {
            manifest,
            has_styles,
        });
    }
    out.sort_by(|a, b| {
        a.manifest
            .name
            .to_lowercase()
            .cmp(&b.manifest.name.to_lowercase())
    });
    Ok(out)
}

/// Writes a downloaded plugin into `.dvoix/plugins/<id>/`. `manifest.json`
/// comes last, so a plugin only counts as installed once all files are there.
/// An update replaces the files; `data.json` stays.
pub fn install(vault: &Vault, expected_id: &str, files: &PluginFiles) -> Result<PluginManifest> {
    let manifest = parse_manifest(&files.manifest)?;
    if manifest.id != expected_id {
        return Err(invalid(format!(
            "Das Plugin heißt „{}“, erwartet war „{expected_id}“",
            manifest.id
        )));
    }
    let id = &manifest.id;
    vault.write_config(&file(id, MAIN_FILE), &files.main)?;
    match &files.styles {
        Some(css) => vault.write_config(&file(id, STYLES_FILE), css)?,
        None => remove(vault, &file(id, STYLES_FILE))?,
    }
    vault.write_config(&file(id, MANIFEST_FILE), &files.manifest)?;
    Ok(manifest)
}

/// Removes the plugin's code. Its `data.json` stays, like a setting does.
pub fn uninstall(vault: &Vault, id: &str) -> Result<()> {
    if !valid_id(id) {
        return Err(invalid(format!("Ungültige Plugin-ID: {id}")));
    }
    // Manifest first: if something below fails, the rest is no plugin anymore.
    for name in [MANIFEST_FILE, MAIN_FILE, STYLES_FILE] {
        remove(vault, &file(id, name))?;
    }
    let dir = vault.config_path(&format!("plugins/{id}"))?;
    if fs::read_dir(&dir).is_ok_and(|mut d| d.next().is_none()) {
        fs::remove_dir(&dir)?;
    }
    Ok(())
}

fn remove(vault: &Vault, rel: &str) -> Result<()> {
    match fs::remove_file(vault.config_path(rel)?) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(id: &str, version: &str) -> String {
        format!(
            r#"{{"id":"{id}","name":"Kanban","version":"{version}","description":"Boards.","permissions":["vault:read"]}}"#
        )
    }

    fn files(id: &str, version: &str, styles: Option<&str>) -> PluginFiles {
        PluginFiles {
            manifest: manifest(id, version),
            main: "export default { onload() {} };".into(),
            styles: styles.map(String::from),
        }
    }

    #[test]
    fn checks_ids_repos_and_versions() {
        assert!(valid_id("kanban-board2"));
        assert!(!valid_id("Kanban"));
        assert!(!valid_id("../x"));
        assert!(!valid_id(""));
        assert!(!valid_id("-x"));
        assert!(valid_repo("DamavoiZ/dvoix-kanban"));
        assert!(!valid_repo("DamavoiZ"));
        assert!(!valid_repo("a/b/c"));
        assert!(!valid_repo("../b"));
        assert!(!valid_repo("a/b?x=1"));
        assert!(valid_version("1.10.0"));
        assert!(!valid_version("1.0"));
        assert!(!valid_version("v1.0.0"));
        assert!(!valid_version("1.0.0-beta"));
    }

    #[test]
    fn registry_skips_bad_entries_and_guards_the_official_label() {
        let text = r#"{
          "version": 1,
          "plugins": [
            {"id":"kanban","name":"Kanban","author":"damavoi","description":"Boards.","repo":"DamavoiZ/dvoix-kanban","channel":"official"},
            {"id":"fake","name":"Fake","author":"x","description":"x","repo":"someone/fake","channel":"official"},
            {"id":"kanban","name":"Twice","author":"x","description":"x","repo":"x/y","channel":"community"},
            {"id":"Bad Id","name":"x","author":"x","description":"x","repo":"x/y","channel":"community"},
            {"id":"nochannel","name":"x","author":"x","description":"x","repo":"x/y"}
          ]
        }"#;
        let list = parse_registry(text).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "kanban");
        assert_eq!(list[0].channel, Channel::Official);
        assert_eq!(list[1].id, "fake");
        assert_eq!(list[1].channel, Channel::Community);
        assert!(parse_registry("nope").is_err());
    }

    #[test]
    fn examples_in_the_repo_parse() {
        let text = include_str!("../../../docs/plugins/registry.example.json");
        assert!(!parse_registry(text).unwrap().is_empty());
        let example = include_str!("../../../docs/plugins/hello/manifest.json");
        assert_eq!(parse_manifest(example).unwrap().id, "hello");
    }

    #[test]
    fn manifest_is_checked() {
        assert!(parse_manifest(&manifest("kanban", "1.0.0")).is_ok());
        assert!(parse_manifest(&manifest("kanban", "1.0")).is_err());
        assert!(parse_manifest(&manifest("../x", "1.0.0")).is_err());
        let unknown =
            r#"{"id":"a","name":"A","version":"1.0.0","description":"x","permissions":["root"]}"#;
        assert!(parse_manifest(unknown).is_err());
    }

    #[test]
    fn urls_only_come_from_checked_parts() {
        assert_eq!(
            release_url("DamavoiZ/dvoix-kanban", "1.2.0", MAIN_FILE).unwrap(),
            "https://github.com/DamavoiZ/dvoix-kanban/releases/download/1.2.0/main.js"
        );
        assert!(release_url("a/b/../c", "1.0.0", MAIN_FILE).is_err());
        assert!(release_url("a/b", "../1", MAIN_FILE).is_err());
        assert!(latest_manifest_url("a b/c").is_err());
    }

    #[test]
    fn install_update_and_uninstall_keep_settings() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        // A core plugin's settings folder is not a store plugin.
        vault.write_config("plugins/tasks/data.json", "{}").unwrap();
        assert!(installed(&vault).unwrap().is_empty());

        install(&vault, "kanban", &files("kanban", "1.0.0", Some("a{}"))).unwrap();
        vault
            .write_config("plugins/kanban/data.json", r#"{"x":1}"#)
            .unwrap();
        let list = installed(&vault).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].manifest.version, "1.0.0");
        assert!(list[0].has_styles);

        // Update without styles removes the old stylesheet.
        install(&vault, "kanban", &files("kanban", "1.1.0", None)).unwrap();
        let list = installed(&vault).unwrap();
        assert_eq!(list[0].manifest.version, "1.1.0");
        assert!(!list[0].has_styles);

        uninstall(&vault, "kanban").unwrap();
        assert!(installed(&vault).unwrap().is_empty());
        assert_eq!(
            vault
                .read_config("plugins/kanban/data.json")
                .unwrap()
                .as_deref(),
            Some(r#"{"x":1}"#)
        );
        assert!(
            vault
                .read_config("plugins/kanban/main.js")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn install_refuses_a_different_id() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        assert!(install(&vault, "kanban", &files("tasks", "1.0.0", None)).is_err());
        assert!(
            vault
                .read_config("plugins/tasks/main.js")
                .unwrap()
                .is_none()
        );
        assert!(
            vault
                .read_config("plugins/kanban/main.js")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn uninstall_removes_an_empty_folder() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        install(&vault, "kanban", &files("kanban", "1.0.0", None)).unwrap();
        uninstall(&vault, "kanban").unwrap();
        assert!(!vault.config_path("plugins/kanban").unwrap().exists());
        assert!(uninstall(&vault, "../x").is_err());
    }

    #[test]
    fn manifest_with_wrong_folder_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        vault
            .write_config("plugins/other/manifest.json", &manifest("kanban", "1.0.0"))
            .unwrap();
        assert!(installed(&vault).unwrap().is_empty());
    }
}
