//! Measures how long the index needs for the operations of a normal vault.
//!
//! Run it with the number of notes to build:
//!
//! ```sh
//! cargo run --release --example index_bench -- 20000
//! ```
//!
//! The numbers are meant to be compared before and after a change, always
//! with the same note count. `PERF.md` keeps what was measured and what was
//! decided about it.

use std::time::{Duration, Instant};

use dvoix_core::Vault;
use dvoix_core::index::Index;
use dvoix_core::watcher::{ChangeKind, VaultChange};

/// The text every note has: a heading, some words, a link and a task.
const NOTE: &str = "---\ntype: note\ntags: [project]\n---\n# Note\n\nA note with some words to search for. It links to [[Other]] and has a task.\n\n- [ ] Something to do #project\n";

/// The body of a note, so a vault of a given size can be built.
fn note_with_body(bytes: usize) -> String {
    let mut note = NOTE.to_string();
    while note.len() < bytes {
        note.push_str("Some more words for the search index to hold.\n\n");
    }
    note
}

/// Fills the vault with `count` notes in folders of 100.
fn fill(vault: &Vault, count: usize, body: usize) {
    let text = note_with_body(body);
    for i in 0..count {
        vault
            .write(&format!("folder{}/note-{i}.md", i % 100), &text)
            .expect("the note can be written");
    }
}

/// Runs `f` and returns how long it took.
fn time<T>(f: impl FnOnce() -> T) -> (Duration, T) {
    let start = Instant::now();
    let value = f();
    (start.elapsed(), value)
}

/// Prints one line of the result table.
fn row(label: &str, duration: Duration) {
    println!("{label:<34} {:>9.2} ms", duration.as_secs_f64() * 1000.0);
}

fn main() {
    let count: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(20_000);
    // The size of a note in bytes. The cost of the old search delete grew
    // with the text of the whole vault, so the size matters for the numbers.
    let body: usize = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(2000);

    let dir = tempfile::tempdir().expect("a temporary folder");
    let vault = Vault::open(dir.path()).expect("the vault opens");

    let (elapsed, ()) = time(|| fill(&vault, count, body));
    println!(
        "vault with {count} notes of {body} bytes filled in {:.0} ms",
        elapsed.as_secs_f64() * 1000.0
    );

    let mut index = Index::for_vault(&vault).expect("the index opens");

    let (elapsed, entries) = time(|| vault.list().expect("the vault is listed"));
    row(
        &format!("list the vault ({} entries)", entries.len()),
        elapsed,
    );

    let (elapsed, indexed) = time(|| index.rebuild(&vault).expect("the index builds"));
    println!("{indexed} notes indexed\n");
    row("full rebuild", elapsed);

    // What opening a vault costs when nothing changed since the last time.
    let (elapsed, changed) = time(|| index.sync(&vault).expect("the index is in line"));
    row(&format!("sync ({} notes read again)", changed), elapsed);

    // The same, but one note changed while the app was closed.
    vault
        .write("folder0/note-0.md", &note_with_body(body))
        .expect("the note is written");
    let (elapsed, changed) = time(|| index.sync(&vault).expect("the index is in line"));
    row(&format!("sync ({} notes read again)", changed), elapsed);

    // The change of one note: what happens on every save.
    let (elapsed, ()) = time(|| {
        index
            .update(&vault, "folder0/note-0.md")
            .expect("the note is re-indexed")
    });
    row("re-index one note", elapsed);

    // The same, but the note is also written first, like a save does it.
    let (elapsed, ()) = time(|| {
        vault
            .write("folder0/note-0.md", &note_with_body(body))
            .expect("the note is written");
        index
            .update(&vault, "folder0/note-0.md")
            .expect("the note is re-indexed")
    });
    row("write and re-index one note", elapsed);

    // What a save of a note in a big vault costs, the note being at the end.
    let (elapsed, ()) = time(|| {
        index
            .update(&vault, &format!("folder99/note-{}.md", count - 1))
            .expect("the note is re-indexed")
    });
    row("re-index the last note", elapsed);

    let (elapsed, hits) = time(|| index.search("words", 50).expect("the search works"));
    row(&format!("search ({} hits)", hits.len()), elapsed);

    // The same search while it is still being typed: one letter and three
    // letters. These are the queries the prefix index of the search table
    // exists for; without it each one walks the whole term dictionary.
    let (elapsed, hits) = time(|| index.search("s", 50).expect("the search works"));
    row(&format!("search one letter ({} hits)", hits.len()), elapsed);
    let (elapsed, hits) = time(|| index.search("sea", 50).expect("the search works"));
    row(
        &format!("search three letters ({} hits)", hits.len()),
        elapsed,
    );

    let (elapsed, tags) = time(|| index.tags().expect("the tags are read"));
    row(&format!("tags ({} tags)", tags.len()), elapsed);

    let (elapsed, tasks) = time(|| index.tasks().expect("the tasks are read"));
    row(&format!("tasks ({} tasks)", tasks.len()), elapsed);

    let (elapsed, projects) = time(|| index.projects().expect("the projects are read"));
    row(&format!("projects ({} projects)", projects.len()), elapsed);

    let (elapsed, graph) = time(|| index.graph().expect("the graph is read"));
    row(
        &format!(
            "graph ({} notes, {} links)",
            graph.notes.len(),
            graph.links.len()
        ),
        elapsed,
    );

    // The removal of a folder with 100 notes in it.
    let (elapsed, ()) = time(|| index.remove("folder50").expect("the folder is removed"));
    row("remove a folder of 100 notes", elapsed);

    // The rename of a folder: the paths of its notes move in the index
    // without the notes being read again (see `move_folder`). Twice, because
    // the first one can still be busy with the write ahead log of the
    // rebuild (see the batches below).
    for folder in ["folder0", "folder1"] {
        let to = format!("renamed-{folder}");
        let (elapsed, ()) = time(|| {
            index
                .move_folder(&vault, folder, &to)
                .expect("the folder is moved")
        });
        row(&format!("rename a folder of 100 notes ({folder})"), elapsed);
    }

    // A burst of changes, like a git checkout: 100 notes at once.
    let changes: Vec<VaultChange> = (0..100)
        .map(|i| VaultChange {
            kind: ChangeKind::Modified,
            path: format!("burst/note-{i}.md"),
            hidden: false,
            entry: None,
        })
        .collect();

    // Every run gets files that really changed, so every run has the same
    // work to do.
    let write_burst = |tag: &str| {
        for i in 0..100 {
            vault
                .write(
                    &format!("burst/note-{i}.md"),
                    &format!("{tag} {}", note_with_body(body)),
                )
                .expect("the note is written");
        }
    };

    write_burst("first");
    let (elapsed, ()) = time(|| {
        for change in &changes {
            index
                .apply_changes(&vault, std::slice::from_ref(change))
                .expect("the change is applied");
        }
    });
    row("100 changes one by one", elapsed);

    write_burst("second");
    let (elapsed, ()) = time(|| {
        index
            .apply_changes(&vault, &changes)
            .expect("the changes are applied")
    });
    row("the same 100 changes in one batch", elapsed);

    // The size of the file the index lives in, with the write ahead log that
    // still belongs to it. Schema options such as the prefix index of the
    // search table show here and on the rebuild, not on a single query.
    let size = ["index.sqlite", "index.sqlite-wal"]
        .iter()
        .map(|name| {
            vault
                .app_dir()
                .join(name)
                .metadata()
                .map(|m| m.len())
                .unwrap_or(0)
        })
        .sum::<u64>();
    println!("\nindex file: {:.0} MB", size as f64 / 1e6);

    // The same batch again, twice. The first one can still be busy with the
    // write ahead log of the rebuild, so its number is the cost of that log
    // and not of the batch; only the last of the three is the batch itself.
    for round in ["third", "fourth"] {
        write_burst(round);
        let (elapsed, ()) = time(|| {
            index
                .apply_changes(&vault, &changes)
                .expect("the changes are applied")
        });
        row(&format!("a batch again ({round})"), elapsed);
    }
}
