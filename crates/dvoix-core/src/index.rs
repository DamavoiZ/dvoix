//! SQLite index over the vault: notes, tags, links and full-text search (FTS5).
//!
//! The index lives in `<vault>/.dvoix/index.sqlite` and is only a cache. It can
//! be deleted at any time; [`Index::rebuild`] recreates it from the files.
//! (If search ever outgrows FTS5, `tantivy` can replace that part without
//! touching the rest.)

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::Result;
use crate::markdown::{self, ParsedNote};
use crate::project::{self, ProjectMeta};
use crate::tasks::{self, Task};
use crate::vault::{Vault, VaultEntry};
use crate::watcher::{ChangeKind, VaultChange};

/// Bump this when the schema changes; an outdated index is rebuilt.
const SCHEMA_VERSION: i64 = 4;

pub struct Index {
    conn: Connection,
    /// How many transactions the index has written. Only the tests read it.
    #[cfg_attr(not(test), allow(dead_code))]
    writes: usize,
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

/// A task together with the note it lives in.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub path: String,
    /// Title of the note the task is in.
    pub note_title: String,
    #[serde(flatten)]
    pub task: Task,
}

/// A project note with its frontmatter fields and how many of its tasks are done.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub path: String,
    pub title: String,
    #[serde(flatten)]
    pub meta: ProjectMeta,
    pub open_tasks: u32,
    pub done_tasks: u32,
    /// Earliest due date among the open tasks, if any.
    pub next_due: Option<String>,
}

/// A note as a node of the graph view.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphNote {
    pub path: String,
    pub title: String,
    pub tags: Vec<String>,
}

/// A link as written in a note. `target` is the raw (lower-cased) link text;
/// the UI resolves it against the file list, so links to attachments and to
/// notes that don't exist yet can be shown too.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphLink {
    pub source: String,
    pub target: String,
}

/// Everything the graph view needs, in one round trip.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphData {
    pub notes: Vec<GraphNote>,
    pub links: Vec<GraphLink>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TagCount {
    pub tag: String,
    pub count: u32,
}

/// Which files the index understands. Other file types (canvas, boards, ...)
/// can add their own extraction here later.
pub fn is_indexable(path: &str) -> bool {
    // Hidden folders stay out even when a note in them is opened and saved
    // (the "Hidden files" plugin can show them).
    if Vault::is_ignored(path) {
        return false;
    }
    // Only the extension is compared, so the whole path is not lower-cased.
    let Some((_, ext)) = path.rsplit_once('.') else {
        return false;
    };
    ext.eq_ignore_ascii_case("md")
}

impl Index {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let index = Self { conn, writes: 0 };
        index.migrate()?;
        Ok(index)
    }

    pub fn open_in_memory() -> Result<Self> {
        let index = Self {
            conn: Connection::open_in_memory()?,
            writes: 0,
        };
        index.migrate()?;
        Ok(index)
    }

    /// Opens the index of a vault (creating it if needed).
    pub fn for_vault(vault: &Vault) -> Result<Self> {
        Self::open(&vault.app_dir().join("index.sqlite"))
    }

    fn migrate(&self) -> Result<()> {
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version != SCHEMA_VERSION {
            self.conn.execute_batch(
                "DROP TABLE IF EXISTS notes;
                 DROP TABLE IF EXISTS tags;
                 DROP TABLE IF EXISTS links;
                 DROP TABLE IF EXISTS notes_fts;
                 DROP TABLE IF EXISTS tasks;",
            )?;
        }
        self.conn.execute_batch(&format!(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS notes (
                 path TEXT PRIMARY KEY,
                 title TEXT NOT NULL,
                 frontmatter TEXT NOT NULL,
                 modified INTEGER NOT NULL,
                 size INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS notes_nocase ON notes(path COLLATE NOCASE);
             CREATE TABLE IF NOT EXISTS tags (
                 path TEXT NOT NULL,
                 tag TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS tags_tag ON tags(tag);
             CREATE INDEX IF NOT EXISTS tags_path ON tags(path);
             CREATE INDEX IF NOT EXISTS tags_nocase ON tags(path COLLATE NOCASE);
             CREATE TABLE IF NOT EXISTS links (
                 source TEXT NOT NULL,
                 target TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS links_target ON links(target);
             CREATE INDEX IF NOT EXISTS links_source ON links(source);
             CREATE INDEX IF NOT EXISTS links_nocase ON links(source COLLATE NOCASE);
             CREATE TABLE IF NOT EXISTS tasks (
                 path TEXT NOT NULL,
                 line INTEGER NOT NULL,
                 done INTEGER NOT NULL,
                 text TEXT NOT NULL,
                 due TEXT,
                 priority TEXT,
                 tags TEXT NOT NULL,
                 indent INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS tasks_path ON tasks(path);
             CREATE INDEX IF NOT EXISTS tasks_nocase ON tasks(path COLLATE NOCASE);
             CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(path UNINDEXED, title, body);
             PRAGMA user_version = {SCHEMA_VERSION};"
        ))?;
        Ok(())
    }

    /// Throws everything away and re-indexes all files of the vault.
    pub fn rebuild(&mut self, vault: &Vault) -> Result<usize> {
        let files = vault.files()?;
        let tx = self.conn.transaction()?;
        tx.execute_batch("DELETE FROM notes; DELETE FROM tags; DELETE FROM links; DELETE FROM notes_fts; DELETE FROM tasks;")?;
        let mut count = 0;
        for entry in files.iter().filter(|e| is_indexable(&e.path)) {
            let Ok(content) = vault.read(&entry.path) else {
                continue;
            };
            let parsed = markdown::parse(&content, stem(&entry.path));
            write_note(&tx, entry, &parsed, &tasks::extract(&content))?;
            count += 1;
        }
        tx.commit()?;
        self.writes += 1;
        Ok(count)
    }

    /// Brings the index in line with the files of the vault, without reading
    /// the notes that did not change. A note is read again when its moment of
    /// change or its size differs from what the index knows; a note whose
    /// file is gone is taken out of the index.
    ///
    /// Returns how many notes were read again.
    pub fn sync(&mut self, vault: &Vault) -> Result<usize> {
        let files = vault.files()?;
        let tx = self.conn.transaction()?;

        // What the index knows about every note.
        let known: HashMap<String, (i64, i64)> = {
            let mut stmt = tx.prepare("SELECT path, modified, size FROM notes")?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })?;
            let mut map = HashMap::new();
            for row in rows {
                let (path, modified, size) = row?;
                map.insert(path, (modified, size));
            }
            map
        };

        let mut seen: HashSet<&str> = HashSet::new();
        let mut count = 0;
        for entry in files.iter().filter(|e| is_indexable(&e.path)) {
            seen.insert(entry.path.as_str());
            let state = (entry.modified as i64, entry.size as i64);
            if known.get(&entry.path) == Some(&state) {
                continue; // the note did not change
            }
            let Ok(content) = vault.read(&entry.path) else {
                continue;
            };
            let parsed = markdown::parse(&content, stem(&entry.path));
            delete_note(&tx, &entry.path)?;
            write_note(&tx, entry, &parsed, &tasks::extract(&content))?;
            count += 1;
        }

        // Notes whose file is gone.
        for path in known.keys() {
            if !seen.contains(path.as_str()) {
                delete_note(&tx, path)?;
            }
        }

        tx.commit()?;
        self.writes += 1;
        Ok(count)
    }

    /// Moves the notes of a folder to its new name, after the folder was
    /// renamed on disk. Their content does not change, only their paths, so
    /// only the paths have to be written again.
    pub fn move_folder(&mut self, vault: &Vault, from: &str, to: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        remove_prefix(&tx, &format!("{from}/"))?;
        for entry in vault
            .files_below(to)?
            .iter()
            .filter(|e| is_indexable(&e.path))
        {
            let Ok(content) = vault.read(&entry.path) else {
                continue;
            };
            let parsed = markdown::parse(&content, stem(&entry.path));
            write_note(&tx, entry, &parsed, &tasks::extract(&content))?;
        }
        tx.commit()?;
        self.writes += 1;
        Ok(())
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
        write_note(&tx, &entry, &parsed, &tasks::extract(&content))?;
        tx.commit()?;
        self.writes += 1;
        Ok(())
    }

    /// Applies a batch of changes to the index, all in one transaction.
    ///
    /// A change for a file that is gone takes its note out of the index, a
    /// change for a file that exists reads it again. A change for a file
    /// whose moment of change and size are already known is left alone, so
    /// a note that dvoix wrote itself is not read a second time.
    pub fn apply_changes(&mut self, vault: &Vault, changes: &[VaultChange]) -> Result<()> {
        let actions = self.actions_for(vault, changes)?;
        if actions.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        for action in actions {
            match action {
                Action::Remove(path) => remove_note(&tx, path)?,
                Action::ReindexFolder(path) => {
                    // A folder that appeared (moved or renamed into place)
                    // brings its notes; the watcher reports only the folder.
                    for entry in vault.files_below(path)?.iter().filter(|e| is_indexable(&e.path)) {
                        let Ok(content) = vault.read(&entry.path) else {
                            continue;
                        };
                        let parsed = markdown::parse(&content, stem(&entry.path));
                        delete_note(&tx, &entry.path)?;
                        write_note(&tx, entry, &parsed, &tasks::extract(&content))?;
                    }
                }
                Action::Reindex(path) => {
                    let entry = vault.entry(path)?;
                    let content = vault.read(path)?;
                    let parsed = markdown::parse(&content, stem(path));
                    delete_note(&tx, path)?;
                    write_note(&tx, &entry, &parsed, &tasks::extract(&content))?;
                }
            }
        }
        tx.commit()?;
        self.writes += 1;
        Ok(())
    }

    /// What a batch of changes means for the index.
    fn actions_for<'c>(
        &self,
        vault: &Vault,
        changes: &'c [VaultChange],
    ) -> Result<Vec<Action<'c>>> {
        let mut actions = Vec::new();
        for change in changes {
            if change.hidden {
                continue;
            }
            match change.kind {
                ChangeKind::Removed => actions.push(Action::Remove(&change.path)),
                ChangeKind::Created | ChangeKind::Modified => {
                    if !is_indexable(&change.path) {
                        let path = vault.resolve(&change.path)?;
                        if path.is_dir() {
                            // A folder that exists is never taken out: after a
                            // rename its notes were just moved here. Its notes
                            // are read (again), which is cheap next to losing them.
                            if change.kind == ChangeKind::Created {
                                actions.push(Action::ReindexFolder(&change.path));
                            }
                        } else if !path.exists() && self.knows(&change.path)? {
                            // A folder that was indexed and is gone.
                            actions.push(Action::Remove(&change.path));
                        }
                        continue;
                    }
                    if !vault.resolve(&change.path)?.is_file() {
                        actions.push(Action::Remove(&change.path));
                        continue;
                    }
                    let entry = vault.entry(&change.path)?;
                    let state = (entry.modified as i64, entry.size as i64);
                    if self.state_of(&change.path)? != Some(state) {
                        actions.push(Action::Reindex(&change.path));
                    }
                }
            }
        }
        Ok(actions)
    }

    /// The moment of change and the size the index knows about a note.
    fn state_of(&self, rel: &str) -> Result<Option<(i64, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT modified, size FROM notes WHERE path = ?1")?;
        let mut rows = stmt.query([rel])?;
        Ok(match rows.next()? {
            Some(row) => Some((row.get(0)?, row.get(1)?)),
            None => None,
        })
    }

    /// True when the index holds a note with this path or below it.
    fn knows(&self, rel: &str) -> Result<bool> {
        let mut stmt = self.conn.prepare(
            "SELECT EXISTS(SELECT 1 FROM notes WHERE path = ?1 OR path LIKE ?2)",
        )?;
        let found: i64 = stmt.query_row([rel, &format!("{rel}/%")], |r| r.get(0))?;
        Ok(found != 0)
    }

    /// Removes a file (or every file below a folder) from the index.
    pub fn remove(&mut self, rel: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        remove_note(&tx, rel)?;
        tx.commit()?;
        self.writes += 1;
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
                Ok(SearchHit {
                    path: r.get(0)?,
                    title: r.get(1)?,
                    snippet: r.get(2)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(hits)
    }

    pub fn tags(&self) -> Result<Vec<TagCount>> {
        let mut stmt = self
            .conn
            .prepare("SELECT tag, COUNT(*) FROM tags GROUP BY tag ORDER BY tag")?;
        let tags = stmt
            .query_map([], |r| {
                Ok(TagCount {
                    tag: r.get(0)?,
                    count: r.get(1)?,
                })
            })?
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

    /// Every task of the vault, ordered by note and line. Filtering (open,
    /// due, ...) happens in the UI; a personal vault has few enough tasks.
    pub fn tasks(&self) -> Result<Vec<TaskItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.path, n.title, t.line, t.done, t.text, t.due, t.priority, t.tags, t.indent
             FROM tasks t JOIN notes n ON n.path = t.path ORDER BY n.path, t.line",
        )?;
        let items = stmt
            .query_map([], |r| {
                let tags: String = r.get(7)?;
                let priority: Option<String> = r.get(6)?;
                Ok(TaskItem {
                    path: r.get(0)?,
                    note_title: r.get(1)?,
                    task: Task {
                        line: r.get(2)?,
                        done: r.get(3)?,
                        text: r.get(4)?,
                        due: r.get(5)?,
                        priority: priority.as_deref().and_then(tasks::Priority::parse),
                        tags: serde_json::from_str(&tags).unwrap_or_default(),
                        indent: r.get(8)?,
                    },
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(items)
    }

    /// All notes with `type: project` in their frontmatter, with task progress.
    pub fn projects(&self) -> Result<Vec<ProjectSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT n.path, n.title, n.frontmatter,
                    COALESCE(SUM(t.done = 0), 0), COALESCE(SUM(t.done = 1), 0),
                    MIN(CASE WHEN t.done = 0 THEN t.due END)
             FROM notes n LEFT JOIN tasks t ON t.path = n.path
             WHERE lower(json_extract(n.frontmatter, '$.type')) IN ('projekt', 'project')
             GROUP BY n.path ORDER BY n.title",
        )?;
        let rows = stmt.query_map([], |r| {
            let fm: String = r.get(2)?;
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                fm,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?;
        let mut projects = Vec::new();
        for row in rows {
            let (path, title, fm, open_tasks, done_tasks, next_due) = row?;
            let fm: serde_json::Value = serde_json::from_str(&fm).unwrap_or_default();
            if let Some(meta) = project::from_frontmatter(&fm) {
                projects.push(ProjectSummary {
                    path,
                    title,
                    meta,
                    open_tasks,
                    done_tasks,
                    next_due,
                });
            }
        }
        Ok(projects)
    }

    /// All notes with their tags, and all links between files, for the graph view.
    pub fn graph(&self) -> Result<GraphData> {
        let mut data = GraphData::default();
        let mut by_path = std::collections::HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT path, title FROM notes ORDER BY path")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get(1)?)))? {
            let (path, title) = row?;
            by_path.insert(path.clone(), data.notes.len());
            data.notes.push(GraphNote {
                path,
                title,
                tags: Vec::new(),
            });
        }
        let mut stmt = self
            .conn
            .prepare("SELECT path, tag FROM tags ORDER BY path, tag")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get(1)?)))? {
            let (path, tag) = row?;
            if let Some(&i) = by_path.get(&path) {
                data.notes[i].tags.push(tag);
            }
        }
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT source, target FROM links ORDER BY source, target")?;
        data.links = stmt
            .query_map([], |r| {
                Ok(GraphLink {
                    source: r.get(0)?,
                    target: r.get(1)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(data)
    }

    fn summaries(&self, sql: &str, arg: &str) -> Result<Vec<NoteSummary>> {
        let mut stmt = self.conn.prepare(sql)?;
        let notes = stmt
            .query_map([arg], row_to_summary)?
            .collect::<std::result::Result<_, _>>()?;
        Ok(notes)
    }

    /// The number of notes and the number of search rows, so the tests can
    /// see whether the two tables stay in step.
    #[cfg(test)]
    fn counts(&self) -> Result<(i64, i64)> {
        let notes = self
            .conn
            .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))?;
        let fts = self
            .conn
            .query_row("SELECT count(*) FROM notes_fts", [], |r| r.get(0))?;
        Ok((notes, fts))
    }

    /// The database connection, for the tests that look at the query plan.
    #[cfg(test)]
    fn conn(&self) -> &Connection {
        &self.conn
    }

    /// How many transactions the index has written.
    #[cfg(test)]
    fn writes(&self) -> usize {
        self.writes
    }

    /// Everything the index holds, as text, so two indexes can be compared.
    #[cfg(test)]
    fn dump(&self) -> String {
        let mut out = String::new();
        for (sql, title) in [
            ("SELECT path, title, frontmatter FROM notes ORDER BY path", "notes"),
            ("SELECT path, tag FROM tags ORDER BY path, tag", "tags"),
            ("SELECT source, target FROM links ORDER BY source, target", "links"),
            (
                "SELECT path, line, done, text, due, priority, tags, indent FROM tasks
                 ORDER BY path, line",
                "tasks",
            ),
            (
                "SELECT notes_fts.path, title, body FROM notes_fts ORDER BY path",
                "search",
            ),
        ] {
            let mut stmt = self.conn.prepare(sql).unwrap();
            let mut rows = stmt.query([]).unwrap();
            out.push_str(title);
            out.push('\n');
            while let Some(row) = rows.next().unwrap() {
                for i in 0..row.as_ref().column_count() {
                    let value = match row.get_ref(i).unwrap() {
                        rusqlite::types::ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
                        rusqlite::types::ValueRef::Integer(i) => i.to_string(),
                        rusqlite::types::ValueRef::Real(f) => f.to_string(),
                        rusqlite::types::ValueRef::Null => String::new(),
                        rusqlite::types::ValueRef::Blob(b) => b.len().to_string(),
                    };
                    out.push_str(&value);
                    out.push('\t');
                }
                out.push('\n');
            }
        }
        out
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

/// What a change of the vault means for the index.
enum Action<'a> {
    /// The note (and everything below it) goes out of the index.
    Remove(&'a str),
    /// The note is read again and written to the index.
    Reindex(&'a str),
    /// Every note below the folder is read and written to the index.
    ReindexFolder(&'a str),
}

/// Takes a note out of the index, and everything below it when it is a
/// folder.
fn remove_note(tx: &rusqlite::Transaction, rel: &str) -> rusqlite::Result<()> {
    delete_note(tx, rel)?;
    remove_prefix(tx, &format!("{rel}/"))
}

/// Removes every note below `prefix` from the index.
fn remove_prefix(tx: &rusqlite::Transaction, prefix: &str) -> rusqlite::Result<()> {
    let pattern = format!("{prefix}%");
    // The search rows go first: their rowids come from the notes below the
    // folder, which are deleted right after.
    tx.execute(
        "DELETE FROM notes_fts WHERE rowid IN (SELECT rowid FROM notes WHERE path LIKE ?1)",
        [&pattern],
    )?;
    tx.execute("DELETE FROM notes WHERE path LIKE ?1", [&pattern])?;
    tx.execute("DELETE FROM tags WHERE path LIKE ?1", [&pattern])?;
    tx.execute("DELETE FROM links WHERE source LIKE ?1", [&pattern])?;
    tx.execute("DELETE FROM tasks WHERE path LIKE ?1", [&pattern])?;
    Ok(())
}

fn delete_note(tx: &rusqlite::Transaction, rel: &str) -> rusqlite::Result<()> {
    // `path` is UNINDEXED in the FTS table, so a delete by path would scan
    // the whole table. The rowid of the note is looked up first and used to
    // delete the FTS row directly.
    let rowid: Option<i64> = tx
        .query_row("SELECT rowid FROM notes WHERE path = ?1", [rel], |r| r.get(0))
        .ok();
    if let Some(rowid) = rowid {
        tx.execute("DELETE FROM notes_fts WHERE rowid = ?1", [rowid])?;
    }
    tx.execute("DELETE FROM notes WHERE path = ?1", [rel])?;
    tx.execute("DELETE FROM tags WHERE path = ?1", [rel])?;
    tx.execute("DELETE FROM links WHERE source = ?1", [rel])?;
    tx.execute("DELETE FROM tasks WHERE path = ?1", [rel])?;
    Ok(())
}

fn write_note(
    tx: &rusqlite::Transaction,
    entry: &VaultEntry,
    note: &ParsedNote,
    tasks: &[Task],
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO notes (path, title, frontmatter, modified, size) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            entry.path,
            note.title,
            note.frontmatter.to_string(),
            entry.modified as i64,
            entry.size as i64
        ],
    )?;
    // The FTS row carries the rowid of the note, so both can be deleted and
    // found again without a scan (see `delete_note`).
    let rowid = tx.last_insert_rowid();
    for tag in &note.tags {
        tx.execute(
            "INSERT INTO tags (path, tag) VALUES (?1, ?2)",
            params![entry.path, tag],
        )?;
    }
    for link in &note.links {
        // Link targets are stored lower-cased, so backlinks find them whatever
        // case they were written in. Most are already lower-case; then no
        // copy is needed.
        let lower = lower_case(link);
        tx.execute(
            "INSERT INTO links (source, target) VALUES (?1, ?2)",
            params![entry.path, lower.as_ref()],
        )?;
    }
    tx.execute(
        "INSERT INTO notes_fts (rowid, path, title, body) VALUES (?1, ?2, ?3, ?4)",
        params![rowid, entry.path, note.title, note.text],
    )?;
    for t in tasks {
        tx.execute(
            "INSERT INTO tasks (path, line, done, text, due, priority, tags, indent) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                entry.path,
                t.line,
                t.done,
                t.text,
                t.due,
                t.priority.map(|p| p.as_str()),
                serde_json::to_string(&t.tags).unwrap_or_else(|_| "[]".into()),
                t.indent
            ],
        )?;
    }
    Ok(())
}

/// File name without folder and extension.
pub fn stem(rel: &str) -> &str {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name)
}

/// The lower-case form of `text`, without a copy when it is already
/// lower-case ASCII. Non-ASCII text is always copied, because `to_lowercase`
/// knows more characters than ASCII does.
fn lower_case(text: &str) -> Cow<'_, str> {
    if text.is_ascii() && !text.bytes().any(|b| b.is_ascii_uppercase()) {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(text.to_lowercase())
    }
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
            .write(
                "projects/alpha.md",
                "---\nstatus: active\ntags: [project]\n---\n# Alpha\nLinks to [[Beta]].",
            )
            .unwrap();
        vault
            .write("beta.md", "# Beta\nA #project with a schedule.")
            .unwrap();
        vault.write("picture.canvas", "{}").unwrap();
        (dir, vault)
    }

    #[test]
    fn rebuild_and_query() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        assert_eq!(index.rebuild(&vault).unwrap(), 2);

        assert_eq!(
            index.tags().unwrap(),
            vec![TagCount {
                tag: "project".into(),
                count: 2
            }]
        );
        let backlinks = index.backlinks("beta.md").unwrap();
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].path, "projects/alpha.md");

        let active = index.notes_where("status", "active").unwrap();
        assert_eq!(active[0].title, "Alpha");

        let hits = index.search("schedule", 10).unwrap();
        assert_eq!(hits[0].path, "beta.md");
        assert!(index.search("\"(", 10).unwrap().is_empty());
    }

    #[test]
    fn notes_in_hidden_folders_are_never_indexed() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();
        vault
            .write(".obsidian/secret.md", "# Secret\nwortxyz")
            .unwrap();
        index.update(&vault, ".obsidian/secret.md").unwrap();
        assert!(index.search("wortxyz", 10).unwrap().is_empty());
    }

    #[test]
    fn tasks_and_projects() {
        let (_dir, vault) = sample_vault();
        vault
            .write(
                "projects/umzug.md",
                "---\ntype: project\nstatus: active\ndue: 2026-12-01\n---\n# Move\n- [ ] Boxes 📅 2026-11-20\n- [x] Cancel contract\n- [ ] Re-register due:2026-11-15 !high\n",
            )
            .unwrap();
        vault.write("inbox.md", "- [ ] Loose task").unwrap();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        let tasks = index.tasks().unwrap();
        assert_eq!(tasks.len(), 4);
        assert_eq!(tasks[0].path, "inbox.md");
        assert_eq!(tasks[1].note_title, "Move");
        assert_eq!(tasks[1].task.line, 6);
        assert_eq!(tasks[3].task.priority, Some(tasks::Priority::High));

        let projects = index.projects().unwrap();
        assert_eq!(projects.len(), 1);
        let p = &projects[0];
        assert_eq!((p.open_tasks, p.done_tasks), (2, 1));
        assert_eq!(p.next_due.as_deref(), Some("2026-11-15"));
        assert_eq!(p.meta.due.as_deref(), Some("2026-12-01"));

        tasks::set_done_in_vault(&vault, "projects/umzug.md", 6, "Boxes", true).unwrap();
        index.update(&vault, "projects/umzug.md").unwrap();
        assert_eq!(index.projects().unwrap()[0].done_tasks, 2);
        assert!(
            tasks::set_done_in_vault(&vault, "projects/umzug.md", 6, "Something else", false)
                .is_err()
        );

        index.remove("projects").unwrap();
        assert_eq!(index.tasks().unwrap().len(), 1);
        assert!(index.projects().unwrap().is_empty());
    }

    #[test]
    fn graph_lists_notes_tags_and_links() {
        let (_dir, vault) = sample_vault();
        vault
            .write(
                "beta.md",
                "# Beta\nA #project, see [[Alpha]] and [Picture](picture.png).",
            )
            .unwrap();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        let graph = index.graph().unwrap();
        let paths: Vec<_> = graph.notes.iter().map(|n| n.path.as_str()).collect();
        assert_eq!(paths, vec!["beta.md", "projects/alpha.md"]);
        assert_eq!(graph.notes[1].tags, vec!["project"]);
        assert_eq!(
            graph.links,
            vec![
                GraphLink {
                    source: "beta.md".into(),
                    target: "alpha".into()
                },
                GraphLink {
                    source: "beta.md".into(),
                    target: "picture.png".into()
                },
                GraphLink {
                    source: "projects/alpha.md".into(),
                    target: "beta".into()
                },
            ]
        );
    }

    #[test]
    fn update_and_remove() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        vault.write("beta.md", "# Beta\nJetzt #fertig").unwrap();
        index.update(&vault, "beta.md").unwrap();
        assert_eq!(index.notes_with_tag("fertig").unwrap().len(), 1);

        index.remove("projects").unwrap();
        assert!(index.backlinks("beta.md").unwrap().is_empty());
    }

    /// A note that was deleted must not be found by the search any more.
    #[test]
    fn a_deleted_note_leaves_no_search_hit() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();
        assert_eq!(index.search("schedule", 10).unwrap().len(), 1);

        index.remove("beta.md").unwrap();
        assert!(index.search("schedule", 10).unwrap().is_empty());
        assert_eq!(index.counts().unwrap(), (1, 1));
    }

    /// A note that was changed must be found by its new text only.
    #[test]
    fn a_changed_note_is_found_only_by_its_new_text() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();
        assert_eq!(index.search("schedule", 10).unwrap().len(), 1);

        vault
            .write("beta.md", "# Beta\nA #project with a timetable.")
            .unwrap();
        index.update(&vault, "beta.md").unwrap();

        assert!(index.search("schedule", 10).unwrap().is_empty());
        let hits = index.search("timetable", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "beta.md");
        assert_eq!(index.counts().unwrap(), (2, 2));
    }

    /// Removing a folder takes every note below it out of the index, the
    /// search index included.
    #[test]
    fn removing_a_folder_takes_all_of_its_notes_out() {
        let (_dir, vault) = sample_vault();
        vault.write("projects/gamma.md", "# Gamma\nA schedule.").unwrap();
        vault
            .write("projects/deep/delta.md", "# Delta\nAnother schedule.")
            .unwrap();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();
        assert_eq!(index.counts().unwrap(), (4, 4));

        index.remove("projects").unwrap();

        // Only `beta.md` is left; it still has its own schedule.
        assert_eq!(index.counts().unwrap(), (1, 1));
        let hits = index.search("schedule", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "beta.md");
        assert_eq!(index.notes_with_tag("project").unwrap().len(), 1);
        assert!(index.backlinks("beta.md").unwrap().is_empty());
    }

    /// The search table and the notes table must stay in step: every note
    /// has exactly one search row and no search row is left over.
    #[test]
    fn the_search_table_stays_in_step_with_the_notes() {
        let (_dir, vault) = sample_vault();
        vault.write("projects/gamma.md", "# Gamma\nA schedule.").unwrap();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        vault.write("beta.md", "# Beta\nChanged text.").unwrap();
        index.update(&vault, "beta.md").unwrap();
        index.remove("projects").unwrap();
        vault.write("new.md", "# New\nA new note.").unwrap();
        index.update(&vault, "new.md").unwrap();

        assert_eq!(index.counts().unwrap(), (2, 2));
        assert_eq!(index.conn().query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0)).unwrap(), "ok");
    }

    /// The delete of a search row must not walk the whole table: it looks the
    /// rowid up and deletes that row. A full scan shows up in the plan as
    /// `INDEX 0:`, the lookup by rowid as `INDEX 0:=`.
    #[test]
    fn the_search_delete_uses_the_rowid() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        let mut by_rowid = index
            .conn()
            .prepare("EXPLAIN QUERY PLAN DELETE FROM notes_fts WHERE rowid = ?1")
            .unwrap();
        let fast: String = by_rowid.query_row([1i64], |r| r.get(3)).unwrap();
        assert!(
            fast.contains("INDEX 0:="),
            "the search delete still walks the table: {fast}"
        );

        let mut by_path = index
            .conn()
            .prepare("EXPLAIN QUERY PLAN DELETE FROM notes_fts WHERE path = ?1")
            .unwrap();
        let slow: String = by_path.query_row(["x.md"], |r| r.get(3)).unwrap();
        assert!(
            !slow.contains("INDEX 0:="),
            "the rowid lookup is not faster than the path lookup: {slow}"
        );
    }

    /// The prefix delete of a folder must not scan the whole table either.
    #[test]
    fn the_folder_delete_uses_the_path_index() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        for table in ["notes", "tags", "links", "tasks"] {
            let column = if table == "links" { "source" } else { "path" };
            let sql = format!("EXPLAIN QUERY PLAN DELETE FROM {table} WHERE {column} LIKE ?1");
            let mut plan = index.conn().prepare(&sql).unwrap();
            let detail: String = plan.query_row(["x/%"], |r| r.get(3)).unwrap();
            assert!(
                !detail.contains(&format!("SCAN {table}")),
                "the delete on {table} still scans the table: {detail}"
            );
        }
    }

    /// Only markdown files belong in the index.
    #[test]
    fn only_markdown_files_are_indexable() {
        assert!(is_indexable("note.md"));
        assert!(is_indexable("folder/note.MD"));
        assert!(is_indexable("folder.with.dots/note.md"));
        assert!(!is_indexable("note.txt"));
        assert!(!is_indexable("note.markdown"));
        assert!(!is_indexable("folder/note.md.txt"));
        assert!(!is_indexable("folder/no-extension"));
        assert!(!is_indexable(".hidden/note.md"));
    }

    /// An index of an older version is thrown away and built again, because
    /// the search rows of the old version have no rowid of their note.
    #[test]
    fn an_index_of_an_older_version_is_built_again() {
        let (_dir, vault) = sample_vault();
        let path = vault.app_dir().join("index.sqlite");

        // An index as the version before the rowid of the note was used.
        {
            let old = Connection::open(&path).unwrap();
            old.execute_batch(
                "CREATE TABLE notes (path TEXT PRIMARY KEY, title TEXT NOT NULL,
                                     frontmatter TEXT NOT NULL, modified INTEGER NOT NULL);
                 CREATE VIRTUAL TABLE notes_fts USING fts5(path UNINDEXED, title, body);
                 INSERT INTO notes VALUES ('old.md', 'Old', '{}', 1);
                 INSERT INTO notes_fts (path, title, body) VALUES ('old.md', 'Old', 'old text');
                 PRAGMA user_version = 2;",
            )
            .unwrap();
        }

        let mut index = Index::open(&path).unwrap();
        assert_eq!(
            index.conn().query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).unwrap(),
            SCHEMA_VERSION
        );

        // The old note is gone and the vault is indexed from scratch.
        index.rebuild(&vault).unwrap();
        assert_eq!(index.counts().unwrap(), (2, 2));
        assert!(index.search("old text", 10).unwrap().is_empty());
        assert_eq!(index.search("schedule", 10).unwrap().len(), 1);
    }

    /// The index of a vault that was opened before must say the same as one
    /// that was built from scratch.
    #[test]
    fn the_sync_leaves_the_same_index_as_a_full_rebuild() {
        let (_dir, vault) = sample_vault();
        vault.write("projects/gamma.md", "# Gamma\nA #project note.").unwrap();
        vault.write("notes/deep/delta.md", "# Delta\nMore text.").unwrap();
        vault.write("inbox.md", "- [ ] A task").unwrap();

        let mut synced = Index::for_vault(&vault).unwrap();
        synced.rebuild(&vault).unwrap();
        let expected = synced.dump();

        // The same vault, but the index is built by the sync.
        let mut again = Index::for_vault(&vault).unwrap();
        again.sync(&vault).unwrap();
        assert_eq!(again.dump(), expected);

        // And once more, now that the index knows the vault.
        let mut third = Index::for_vault(&vault).unwrap();
        third.sync(&vault).unwrap();
        assert_eq!(third.dump(), expected);
    }

    /// A sync must read only the notes that changed.
    #[test]
    fn a_sync_reads_only_the_notes_that_changed() {
        let (_dir, vault) = sample_vault();
        vault.write("projects/gamma.md", "# Gamma\nA #project note.").unwrap();

        let mut index = Index::for_vault(&vault).unwrap();
        index.sync(&vault).unwrap();

        // Nothing changed, so nothing is read again.
        assert_eq!(index.sync(&vault).unwrap(), 0);

        // A new note is read.
        vault.write("new.md", "# New\nA new note.").unwrap();
        assert_eq!(index.sync(&vault).unwrap(), 1);

        // A note that was removed is taken out of the index.
        vault.trash("new.md").unwrap();
        assert_eq!(index.sync(&vault).unwrap(), 0);
        assert_eq!(index.counts().unwrap(), (3, 3));
        assert!(index.search("new note", 10).unwrap().is_empty());
    }

    /// A note that was changed on disk while the app was closed must be
    /// found again with its new text.
    #[test]
    fn a_sync_picks_up_a_note_that_changed_while_the_app_was_closed() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.sync(&vault).unwrap();
        assert_eq!(index.search("schedule", 10).unwrap().len(), 1);

        // The file changes behind the back of the index.
        vault.write("beta.md", "# Beta\nA timetable now.").unwrap();
        assert_eq!(index.sync(&vault).unwrap(), 1);

        assert!(index.search("schedule", 10).unwrap().is_empty());
        assert_eq!(index.search("timetable", 10).unwrap().len(), 1);
        assert_eq!(index.counts().unwrap(), (2, 2));
    }

    /// A folder that was renamed keeps its notes, with their new path.
    #[test]
    fn a_renamed_folder_keeps_its_notes() {        let (_dir, vault) = sample_vault();
        vault.write("projects/gamma.md", "# Gamma\nA #project note.").unwrap();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();
        assert_eq!(index.counts().unwrap(), (3, 3));

        vault.rename("projects", "work").unwrap();
        index.move_folder(&vault, "projects", "work").unwrap();

        assert_eq!(index.counts().unwrap(), (3, 3));
        let notes = index.notes_with_tag("project").unwrap();
        assert!(notes.iter().any(|n| n.path == "work/alpha.md"));
        assert!(notes.iter().any(|n| n.path == "work/gamma.md"));
        assert!(!notes.iter().any(|n| n.path.starts_with("projects/")));
        // The link of alpha to beta still works.
        assert_eq!(index.backlinks("beta.md").unwrap().len(), 1);
    }

    /// A batch of changes is written with one transaction, not one per
    /// change.
    #[test]
    fn a_batch_of_changes_costs_one_transaction() {
        let (_dir, vault) = sample_vault();
        for i in 0..20 {
            vault
                .write(&format!("batch/note-{i}.md"), &format!("# Note {i}\nA schedule."))
                .unwrap();
        }
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();
        let before = index.writes();

        let changes: Vec<VaultChange> = (0..20)
            .map(|i| VaultChange {
                kind: ChangeKind::Modified,
                path: format!("batch/note-{i}.md"),
                hidden: false,
                entry: None,
            })
            .collect();
        // The files did not change, so nothing has to be written at all.
        index.apply_changes(&vault, &changes).unwrap();
        assert_eq!(index.writes(), before, "the batch wrote although nothing changed");

        // Now the files really change.
        for i in 0..20 {
            vault
                .write(
                    &format!("batch/note-{i}.md"),
                    &format!("# Note {i}\nA timetable."),
                )
                .unwrap();
        }
        index.apply_changes(&vault, &changes).unwrap();
        assert_eq!(
            index.writes(),
            before + 1,
            "the batch did not cost one transaction"
        );
        assert_eq!(index.search("schedule", 50).unwrap().len(), 1);
        assert_eq!(index.search("timetable", 50).unwrap().len(), 20);
    }

    /// A note that dvoix wrote itself is not read a second time: the change
    /// the watcher reports about it finds the index already up to date.
    #[test]
    fn a_note_that_was_already_indexed_is_not_read_again() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        // The command writes the note and indexes it right away.
        vault.write("beta.md", "# Beta\nA timetable.").unwrap();
        index.update(&vault, "beta.md").unwrap();
        let before = index.writes();

        // The watcher reports the same change a moment later.
        let changes = vec![VaultChange {
            kind: ChangeKind::Modified,
            path: "beta.md".into(),
            hidden: false,
            entry: None,
        }];
        index.apply_changes(&vault, &changes).unwrap();
        assert_eq!(index.writes(), before, "the note was read a second time");
        assert_eq!(index.search("timetable", 10).unwrap().len(), 1);
    }

    /// A batch with changes that came and went leaves the index in step with
    /// the files.
    #[test]
    fn a_batch_of_mixed_changes_leaves_the_index_in_step() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        vault.write("new.md", "# New\nA new note.").unwrap();
        vault.trash("beta.md").unwrap();
        vault
            .write("projects/gamma.md", "# Gamma\nMore text with a #project tag.")
            .unwrap();

        let changes = vec![
            VaultChange {
                kind: ChangeKind::Created,
                path: "new.md".into(),
                hidden: false,
                entry: None,
            },
            VaultChange {
                kind: ChangeKind::Removed,
                path: "beta.md".into(),
                hidden: false,
                entry: None,
            },
            VaultChange {
                kind: ChangeKind::Created,
                path: "projects/gamma.md".into(),
                hidden: false,
                entry: None,
            },
        ];
        index.apply_changes(&vault, &changes).unwrap();

        assert_eq!(index.counts().unwrap(), (3, 3));
        assert_eq!(index.search("new note", 10).unwrap().len(), 1);
        assert!(index.search("schedule", 10).unwrap().is_empty());
        assert_eq!(index.notes_with_tag("project").unwrap().len(), 2);
    }

    /// The watcher reports a folder rename after the command already moved
    /// the notes; that report must not take them out of the index again.
    #[test]
    fn the_watcher_report_of_a_renamed_folder_keeps_its_notes() {
        let (_dir, vault) = sample_vault();
        vault.write("projects/gamma.md", "# Gamma\nA #project note.").unwrap();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        vault.rename("projects", "work").unwrap();
        index.move_folder(&vault, "projects", "work").unwrap();
        let change = |kind, path: &str| VaultChange { kind, path: path.into(), hidden: false, entry: None };
        index
            .apply_changes(&vault, &[change(ChangeKind::Removed, "projects"), change(ChangeKind::Created, "work")])
            .unwrap();

        assert_eq!(index.counts().unwrap(), (3, 3));
        assert_eq!(index.notes_with_tag("project").unwrap().len(), 3);
    }

    /// A folder that is moved into the vault from outside brings its notes.
    #[test]
    fn a_folder_moved_in_from_outside_is_indexed() {
        let (dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        std::fs::create_dir_all(dir.path().join("moved/deep")).unwrap();
        std::fs::write(dir.path().join("moved/deep/new.md"), "# New\nA moved note.").unwrap();
        let change = VaultChange { kind: ChangeKind::Created, path: "moved".into(), hidden: false, entry: None };
        index.apply_changes(&vault, &[change]).unwrap();

        assert_eq!(index.counts().unwrap(), (3, 3));
        assert_eq!(index.search("moved note", 10).unwrap().len(), 1);
    }
}
