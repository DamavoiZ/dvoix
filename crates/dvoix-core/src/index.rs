//! SQLite index over the vault: notes, tags, links and full-text search (FTS5).
//!
//! The index lives in `<vault>/.dvoix/index.sqlite` and is only a cache. It can
//! be deleted at any time; [`Index::rebuild`] recreates it from the files.
//! (If search ever outgrows FTS5, `tantivy` can replace that part without
//! touching the rest.)

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::Result;
use crate::markdown::{self, ParsedNote};
use crate::project::{self, ProjectMeta};
use crate::tasks::{self, Task};
use crate::vault::{Vault, VaultEntry};
use crate::watcher::{ChangeKind, VaultChange};

/// Bump this when the schema changes; an outdated index is rebuilt.
const SCHEMA_VERSION: i64 = 6;

/// How the search table is built. `prefix='1'` keeps an extra index over the
/// first letter of every word. The search of the app matches every word as a
/// prefix (see `fts_query`), because the user is usually still typing, and
/// without that extra index a search for a short beginning walks the whole
/// term dictionary of the vault.
///
/// Measured with 80 000 notes of 2 kB (see `PERF.md`): the search for `s`
/// goes from 155 ms to 90 ms, and on a vault with many different words (a
/// large term dictionary) from 4357 ms to 210 ms. A rebuild costs 18 % more
/// and the file of the index grows by 15 %; writing a batch of changes is not
/// measurably slower. `prefix='1 2 3'` was measured too: it adds 58 % to the
/// rebuild and 45 % to the file, makes a batch 74 % slower and wins nothing
/// over `prefix='1'`, because a longer beginning already matches few terms in
/// the plain dictionary.
const FTS_TABLE: &str = "CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(path UNINDEXED, title, body, prefix='1');";

/// The secondary indexes of the tables. They are kept in one place because
/// `rebuild` drops them for the bulk insert and builds them again afterwards:
/// building an index once over the finished table is cheaper than maintaining
/// it row by row while 20 000 notes are inserted. The UNIQUE index on links is
/// not part of this: `write_note` needs it during the insert to ignore
/// duplicate targets (two links that differ only in case lower-case to the
/// same target).
const SECONDARY_INDEXES: &str = "
    CREATE INDEX IF NOT EXISTS notes_nocase ON notes(path COLLATE NOCASE);
    CREATE INDEX IF NOT EXISTS tags_tag ON tags(tag);
    CREATE INDEX IF NOT EXISTS tags_path ON tags(path);
    CREATE INDEX IF NOT EXISTS tags_nocase ON tags(path COLLATE NOCASE);
    CREATE INDEX IF NOT EXISTS links_target ON links(target);
    CREATE INDEX IF NOT EXISTS links_nocase ON links(source COLLATE NOCASE);
    CREATE INDEX IF NOT EXISTS tasks_path ON tasks(path);
    CREATE INDEX IF NOT EXISTS tasks_nocase ON tasks(path COLLATE NOCASE);";

const DROP_SECONDARY_INDEXES: &str = "
    DROP INDEX IF EXISTS notes_nocase;
    DROP INDEX IF EXISTS tags_tag;
    DROP INDEX IF EXISTS tags_path;
    DROP INDEX IF EXISTS tags_nocase;
    DROP INDEX IF EXISTS links_target;
    DROP INDEX IF EXISTS links_nocase;
    DROP INDEX IF EXISTS tasks_path;
    DROP INDEX IF EXISTS tasks_nocase;";

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
        // The tuning is safe because the index is only a cache (rule 1):
        // synchronous=NORMAL with WAL skips the fsync on every commit; after
        // a power loss the worst case is a stale index, which `sync` repairs.
        // A bigger page cache and memory temp stores keep the joins and the
        // rebuild sorts off the disk, mmap saves read syscalls on queries.
        self.conn.execute_batch(&format!(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -16000;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = 268435456;
             CREATE TABLE IF NOT EXISTS notes (
                 path TEXT PRIMARY KEY,
                 title TEXT NOT NULL,
                 frontmatter TEXT NOT NULL,
                 modified INTEGER NOT NULL,
                 size INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS tags (
                 path TEXT NOT NULL,
                 tag TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS links (
                 source TEXT NOT NULL,
                 target TEXT NOT NULL
             );
             -- A note links to a target at most once (the parser dedups, and
             -- write_note ignores duplicates that differ only in case). The
             -- UNIQUE index makes the graph's DISTINCT + ORDER BY a plain
             -- index scan and covers lookups by source as a prefix.
             CREATE UNIQUE INDEX IF NOT EXISTS links_source_target ON links(source, target);
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
             {SECONDARY_INDEXES}
             {FTS_TABLE}
             PRAGMA user_version = {SCHEMA_VERSION};"
        ))?;
        // The hot statements of the write path (delete_note, write_note,
        // state_of, ...) are prepared once and reused; the default cache is
        // too small to hold all of them at once.
        self.conn.set_prepared_statement_cache_capacity(64);
        Ok(())
    }

    /// Throws everything away and re-indexes all files of the vault.
    pub fn rebuild(&mut self, vault: &Vault) -> Result<usize> {
        let files = vault.files()?;
        // Drop the statements cached from earlier operations: they were
        // planned against the current tables and indexes, some of which are
        // about to be dropped and rebuilt.
        self.conn.flush_prepared_statement_cache();
        let tx = self.conn.transaction()?;
        tx.execute_batch("DELETE FROM notes; DELETE FROM tags; DELETE FROM links; DELETE FROM notes_fts; DELETE FROM tasks;")?;
        // The secondary indexes are built once over the finished tables
        // instead of being maintained for every inserted row. A failure
        // rolls back to the old content with its indexes.
        tx.execute_batch(DROP_SECONDARY_INDEXES)?;
        let mut count = 0;
        for entry in files.iter().filter(|e| is_indexable(&e.path)) {
            let Ok(content) = vault.read(&entry.path) else {
                continue;
            };
            let parsed = markdown::parse(&content, stem(&entry.path));
            write_note(&tx, entry, &parsed, &tasks::extract(&content))?;
            count += 1;
        }
        tx.execute_batch(SECONDARY_INDEXES)?;
        tx.commit()?;
        self.writes += 1;
        self.clear_planner_stats();
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
            // A capacity hint from the row count saves the rehashing while
            // the map is filled on a big vault.
            let rows: i64 = tx.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))?;
            let mut stmt = tx.prepare("SELECT path, modified, size FROM notes")?;
            let rows_iter = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })?;
            let mut map = HashMap::with_capacity(rows.max(0) as usize);
            for row in rows_iter {
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
        self.clear_planner_stats();
        Ok(count)
    }

    /// Clears the statistics of the query planner after a bulk write.
    /// `PRAGMA optimize` would collect them (ANALYZE), but on a small table
    /// the statistics make the planner believe a scan is as cheap as the
    /// NOCASE range search that the folder deletes need; on a big vault that
    /// scan walks every row. Without statistics every query is planned by
    /// the shape of the indexes, which is what this schema is built for.
    /// A failure here must not fail the write that already succeeded.
    fn clear_planner_stats(&self) {
        let _ = self
            .conn
            .execute_batch("DROP TABLE IF EXISTS sqlite_stat1;");
    }

    /// Moves the notes of a folder to its new name, after the folder was
    /// renamed on disk. Their content does not change, only their paths, so
    /// the paths are rewritten in SQL instead of reading and parsing every
    /// note again. Titles need no refresh: a folder rename keeps every file
    /// name, so a title that falls back to the file stem stays correct.
    pub fn move_folder(&mut self, _vault: &Vault, from: &str, to: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        // The notes of the folder are collected first: one range search on
        // the NOCASE index, the same search the old prefix delete used.
        let mut moved: Vec<(i64, String)> = Vec::new();
        {
            let mut stmt =
                tx.prepare("SELECT rowid, path FROM notes WHERE path LIKE ?1 ESCAPE '\\'")?;
            let found = stmt.query_map([prefix_pattern(&format!("{from}/"))], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?;
            for row in found {
                moved.push(row?);
            }
        }
        // Stale rows under the destination (left over from an earlier folder
        // of that name) go first, so the updates cannot collide with them.
        // Rows that are about to move are kept: on a case-only rename the
        // destination prefix matches them too (LIKE is case-insensitive).
        let moved_ids: HashSet<i64> = moved.iter().map(|(id, _)| *id).collect();
        let stale: Vec<String> = {
            let mut stmt =
                tx.prepare("SELECT rowid, path FROM notes WHERE path LIKE ?1 ESCAPE '\\'")?;
            let found = stmt.query_map([prefix_pattern(&format!("{to}/"))], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?;
            let mut paths = Vec::new();
            for row in found {
                let (id, path) = row?;
                if !moved_ids.contains(&id) {
                    paths.push(path);
                }
            }
            paths
        };
        for path in &stale {
            delete_note(&tx, path)?;
        }
        // The new paths are written row by row, not with one bulk UPDATE per
        // table: `notes.path` is the primary key, and a bulk update can
        // collide with a row of its own result set midway (a case-only
        // rename). Per-row writes cannot, because the destination paths were
        // free before the rename. The child tables are matched by exact path
        // (BINARY, like `delete_note`), the FTS row by its rowid.
        let mut note_path = tx.prepare("UPDATE notes SET path = ?1 WHERE rowid = ?2")?;
        let mut tag_path = tx.prepare("UPDATE tags SET path = ?1 WHERE path = ?2")?;
        let mut link_source = tx.prepare("UPDATE links SET source = ?1 WHERE source = ?2")?;
        let mut task_path = tx.prepare("UPDATE tasks SET path = ?1 WHERE path = ?2")?;
        let mut fts_path = tx.prepare("UPDATE notes_fts SET path = ?1 WHERE rowid = ?2")?;
        for (rowid, path) in &moved {
            let new_path = format!("{to}{}", &path[from.len()..]);
            note_path.execute(params![new_path, rowid])?;
            tag_path.execute(params![new_path, path])?;
            link_source.execute(params![new_path, path])?;
            task_path.execute(params![new_path, path])?;
            // `path` is UNINDEXED in the FTS table, so this only rewrites the
            // stored path; the rowid still points at the same note.
            fts_path.execute(params![new_path, rowid])?;
        }
        drop((note_path, tag_path, link_source, task_path, fts_path));
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
        // One change per path is enough: only the last report about a path
        // describes its final state, earlier ones are outdated. Walking
        // backwards keeps the last change of every path, and the reverse
        // afterwards restores the arrival order, which matters when a folder
        // and a note inside it both changed.
        let mut seen: HashSet<&str> = HashSet::with_capacity(changes.len());
        let mut unique: Vec<&VaultChange> = Vec::with_capacity(changes.len());
        for change in changes.iter().rev() {
            if seen.insert(change.path.as_str()) {
                unique.push(change);
            }
        }
        unique.reverse();
        let actions = self.actions_for(vault, &unique)?;
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
                    for entry in vault
                        .files_below(path)?
                        .iter()
                        .filter(|e| is_indexable(&e.path))
                    {
                        let Ok(content) = vault.read(&entry.path) else {
                            continue;
                        };
                        let parsed = markdown::parse(&content, stem(&entry.path));
                        delete_note(&tx, &entry.path)?;
                        write_note(&tx, entry, &parsed, &tasks::extract(&content))?;
                    }
                }
                Action::Reindex(path, entry) => {
                    // The entry was already stat'ed while deciding the
                    // action; the file is not asked a second time.
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
        changes: &[&'c VaultChange],
    ) -> Result<Vec<Action<'c>>> {
        let mut actions = Vec::new();
        // Prepared once for the whole batch instead of once per change.
        let mut state_of = self
            .conn
            .prepare_cached("SELECT modified, size FROM notes WHERE path = ?1")?;
        for change in changes {
            if change.hidden {
                continue;
            }
            match change.kind {
                ChangeKind::Removed => actions.push(Action::Remove(&change.path)),
                ChangeKind::Created | ChangeKind::Modified => {
                    if !is_indexable(&change.path) {
                        // The entry of the watcher, when it has one, answers
                        // whether the path is a folder without a stat.
                        let is_dir = match &change.entry {
                            Some(entry) => entry.is_dir,
                            None => vault.resolve(&change.path)?.is_dir(),
                        };
                        if is_dir {
                            // A folder that exists is never taken out: after a
                            // rename its notes were just moved here. Its notes
                            // are read (again), which is cheap next to losing them.
                            if change.kind == ChangeKind::Created {
                                actions.push(Action::ReindexFolder(&change.path));
                            }
                        } else if change.entry.is_none()
                            && !vault.resolve(&change.path)?.exists()
                            && self.knows(&change.path)?
                        {
                            // A folder that was indexed and is gone.
                            actions.push(Action::Remove(&change.path));
                        }
                        continue;
                    }
                    // One stat per change: the entry of the watcher is used
                    // when it has one, otherwise it is read here once and
                    // carried into the action.
                    let entry: Option<Cow<'c, VaultEntry>> = match &change.entry {
                        Some(entry) if !entry.is_dir => Some(Cow::Borrowed(entry)),
                        _ => vault
                            .entry(&change.path)
                            .ok()
                            .filter(|e| !e.is_dir)
                            .map(Cow::Owned),
                    };
                    let Some(entry) = entry else {
                        actions.push(Action::Remove(&change.path));
                        continue;
                    };
                    let state = (entry.modified as i64, entry.size as i64);
                    let known: Option<(i64, i64)> = state_of
                        .query_row([&change.path], |r| Ok((r.get(0)?, r.get(1)?)))
                        .optional()?;
                    if known != Some(state) {
                        actions.push(Action::Reindex(&change.path, entry));
                    }
                }
            }
        }
        Ok(actions)
    }

    /// True when the index holds a note with this path or below it.
    fn knows(&self, rel: &str) -> Result<bool> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM notes WHERE path = ?1 OR path LIKE ?2 ESCAPE '\\')",
        )?;
        let found: i64 =
            stmt.query_row([rel, &prefix_pattern(&format!("{rel}/"))], |r| r.get(0))?;
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
                // Most tasks have no tags; their JSON needs no parser.
                let tags = if tags == "[]" {
                    Vec::new()
                } else {
                    serde_json::from_str(&tags).unwrap_or_default()
                };
                Ok(TaskItem {
                    path: r.get(0)?,
                    note_title: r.get(1)?,
                    task: Task {
                        line: r.get(2)?,
                        done: r.get(3)?,
                        text: r.get(4)?,
                        due: r.get(5)?,
                        priority: priority.as_deref().and_then(tasks::Priority::parse),
                        tags,
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
        // The UNIQUE index on (source, target) holds every link once and in
        // this order, so the graph reads it as a plain covering index scan,
        // without a DISTINCT over every row.
        let mut stmt = self
            .conn
            .prepare("SELECT source, target FROM links ORDER BY source, target")?;
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
            (
                "SELECT path, title, frontmatter FROM notes ORDER BY path",
                "notes",
            ),
            ("SELECT path, tag FROM tags ORDER BY path, tag", "tags"),
            (
                "SELECT source, target FROM links ORDER BY source, target",
                "links",
            ),
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
                        rusqlite::types::ValueRef::Text(t) => {
                            String::from_utf8_lossy(t).into_owned()
                        }
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
    /// The note is read again and written to the index. The entry is the
    /// stat result that was already read while deciding the action.
    Reindex(&'a str, Cow<'a, VaultEntry>),
    /// Every note below the folder is read and written to the index.
    ReindexFolder(&'a str),
}

/// The `LIKE` pattern of everything below `prefix`, with the wildcards of
/// the prefix itself escaped: a folder named `a_b` must not match `axb/...`
/// (`_` and `%` are wildcards in LIKE). Used with `ESCAPE '\'`; the NOCASE
/// indexes still answer it with a range search.
fn prefix_pattern(prefix: &str) -> String {
    let mut out = String::with_capacity(prefix.len() + 1);
    for c in prefix.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

/// Takes a note out of the index, and everything below it when it is a
/// folder.
fn remove_note(tx: &rusqlite::Transaction, rel: &str) -> rusqlite::Result<()> {
    let had_note = delete_note(tx, rel)?;
    // A plain file cannot have notes below it, so the five prefix deletes
    // are skipped. One cheap probe still guards the rare case of stale rows
    // below the path (a folder that was replaced by a file of the same name).
    if had_note {
        let children: i64 = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM notes WHERE path LIKE ?1 ESCAPE '\\')",
            [prefix_pattern(&format!("{rel}/"))],
            |r| r.get(0),
        )?;
        if children == 0 {
            return Ok(());
        }
    }
    remove_prefix(tx, &format!("{rel}/"))
}

/// Removes every note below `prefix` from the index.
fn remove_prefix(tx: &rusqlite::Transaction, prefix: &str) -> rusqlite::Result<()> {
    let pattern = prefix_pattern(prefix);
    // The search rows go first: their rowids come from the notes below the
    // folder, which are deleted right after. The statements are cached on
    // the connection like the ones of `delete_note`.
    tx.prepare_cached(
        "DELETE FROM notes_fts WHERE rowid IN (SELECT rowid FROM notes WHERE path LIKE ?1 ESCAPE '\\')",
    )?
    .execute([&pattern])?;
    tx.prepare_cached("DELETE FROM notes WHERE path LIKE ?1 ESCAPE '\\'")?
        .execute([&pattern])?;
    tx.prepare_cached("DELETE FROM tags WHERE path LIKE ?1 ESCAPE '\\'")?
        .execute([&pattern])?;
    tx.prepare_cached("DELETE FROM links WHERE source LIKE ?1 ESCAPE '\\'")?
        .execute([&pattern])?;
    tx.prepare_cached("DELETE FROM tasks WHERE path LIKE ?1 ESCAPE '\\'")?
        .execute([&pattern])?;
    Ok(())
}

/// Deletes the rows of one note. Returns whether the notes table held the
/// path, so `remove_note` can tell a plain file from a folder.
fn delete_note(tx: &rusqlite::Transaction, rel: &str) -> rusqlite::Result<bool> {
    // `path` is UNINDEXED in the FTS table, so a delete by path would scan
    // the whole table. The rowid of the note is looked up first and used to
    // delete the FTS row directly.
    let rowid: Option<i64> = tx
        .query_row("SELECT rowid FROM notes WHERE path = ?1", [rel], |r| {
            r.get(0)
        })
        .ok();
    if let Some(rowid) = rowid {
        tx.prepare_cached("DELETE FROM notes_fts WHERE rowid = ?1")?
            .execute([rowid])?;
    }
    // The statements are cached on the connection, so a batch or a rebuild
    // prepares them once instead of once per note.
    tx.prepare_cached("DELETE FROM notes WHERE path = ?1")?
        .execute([rel])?;
    tx.prepare_cached("DELETE FROM tags WHERE path = ?1")?
        .execute([rel])?;
    tx.prepare_cached("DELETE FROM links WHERE source = ?1")?
        .execute([rel])?;
    tx.prepare_cached("DELETE FROM tasks WHERE path = ?1")?
        .execute([rel])?;
    Ok(rowid.is_some())
}

fn write_note(
    tx: &rusqlite::Transaction,
    entry: &VaultEntry,
    note: &ParsedNote,
    tasks: &[Task],
) -> rusqlite::Result<()> {
    // Every statement is prepared once per connection and reused for all
    // notes of a rebuild or a batch (see `delete_note`).
    // A note without frontmatter is the common case; its JSON is written
    // without going through the serializer.
    let frontmatter = match note.frontmatter.as_object() {
        Some(map) if map.is_empty() => Cow::Borrowed("{}"),
        _ => Cow::Owned(note.frontmatter.to_string()),
    };
    tx.prepare_cached(
        "INSERT INTO notes (path, title, frontmatter, modified, size) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(params![
        entry.path,
        note.title,
        frontmatter.as_ref(),
        entry.modified as i64,
        entry.size as i64
    ])?;
    // The FTS row carries the rowid of the note, so both can be deleted and
    // found again without a scan (see `delete_note`).
    let rowid = tx.last_insert_rowid();
    {
        let mut insert_tag = tx.prepare_cached("INSERT INTO tags (path, tag) VALUES (?1, ?2)")?;
        for tag in &note.tags {
            insert_tag.execute(params![entry.path, tag])?;
        }
    }
    {
        // OR IGNORE: two links that differ only in case lower-case to the
        // same target, which the UNIQUE index on (source, target) allows
        // only once.
        let mut insert_link =
            tx.prepare_cached("INSERT OR IGNORE INTO links (source, target) VALUES (?1, ?2)")?;
        for link in &note.links {
            // Link targets are stored lower-cased, so backlinks find them whatever
            // case they were written in. Most are already lower-case; then no
            // copy is needed.
            let lower = lower_case(link);
            insert_link.execute(params![entry.path, lower.as_ref()])?;
        }
    }
    tx.prepare_cached("INSERT INTO notes_fts (rowid, path, title, body) VALUES (?1, ?2, ?3, ?4)")?
        .execute(params![rowid, entry.path, note.title, note.text])?;
    {
        let mut insert_task = tx.prepare_cached(
            "INSERT INTO tasks (path, line, done, text, due, priority, tags, indent) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        for t in tasks {
            // A task without tags is the common case; its JSON needs no
            // serializer.
            let tags = if t.tags.is_empty() {
                Cow::Borrowed("[]")
            } else {
                Cow::Owned(serde_json::to_string(&t.tags).unwrap_or_else(|_| "[]".into()))
            };
            insert_task.execute(params![
                entry.path,
                t.line,
                t.done,
                t.text,
                t.due,
                t.priority.map(|p| p.as_str()),
                tags.as_ref(),
                t.indent
            ])?;
        }
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

    /// The search matches every word as a prefix, because the user is still
    /// typing. The prefix index of the search table is what makes even a
    /// one-letter beginning cheap; a test of its own, since the cost cannot
    /// be checked by a test and the result can.
    #[test]
    fn a_search_finds_a_note_while_the_word_is_still_being_typed() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        // The whole word and every beginning of it give the same note.
        for typed in ["schedul", "schedule"] {
            let hits = index.search(typed, 10).unwrap();
            assert_eq!(hits.len(), 1, "search for {typed:?}");
            assert_eq!(hits[0].path, "beta.md");
        }
        // A prefix of one letter already reaches the term dictionary.
        assert!(!index.search("s", 10).unwrap().is_empty());
        // Something that is not in any note finds nothing.
        assert!(index.search("zzznothing", 10).unwrap().is_empty());
    }

    /// The search table of an index that was opened after a version bump
    /// really has the prefix option, and a sync (the way the app brings an
    /// existing index in line) fills it. `rebuild` is covered by the test
    /// above; the app itself only calls `sync` on open.
    #[test]
    fn an_upgraded_index_gets_the_prefix_index_of_the_search_table() {
        let (_dir, vault) = sample_vault();
        let path = vault.app_dir().join("index.sqlite");

        // An index of the version before the prefix index was introduced.
        {
            let old = Connection::open(&path).unwrap();
            old.execute_batch(
                "CREATE TABLE notes (path TEXT PRIMARY KEY, title TEXT NOT NULL,
                                     frontmatter TEXT NOT NULL, modified INTEGER NOT NULL,
                                     size INTEGER NOT NULL);
                 CREATE VIRTUAL TABLE notes_fts USING fts5(path UNINDEXED, title, body);
                 INSERT INTO notes VALUES ('old.md', 'Old', '{}', 1, 10);
                 PRAGMA user_version = 5;",
            )
            .unwrap();
        }

        let mut index = Index::open(&path).unwrap();
        // The schema of the new search table is the one of `FTS_TABLE`.
        let ddl: String = index
            .conn()
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name = 'notes_fts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            ddl.contains("prefix='1'"),
            "schema of the search table: {ddl}"
        );

        // A sync over an emptied index indexes the vault, and the search
        // still answers a beginning of a word.
        index.sync(&vault).unwrap();
        assert_eq!(index.counts().unwrap(), (2, 2));
        assert_eq!(index.search("schedul", 10).unwrap().len(), 1);
    }

    /// Removing a folder takes every note below it out of the index, the
    /// search index included.
    #[test]
    fn removing_a_folder_takes_all_of_its_notes_out() {
        let (_dir, vault) = sample_vault();
        vault
            .write("projects/gamma.md", "# Gamma\nA schedule.")
            .unwrap();
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
        vault
            .write("projects/gamma.md", "# Gamma\nA schedule.")
            .unwrap();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        vault.write("beta.md", "# Beta\nChanged text.").unwrap();
        index.update(&vault, "beta.md").unwrap();
        index.remove("projects").unwrap();
        vault.write("new.md", "# New\nA new note.").unwrap();
        index.update(&vault, "new.md").unwrap();

        assert_eq!(index.counts().unwrap(), (2, 2));
        assert_eq!(
            index
                .conn()
                .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
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
            let sql = format!(
                "EXPLAIN QUERY PLAN DELETE FROM {table} WHERE {column} LIKE ?1 ESCAPE '\\'"
            );
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
            index
                .conn()
                .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
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
        vault
            .write("projects/gamma.md", "# Gamma\nA #project note.")
            .unwrap();
        vault
            .write("notes/deep/delta.md", "# Delta\nMore text.")
            .unwrap();
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
        vault
            .write("projects/gamma.md", "# Gamma\nA #project note.")
            .unwrap();

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
    fn a_renamed_folder_keeps_its_notes() {
        let (_dir, vault) = sample_vault();
        vault
            .write("projects/gamma.md", "# Gamma\nA #project note.")
            .unwrap();
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
                .write(
                    &format!("batch/note-{i}.md"),
                    &format!("# Note {i}\nA schedule."),
                )
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
        assert_eq!(
            index.writes(),
            before,
            "the batch wrote although nothing changed"
        );

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
            .write(
                "projects/gamma.md",
                "# Gamma\nMore text with a #project tag.",
            )
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
        vault
            .write("projects/gamma.md", "# Gamma\nA #project note.")
            .unwrap();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        vault.rename("projects", "work").unwrap();
        index.move_folder(&vault, "projects", "work").unwrap();
        let change = |kind, path: &str| VaultChange {
            kind,
            path: path.into(),
            hidden: false,
            entry: None,
        };
        index
            .apply_changes(
                &vault,
                &[
                    change(ChangeKind::Removed, "projects"),
                    change(ChangeKind::Created, "work"),
                ],
            )
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
        let change = VaultChange {
            kind: ChangeKind::Created,
            path: "moved".into(),
            hidden: false,
            entry: None,
        };
        index.apply_changes(&vault, &[change]).unwrap();

        assert_eq!(index.counts().unwrap(), (3, 3));
        assert_eq!(index.search("moved note", 10).unwrap().len(), 1);
    }

    /// Renaming a folder rewrites only the paths: the index afterwards says
    /// exactly what a full rebuild of the renamed vault says, including the
    /// search rows and the tasks.
    #[test]
    fn a_renamed_folder_says_the_same_as_a_rebuild() {
        let (_dir, vault) = sample_vault();
        vault
            .write(
                "projects/deep/tasks.md",
                "# Tasks\n- [ ] A task #project\n- [x] Done\n",
            )
            .unwrap();
        // A note whose title falls back to the file stem: the file name does
        // not change in a folder rename, so the title must stay.
        vault
            .write("projects/plain.md", "No heading here.")
            .unwrap();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        vault.rename("projects", "work").unwrap();
        index.move_folder(&vault, "projects", "work").unwrap();

        let mut fresh = Index::open_in_memory().unwrap();
        fresh.rebuild(&vault).unwrap();
        assert_eq!(index.dump(), fresh.dump());
        assert_eq!(index.counts().unwrap(), fresh.counts().unwrap());

        // The search rows carry the new path, and the tasks moved with it.
        let hits = index.search("task", 10).unwrap();
        assert!(hits.iter().any(|h| h.path == "work/deep/tasks.md"));
        let tasks = index.tasks().unwrap();
        assert!(tasks.iter().all(|t| !t.path.starts_with("projects/")));
        assert!(tasks.iter().any(|t| t.path == "work/deep/tasks.md"));
    }

    /// A case-only rename works too: LIKE finds the notes case-insensitively,
    /// and the per-row updates cannot collide with their own result set.
    #[test]
    fn a_folder_can_be_renamed_to_a_different_case() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        // On a case-insensitive file system this rename is a no-op there,
        // but the index must still take over the new spelling.
        let _ = vault.rename("projects", "Projects");
        index.move_folder(&vault, "projects", "Projects").unwrap();

        let notes = index.notes_with_tag("project").unwrap();
        assert!(notes.iter().any(|n| n.path == "Projects/alpha.md"));
        assert!(!notes.iter().any(|n| n.path.starts_with("projects/")));
        assert_eq!(index.counts().unwrap(), (2, 2));
    }

    /// Stale rows under the destination name go away, as the old
    /// delete-and-reparse did.
    #[test]
    fn moving_a_folder_over_stale_rows_replaces_them() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();
        // A leftover note under the destination, whose file no longer exists.
        let ghost = VaultEntry {
            path: "work/ghost.md".into(),
            name: "ghost.md".into(),
            extension: "md".into(),
            is_dir: false,
            size: 1,
            modified: 1,
        };
        let parsed = markdown::parse("# Ghost\nA phantom.", "ghost");
        let tx = index.conn.transaction().unwrap();
        write_note(&tx, &ghost, &parsed, &[]).unwrap();
        tx.commit().unwrap();

        vault.rename("projects", "work").unwrap();
        index.move_folder(&vault, "projects", "work").unwrap();

        assert!(index.search("phantom", 10).unwrap().is_empty());
        assert_eq!(index.counts().unwrap(), (2, 2));
        assert!(
            index
                .notes_with_tag("project")
                .unwrap()
                .iter()
                .any(|n| n.path == "work/alpha.md")
        );
    }

    /// Several reports about one path in a batch mean one re-index of the
    /// final state, not one per report.
    #[test]
    fn a_batch_keeps_only_the_last_change_of_a_path() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        vault.write("beta.md", "# Beta\nA timetable now.").unwrap();
        let change = |kind| VaultChange {
            kind,
            path: "beta.md".into(),
            hidden: false,
            entry: None,
        };
        index
            .apply_changes(
                &vault,
                &[
                    change(ChangeKind::Modified),
                    change(ChangeKind::Modified),
                    change(ChangeKind::Modified),
                ],
            )
            .unwrap();

        assert_eq!(index.counts().unwrap(), (2, 2));
        assert!(index.search("schedule", 10).unwrap().is_empty());
        assert_eq!(index.search("timetable", 10).unwrap().len(), 1);

        // Removed and then created again: the file exists, so it stays.
        vault.write("beta.md", "# Beta\nA schedule again.").unwrap();
        index
            .apply_changes(
                &vault,
                &[change(ChangeKind::Removed), change(ChangeKind::Created)],
            )
            .unwrap();
        assert_eq!(index.search("schedule", 10).unwrap().len(), 1);
    }

    /// The stat result of the watcher is used: a change whose entry already
    /// matches the index is not read again, and one that differs is indexed
    /// with the content of the file.
    #[test]
    fn a_batch_uses_the_entry_of_the_watcher() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();
        let before = index.writes();

        // The entry the index already knows: nothing to do, even though the
        // file on disk changed after the entry was taken.
        let known = vault.entry("beta.md").unwrap();
        vault.write("beta.md", "# Beta\nDifferent text.").unwrap();
        let change = VaultChange {
            kind: ChangeKind::Modified,
            path: "beta.md".into(),
            hidden: false,
            entry: Some(known),
        };
        index.apply_changes(&vault, &[change]).unwrap();
        assert_eq!(index.writes(), before, "the known entry was not trusted");

        // A fresh entry: the note is read and indexed.
        let change = VaultChange {
            kind: ChangeKind::Modified,
            path: "beta.md".into(),
            hidden: false,
            entry: Some(vault.entry("beta.md").unwrap()),
        };
        index.apply_changes(&vault, &[change]).unwrap();
        assert_eq!(index.writes(), before + 1);
        assert_eq!(index.search("different", 10).unwrap().len(), 1);
    }

    /// The graph reads the links through the UNIQUE index: no temporary
    /// B-tree for the DISTINCT and the ORDER BY.
    #[test]
    fn the_graph_links_come_straight_from_the_index() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        let mut plan = index
            .conn()
            .prepare(
                "EXPLAIN QUERY PLAN SELECT DISTINCT source, target FROM links ORDER BY source, target",
            )
            .unwrap();
        let mut details = Vec::new();
        let mut rows = plan.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            details.push(row.get::<_, String>(3).unwrap());
        }
        let plan = details.join(" | ");
        assert!(
            plan.contains("links_source_target"),
            "the graph does not use the UNIQUE index: {plan}"
        );
        assert!(
            !plan.contains("TEMP B-TREE"),
            "the graph still sorts or dedupes by hand: {plan}"
        );
    }

    /// The backlinks join starts from the links of the target, not from a
    /// scan over all notes.
    #[test]
    fn the_backlinks_join_uses_the_target_index() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        let mut plan = index
            .conn()
            .prepare(
                "EXPLAIN QUERY PLAN SELECT DISTINCT n.path, n.title, n.frontmatter
                 FROM notes n JOIN links l ON l.source = n.path
                 WHERE l.target IN (?1, ?2, ?3) AND n.path != ?4 ORDER BY n.title",
            )
            .unwrap();
        let mut details = Vec::new();
        let mut rows = plan.query(["beta", "beta", "beta", "beta.md"]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            details.push(row.get::<_, String>(3).unwrap());
        }
        let plan = details.join(" | ");
        assert!(
            plan.contains("links_target"),
            "the backlinks do not use the target index: {plan}"
        );
        assert!(
            !plan.contains("SCAN n"),
            "the backlinks scan the notes table: {plan}"
        );
    }

    /// Two links that differ only in case are one edge in the graph: the
    /// UNIQUE index holds them once.
    #[test]
    fn links_that_differ_only_in_case_are_stored_once() {
        let (_dir, vault) = sample_vault();
        vault
            .write(
                "beta.md",
                "# Beta\nSee [[Alpha]] and [[alpha]] and [[ALPHA]].",
            )
            .unwrap();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        let edges: i64 = index
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM links WHERE source = 'beta.md'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(edges, 1);
        let graph = index.graph().unwrap();
        assert_eq!(
            graph.links.iter().filter(|l| l.source == "beta.md").count(),
            1
        );
    }

    /// A folder rename only rewrites the paths in SQL; everything the index
    /// holds about the notes moves along: tags, links, tasks and the search
    /// rows.
    #[test]
    fn a_folder_rename_moves_every_table() {
        let (_dir, vault) = sample_vault();
        vault
            .write(
                "projects/todo.md",
                "# Todo\n- [ ] A task #project\nSee [[Beta]].",
            )
            .unwrap();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        vault.rename("projects", "work").unwrap();
        index.move_folder(&vault, "projects", "work").unwrap();

        assert_eq!(index.notes_with_tag("project").unwrap().len(), 3);
        let tasks = index.tasks().unwrap();
        assert!(tasks.iter().any(|t| t.path == "work/todo.md"));
        assert_eq!(index.backlinks("beta.md").unwrap().len(), 2);
        // The search row of a moved note carries its new path.
        let paths: Vec<String> = index
            .conn()
            .prepare("SELECT path FROM notes_fts ORDER BY path")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert!(paths.contains(&"work/todo.md".to_string()));
        assert!(!paths.iter().any(|p| p.starts_with("projects/")));
        assert_eq!(index.counts().unwrap(), (3, 3));
    }

    /// A case-only rename (`projects` -> `Projects`) keeps every note: the
    /// LIKE prefix matches the rows that are about to move, so they must not
    /// be taken out as stale rows of the destination.
    #[test]
    fn a_case_only_folder_rename_keeps_its_notes() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        // On a case-sensitive file system this is a real rename.
        vault.rename("projects", "Projects").unwrap();
        index.move_folder(&vault, "projects", "Projects").unwrap();

        assert_eq!(index.counts().unwrap(), (2, 2));
        assert!(
            index
                .backlinks("beta.md")
                .unwrap()
                .iter()
                .any(|n| n.path == "Projects/alpha.md")
        );
        assert_eq!(
            index.search("links", 10).unwrap()[0].path,
            "Projects/alpha.md"
        );
    }

    /// The same path reported several times in one batch is applied once,
    /// and the last report wins: a note that was written and removed again
    /// before the batch arrives leaves nothing behind.
    #[test]
    fn a_batch_applies_only_the_last_report_of_a_path() {
        let (_dir, vault) = sample_vault();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        vault
            .write("ghost.md", "# Ghost\nA fleeting note.")
            .unwrap();
        vault.trash("ghost.md").unwrap();
        let change = |kind, path: &str| VaultChange {
            kind,
            path: path.into(),
            hidden: false,
            entry: None,
        };
        index
            .apply_changes(
                &vault,
                &[
                    change(ChangeKind::Created, "ghost.md"),
                    change(ChangeKind::Modified, "ghost.md"),
                    change(ChangeKind::Removed, "ghost.md"),
                ],
            )
            .unwrap();
        assert_eq!(index.counts().unwrap(), (2, 2));
        assert!(index.search("fleeting", 10).unwrap().is_empty());

        // And the other way round: removed and written again, the note is
        // there with its new text.
        vault.write("beta.md", "# Beta\nA comeback.").unwrap();
        index
            .apply_changes(
                &vault,
                &[
                    change(ChangeKind::Removed, "beta.md"),
                    change(ChangeKind::Created, "beta.md"),
                ],
            )
            .unwrap();
        assert_eq!(index.search("comeback", 10).unwrap().len(), 1);
        assert_eq!(index.counts().unwrap(), (2, 2));
    }

    /// `_` and `%` in a folder name are literal characters, not wildcards:
    /// removing `a_b` must leave the notes of `axb` alone.
    #[test]
    fn a_folder_name_with_like_wildcards_is_matched_literally() {
        let (_dir, vault) = sample_vault();
        vault.write("a_b/one.md", "# One\nWortuniq text.").unwrap();
        vault.write("axb/two.md", "# Two\nAnother text.").unwrap();
        let mut index = Index::for_vault(&vault).unwrap();
        index.rebuild(&vault).unwrap();

        index.remove("a_b").unwrap();
        assert!(index.search("wortuniq", 10).unwrap().is_empty());
        assert_eq!(index.search("another", 10).unwrap().len(), 1);
        assert_eq!(index.counts().unwrap(), (3, 3));
    }
}
