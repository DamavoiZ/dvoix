//! SQLite index over the vault: notes, tags, links and full-text search (FTS5).
//!
//! The index lives in `<vault>/.dvoix/index.sqlite` and is only a cache. It can
//! be deleted at any time; [`Index::rebuild`] recreates it from the files.
//! (If search ever outgrows FTS5, `tantivy` can replace that part without
//! touching the rest.)

use std::path::Path;

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::markdown::{self, ParsedNote};
use crate::vault::{Vault, VaultEntry};
use crate::Result;

/// Bump this when the schema changes; an outdated index is rebuilt.
const SCHEMA_VERSION: i64 = 1;

pub struct Index {
    conn: Connection,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub path: String,
    pub title: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub path: String,
    pub title: String,
    pub frontmatter: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TagCount {
    pub tag: String,
    pub count: u32,
}

/// Which files the index understands. Other file types (canvas, boards, ...)
/// can add their own extraction here later.
pub fn is_indexable(path: &str) -> bool {
    path.to_lowercase().ends_with(".md")
}

impl Index {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let index = Self { conn };
        index.migrate()?;
        Ok(index)
    }

    pub fn open_in_memory() -> Result<Self> {
        let index = Self { conn: Connection::open_in_memory()? };
        index.migrate()?;
        Ok(index)
    }

    /// Opens the index of a vault (creating it if needed).
    pub fn for_vault(vault: &Vault) -> Result<Self> {
        Self::open(&vault.app_dir().join("index.sqlite"))
    }

    fn migrate(&self) -> Result<()> {
        let version: i64 = self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version != SCHEMA_VERSION {
            self.conn.execute_batch(
                "DROP TABLE IF EXISTS notes;
                 DROP TABLE IF EXISTS tags;
                 DROP TABLE IF EXISTS links;
                 DROP TABLE IF EXISTS notes_fts;",
            )?;
        }
        self.conn.execute_batch(&format!(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS notes (
                 path TEXT PRIMARY KEY,
                 title TEXT NOT NULL,
                 frontmatter TEXT NOT NULL,
                 modified INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS tags (
                 path TEXT NOT NULL,
                 tag TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS tags_tag ON tags(tag);
             CREATE INDEX IF NOT EXISTS tags_path ON tags(path);
             CREATE TABLE IF NOT EXISTS links (
                 source TEXT NOT NULL,
                 target TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS links_target ON links(target);
             CREATE INDEX IF NOT EXISTS links_source ON links(source);
             CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(path UNINDEXED, title, body);
             PRAGMA user_version = {SCHEMA_VERSION};"
        ))?;
        Ok(())
    }

    /// Throws everything away and re-indexes all files of the vault.
    pub fn rebuild(&mut self, vault: &Vault) -> Result<usize> {
        let files = vault.files()?;
        let tx = self.conn.transaction()?;
        tx.execute_batch("DELETE FROM notes; DELETE FROM tags; DELETE FROM links; DELETE FROM notes_fts;")?;
        let mut count = 0;
        for entry in files.iter().filter(|e| is_indexable(&e.path)) {
            let Ok(content) = vault.read(&entry.path) else { continue };
            let parsed = markdown::parse(&content, stem(&entry.path));
            write_note(&tx, entry, &parsed)?;
            count += 1;
        }
        tx.commit()?;
        Ok(count)
    }

    /// Re-indexes a single file after it changed on disk.
    pub fn update(&mut self, vault: &Vault, rel: &str) -> Result<()> {
        if !is_indexable(rel) {
            return Ok(());
        }
        let abs = vault.resolve(rel)?;
        if !abs.is_file() {
            return self.remove(rel);
        }
        let entry = vault.entry(rel)?;
        let content = vault.read(rel)?;
        let parsed = markdown::parse(&content, stem(rel));
        let tx = self.conn.transaction()?;
        delete_note(&tx, rel)?;
        write_note(&tx, &entry, &parsed)?;
        tx.commit()?;
        Ok(())
    }

    /// Removes a file (or every file below a folder) from the index.
    pub fn remove(&mut self, rel: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        delete_note(&tx, rel)?;
        let prefix = format!("{rel}/%");
        tx.execute("DELETE FROM notes WHERE path LIKE ?1", [&prefix])?;
        tx.execute("DELETE FROM tags WHERE path LIKE ?1", [&prefix])?;
        tx.execute("DELETE FROM links WHERE source LIKE ?1", [&prefix])?;
        tx.execute("DELETE FROM notes_fts WHERE path LIKE ?1", [&prefix])?;
        tx.commit()?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<SearchHit>> {
        let query = fts_query(query);
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let mut stmt = self.conn.prepare(
            "SELECT path, title, snippet(notes_fts, 2, '«', '»', ' … ', 12)
             FROM notes_fts WHERE notes_fts MATCH ?1 ORDER BY rank LIMIT ?2",
        )?;
        let hits = stmt
            .query_map(params![query, limit], |r| {
                Ok(SearchHit { path: r.get(0)?, title: r.get(1)?, snippet: r.get(2)? })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(hits)
    }

    pub fn tags(&self) -> Result<Vec<TagCount>> {
        let mut stmt = self.conn.prepare("SELECT tag, COUNT(*) FROM tags GROUP BY tag ORDER BY tag")?;
        let tags = stmt
            .query_map([], |r| Ok(TagCount { tag: r.get(0)?, count: r.get(1)? }))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(tags)
    }

    pub fn notes_with_tag(&self, tag: &str) -> Result<Vec<NoteSummary>> {
        self.summaries(
            "SELECT n.path, n.title, n.frontmatter FROM notes n JOIN tags t ON t.path = n.path
             WHERE t.tag = ?1 ORDER BY n.title",
            &tag.to_lowercase(),
        )
    }

    /// Notes that link to `rel`, either by path (`folder/note.md`) or by
    /// wikilink name (`note`), case-insensitive like Obsidian resolves them.
    pub fn backlinks(&self, rel: &str) -> Result<Vec<NoteSummary>> {
        let full = rel.to_lowercase();
        let name = stem(&full).to_string();
        let without_ext = full.strip_suffix(".md").unwrap_or(&full).to_string();
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT n.path, n.title, n.frontmatter FROM notes n JOIN links l ON l.source = n.path
             WHERE l.target IN (?1, ?2, ?3) AND n.path != ?4 ORDER BY n.title",
        )?;
        let notes = stmt
            .query_map(params![full, without_ext, name, rel], row_to_summary)?
            .collect::<std::result::Result<_, _>>()?;
        Ok(notes)
    }

    /// All notes whose frontmatter `key` equals `value`, e.g. `status = active`.
    /// This is the building block for project/tracking views.
    pub fn notes_where(&self, key: &str, value: &str) -> Result<Vec<NoteSummary>> {
        let path = format!("$.\"{}\"", key.replace('"', ""));
        let mut stmt = self.conn.prepare(
            "SELECT path, title, frontmatter FROM notes WHERE json_extract(frontmatter, ?1) = ?2 ORDER BY title",
        )?;
        let notes = stmt
            .query_map(params![path, value], row_to_summary)?
            .collect::<std::result::Result<_, _>>()?;
        Ok(notes)
    }

    fn summaries(&self, sql: &str, arg: &str) -> Result<Vec<NoteSummary>> {
        let mut stmt = self.conn.prepare(sql)?;
        let notes = stmt.query_map([arg], row_to_summary)?.collect::<std::result::Result<_, _>>()?;
        Ok(notes)
    }
}

fn row_to_summary(r: &rusqlite::Row) -> rusqlite::Result<NoteSummary> {
    let fm: String = r.get(2)?;
    Ok(NoteSummary {
        path: r.get(0)?,
        title: r.get(1)?,
        frontmatter: serde_json::from_str(&fm).unwrap_or(serde_json::Value::Null),
    })
}

fn delete_note(tx: &rusqlite::Transaction, rel: &str) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM notes WHERE path = ?1", [rel])?;
    tx.execute("DELETE FROM tags WHERE path = ?1", [rel])?;
    tx.execute("DELETE FROM links WHERE source = ?1", [rel])?;
    tx.execute("DELETE FROM notes_fts WHERE path = ?1", [rel])?;
    Ok(())
}

fn write_note(tx: &rusqlite::Transaction, entry: &VaultEntry, note: &ParsedNote) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO notes (path, title, frontmatter, modified) VALUES (?1, ?2, ?3, ?4)",
        params![entry.path, note.title, note.frontmatter.to_string(), entry.modified as i64],
    )?;
    for tag in &note.tags {
        tx.execute("INSERT INTO tags (path, tag) VALUES (?1, ?2)", params![entry.path, tag])?;
    }
    for link in &note.links {
        tx.execute("INSERT INTO links (source, target) VALUES (?1, ?2)", params![entry.path, link.to_lowercase()])?;
    }
    tx.execute(
        "INSERT INTO notes_fts (path, title, body) VALUES (?1, ?2, ?3)",
        params![entry.path, note.title, note.text],
    )?;
    Ok(())
}

/// File name without folder and extension.
pub fn stem(rel: &str) -> &str {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name)
}

/// Turns user input into a safe FTS5 query: every word becomes a quoted
/// prefix term, so special characters cannot break the query syntax.
fn fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .map(|w| format!("\"{}\"*", w.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        vault
            .write("projekte/alpha.md", "---\nstatus: active\ntags: [projekt]\n---\n# Alpha\nVerweist auf [[Beta]].")
            .unwrap();
        vault.write("beta.md", "# Beta\nEin #projekt mit Zeitplanung.").unwrap();
        vault.write("bild.canvas", "{}").unwrap();
        (dir, vault)
    }

    #[test]
    fn rebuild_and_query() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        assert_eq!(index.rebuild(&vault).unwrap(), 2);

        assert_eq!(index.tags().unwrap(), vec![TagCount { tag: "projekt".into(), count: 2 }]);
        let backlinks = index.backlinks("beta.md").unwrap();
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].path, "projekte/alpha.md");

        let active = index.notes_where("status", "active").unwrap();
        assert_eq!(active[0].title, "Alpha");

        let hits = index.search("zeitpl", 10).unwrap();
        assert_eq!(hits[0].path, "beta.md");
        assert!(index.search("\"(", 10).unwrap().is_empty());
    }

    #[test]
    fn update_and_remove() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        vault.write("beta.md", "# Beta\nJetzt #fertig").unwrap();
        index.update(&vault, "beta.md").unwrap();
        assert_eq!(index.notes_with_tag("fertig").unwrap().len(), 1);

        index.remove("projekte").unwrap();
        assert!(index.backlinks("beta.md").unwrap().is_empty());
    }
}
