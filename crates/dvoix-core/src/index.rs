//! SQLite index over the vault: notes, tags, links and full-text search (FTS5).
//!
//! The index lives in `<vault>/.dvoix/index.sqlite` and is only a cache. It can
//! be deleted at any time; [`Index::rebuild`] recreates it from the files.
//! (If search ever outgrows FTS5, `tantivy` can replace that part without
//! touching the rest.)

use std::path::Path;

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::Result;
use crate::markdown::{self, ParsedNote};
use crate::project::{self, ProjectMeta};
use crate::tasks::{self, Task};
use crate::vault::{Vault, VaultEntry};

/// Bump this when the schema changes; an outdated index is rebuilt.
const SCHEMA_VERSION: i64 = 2;

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
        let index = Self {
            conn: Connection::open_in_memory()?,
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
        write_note(&tx, &entry, &parsed, &tasks::extract(&content))?;
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
        tx.execute("DELETE FROM tasks WHERE path LIKE ?1", [&prefix])?;
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

    /// All notes with `type: projekt` in their frontmatter, with task progress.
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
        "INSERT INTO notes (path, title, frontmatter, modified) VALUES (?1, ?2, ?3, ?4)",
        params![
            entry.path,
            note.title,
            note.frontmatter.to_string(),
            entry.modified as i64
        ],
    )?;
    for tag in &note.tags {
        tx.execute(
            "INSERT INTO tags (path, tag) VALUES (?1, ?2)",
            params![entry.path, tag],
        )?;
    }
    for link in &note.links {
        tx.execute(
            "INSERT INTO links (source, target) VALUES (?1, ?2)",
            params![entry.path, link.to_lowercase()],
        )?;
    }
    tx.execute(
        "INSERT INTO notes_fts (path, title, body) VALUES (?1, ?2, ?3)",
        params![entry.path, note.title, note.text],
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
                "projekte/alpha.md",
                "---\nstatus: active\ntags: [projekt]\n---\n# Alpha\nVerweist auf [[Beta]].",
            )
            .unwrap();
        vault
            .write("beta.md", "# Beta\nEin #projekt mit Zeitplanung.")
            .unwrap();
        vault.write("bild.canvas", "{}").unwrap();
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
                tag: "projekt".into(),
                count: 2
            }]
        );
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
    fn tasks_and_projects() {
        let (_dir, vault) = sample_vault();
        vault
            .write(
                "projekte/umzug.md",
                "---\ntype: projekt\nstatus: aktiv\ndue: 2026-12-01\n---\n# Umzug\n- [ ] Kartons 📅 2026-11-20\n- [x] Kündigen\n- [ ] Ummelden due:2026-11-15 !hoch\n",
            )
            .unwrap();
        vault.write("inbox.md", "- [ ] Lose Aufgabe").unwrap();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        let tasks = index.tasks().unwrap();
        assert_eq!(tasks.len(), 4);
        assert_eq!(tasks[0].path, "inbox.md");
        assert_eq!(tasks[1].note_title, "Umzug");
        assert_eq!(tasks[1].task.line, 6);
        assert_eq!(tasks[3].task.priority, Some(tasks::Priority::High));

        let projects = index.projects().unwrap();
        assert_eq!(projects.len(), 1);
        let p = &projects[0];
        assert_eq!((p.open_tasks, p.done_tasks), (2, 1));
        assert_eq!(p.next_due.as_deref(), Some("2026-11-15"));
        assert_eq!(p.meta.due.as_deref(), Some("2026-12-01"));

        tasks::set_done_in_vault(&vault, "projekte/umzug.md", 6, "Kartons", true).unwrap();
        index.update(&vault, "projekte/umzug.md").unwrap();
        assert_eq!(index.projects().unwrap()[0].done_tasks, 2);
        assert!(
            tasks::set_done_in_vault(&vault, "projekte/umzug.md", 6, "Etwas anderes", false)
                .is_err()
        );

        index.remove("projekte").unwrap();
        assert_eq!(index.tasks().unwrap().len(), 1);
        assert!(index.projects().unwrap().is_empty());
    }

    #[test]
    fn graph_lists_notes_tags_and_links() {
        let (_dir, vault) = sample_vault();
        vault
            .write(
                "beta.md",
                "# Beta\nEin #projekt, siehe [[Alpha]] und [Bild](bild.png).",
            )
            .unwrap();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        let graph = index.graph().unwrap();
        let paths: Vec<_> = graph.notes.iter().map(|n| n.path.as_str()).collect();
        assert_eq!(paths, vec!["beta.md", "projekte/alpha.md"]);
        assert_eq!(graph.notes[1].tags, vec!["projekt"]);
        assert_eq!(
            graph.links,
            vec![
                GraphLink {
                    source: "beta.md".into(),
                    target: "alpha".into()
                },
                GraphLink {
                    source: "beta.md".into(),
                    target: "bild.png".into()
                },
                GraphLink {
                    source: "projekte/alpha.md".into(),
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

        index.remove("projekte").unwrap();
        assert!(index.backlinks("beta.md").unwrap().is_empty());
    }
}
