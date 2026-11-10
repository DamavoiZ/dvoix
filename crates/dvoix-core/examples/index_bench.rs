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

    let (elapsed, tags) = time(|| index.tags().expect("the tags are read"));
    row(&format!("tags ({} tags)", tags.len()), elapsed);

    let (elapsed, tasks) = time(|| index.tasks().expect("the tasks are read"));
    row(&format!("tasks ({} tasks)", tasks.len()), elapsed);

    let (elapsed, projects) = time(|| index.projects().expect("the projects are read"));
    row(&format!("projects ({} projects)", projects.len()), elapsed);

    let (elapsed, graph) = time(|| index.graph().expect("the graph is read"));
    row(
        &format!("graph ({} notes, {} links)", graph.notes.len(), graph.links.len()),
        elapsed,
    );

    // The removal of a folder with 100 notes in it.
    let (elapsed, ()) = time(|| index.remove("folder50").expect("the folder is removed"));
    row("remove a folder of 100 notes", elapsed);
}
