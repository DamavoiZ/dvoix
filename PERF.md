# Performance

What was measured for dvoix, what it was, and what came out of it. Every
number comes from `cargo run --release --example index_bench -- <notes> <bytes>`,
which builds a vault of synthetic notes and times the operations of the index.

The same command is used before and after a change, with the same note count
and the same note size, so the numbers are comparable.

## How to measure

```sh
# 20 000 notes of 2 kB each: a big but normal vault.
cargo run --release --example index_bench -- 20000 2000

# 80 000 notes: the case where the old costs really showed.
cargo run --release --example index_bench -- 80000 2000

# The editor: the live preview and the check of the note.
node scripts/bench-editor.mjs

# The panels: the lookups of links, the tree of the vault, the suggestions.
node scripts/bench-panels.mjs
```

The note size matters. The old delete of a search row walked the whole search
table, so its cost grew with the text of the entire vault.

The numbers move a little from run to run. Every table below is one run; the
numbers of the editor were taken from a run that was repeated three times.

## The results

20 000 notes of 2 kB, in milliseconds:

| Operation | Before | After | |
| --- | --- | --- | --- |
| re-index one note | 13.68 | 0.42 | **33×** |
| write and re-index one note | 12.94 | 0.25 | **52×** |
| remove a folder of 100 notes | 32.72 | 2.93 | **11×** |
| search (50 hits) | 16.85 | 16.49 | unchanged |
| tasks (20 000) | 19.08 | 19.01 | unchanged |
| tags | 0.78 | 0.75 | unchanged |
| projects | 4.33 | 3.82 | unchanged |
| graph | 27.53 | 40.16 | noise, the graph does not delete |
| full rebuild | 1188 | 1229 | 3 % slower, see below |

80 000 notes of 2 kB, in milliseconds:

| Operation | Before | After | |
| --- | --- | --- | --- |
| re-index one note | 44.14 | 0.39 | **113×** |
| write and re-index one note | 43.69 | 0.23 | **190×** |
| remove a folder of 100 notes | 108.61 | 10.14 | **11×** |
| search (50 hits) | 67.04 | 66.23 | unchanged |
| full rebuild | 4477 | 4849 | 8 % slower, see below |

The gain grows with the vault, because the old cost grew with it and the new
cost does not.

### Opening a vault

The index is a cache of the files, so it can be kept between two runs of the
app. It was thrown away and built from scratch every time a vault was opened,
which meant reading and parsing every note of the vault on every start.

Now the index remembers the moment of change and the size of every note. A
note is read again only when one of the two differs. A full rebuild stays
available as the way out.

20 000 notes of 2 kB:

| | Before | After |
| --- | --- | --- |
| open a vault that did not change | 1213 ms | 55 ms |
| open a vault with one changed note | 1213 ms | 55 ms |

80 000 notes of 2 kB:

| | Before | After |
| --- | --- | --- |
| open a vault that did not change | 5169 ms | 222 ms |
| open a vault with one changed note | 5169 ms | 242 ms |

The rest of the 222 ms is the walk over the files of the vault, which the
index needs to know what is there. The notes themselves are no longer read.

A note whose moment of change and size are both the same as the index knows
is taken as unchanged. A note that was written in the same millisecond as the
last index build and has the same size as before would therefore be missed.
That is rare, and the full rebuild is still there for the cases where it
matters.

### Renaming a folder

Renaming a folder re-indexed the whole vault. Now only the notes of that
folder get their new path; the rest of the vault stays as it is.

### Reacting to changes in the vault

Every change the watcher reported was written to the index on its own, in its
own transaction. A `git checkout` or a sync tool writes hundreds of files at
once, and every one of them cost a transaction with a flush to the disk.

The changes are now collected until the vault is quiet for a moment and are
then written in one go:

| 100 changed notes (20 000 notes in the vault) | Before | After |
| --- | --- | --- |
| every change on its own | 23.45 ms | — |
| all of them in one batch | — | 15.72 ms |

A note that dvoix wrote itself is not read a second time: the index knows the
moment of change and the size of the note and sees that they did not change.

### Listing the vault

The listing of the vault walked the files and asked the file system for the
information of every entry a second time, and it sorted the entries with a
key that was built again for every comparison.

| 20 100 entries | Before | After |
| --- | --- | --- |
| list the vault | 45.26 ms | 37.40 ms |
| open a vault that did not change | 53.35 ms | 44.33 ms |

### The lock of the workspace

Every command held the lock of the workspace while it worked, so a slow
listing of the vault held up reading a file. The lock is now only held to
get the vault; the work happens outside of it.

### The editor

The editor had three costs that were paid on every keystroke.

**The note was put into the editor again.** After every change the editor
looked at the whole note to see whether it had to be replaced. The check
copied the note twice: once to turn it into the text the editor keeps, and
once to compare it with the text of the editor. The editor now knows the
text it produced itself and skips the check.

| Note | Before | After |
| --- | --- | --- |
| 20 000 characters | 0.009 ms | 0.0000 ms |
| 100 000 characters | 0.043 ms | 0.0000 ms |

The new check compares the text of the editor with the text of the note.
Both are the same string, so the answer is known without looking at it.

**The live preview was built again.** The live preview walks the syntax tree
of the whole note to find the properties and the tables, and it did that
again for every change of the cursor. The blocks are now kept, so a change
of the cursor only redraws them. A change of the text walks the tree again,
but a block whose text did not change keeps its widget.

| Note | Cursor move before | Cursor move after |
| --- | --- | --- |
| 5 tables (363 characters) | 0.021 ms | 0.004 ms |
| 50 tables (3 013 characters) | 0.025 ms | 0.004 ms |
| 200 tables (11 963 characters) | 0.023 ms | 0.002 ms |

A keystroke gets a little cheaper as well, because the widget of a table
that did not change is not built again:

| Note | Before | After |
| --- | --- | --- |
| 5 tables | 0.082 ms | 0.051 ms |
| 50 tables | 0.065 ms | 0.065 ms |
| 200 tables | 0.060 ms | 0.058 ms |

**The reading view was rendered again.** Every change of a property sent the
whole note to the backend and put the answer into the page again, even
though the text of the note had not changed. The reading view now watches
the text of the note and leaves the properties to the properties. The
backend renders a note of 3 140 characters in 0.044 ms, but the round trip
through the backend and the replacement of the HTML in the page are what
the change avoids.

The line endings of a note were also looked for in the whole text on every
keystroke. They are decided once per note now.

### The panels

Three things happened on every change on disk, and all three were done for
the whole vault.

**The vault was listed again.** Every change asked the backend for the list
of all the files of the vault, and the app got 20 000 entries over the
connection and made a proxy of every one of them. Now only a file that was
written is corrected in place; the vault is listed again when a file was
added or removed, because then the tree of the vault really changes.

The list itself costs 37 ms with 20 100 entries (see above), and that is
what a save of a note used to cost.

**Every panel refreshed.** The panels did not know what had changed, so
every one of them asked the backend again. Now the vault keeps three
counters: a note changed, the shape of the vault changed, any file changed.
The panels of notes (tags, tasks, projects, backlinks, search, graph) read
the first, the panels of the repository read the last one. A file that is
not a note changes none of the panels of notes.

**The lookups of links walked over all the files.** Following a link or
looking for the name of a file looked at every file of the vault, twice
over, with a `toLowerCase()` for every one of them. The files are now in
two maps, by path and by name, that are built once per change of the list.

| 20 000 files | Before | After |
| --- | --- | --- |
| look for the target of a link | 1.32 ms | 0.0003 ms |
| build the maps once | — | 3.9 ms |

The tree of the vault is built once per change of the list as well. Its
names were compared with `localeCompare`, which builds a new collator for
every single comparison.

| 20 100 entries | Before | After |
| --- | --- | --- |
| build the tree | 72.0 ms | 6.3 ms |
| add the entries of the plugins | 10.5 ms | 5.7 ms |

The suggestions of links were built again for every keystroke: every file
of the vault got a label, a name and a place in the list, and the list was
sorted. That happens now once per change of the vault.

| 20 000 files | Before | After |
| --- | --- | --- |
| build the targets | 13.7 ms | 8.4 ms |
| build the targets | every keystroke | once per change |
| walk the targets for a suggestion | — | 0.26 ms |

## The attempts

### Kept: the search row takes the rowid of its note

The search table is `fts5(path UNINDEXED, title, body)`. `path` is stored but
not indexed, so `DELETE FROM notes_fts WHERE path = ?` could only walk the
whole table. The plan said so:

```
EXPLAIN QUERY PLAN DELETE FROM notes_fts WHERE path = 'x.md';
-->SCAN notes_fts VIRTUAL TABLE INDEX 0:
```

The note's rowid is now written into the search row, and the row is deleted by
that rowid. The plan of the new delete is `INDEX 0:=` — a lookup, not a walk.

The search itself did not change: the rowid is a hidden column, so the query
and its `snippet()` call are the same as before.

### Kept: a `NOCASE` index for the delete of a folder

The delete of a folder used `path LIKE 'folder/%'`. The indexes were `BINARY`,
so SQLite could not use them and walked every table. An index with
`COLLATE NOCASE` makes the same query a range search, and keeps the exact
case-insensitive meaning of `LIKE`:

```
EXPLAIN QUERY PLAN DELETE FROM notes WHERE path LIKE 'd5/%';
-->SEARCH notes USING INDEX notes_nocase (path>? AND path<?)
```

### Kept: no copy for a link target that is already lower-case

Link targets are stored lower-cased. Most of them already are, so the copy is
skipped. Text outside ASCII is always copied, because `to_lowercase` knows more
characters than ASCII does.

### Accepted: the full rebuild is a little slower

The three extra `NOCASE` indexes have to be filled when the whole vault is
indexed, which makes a full rebuild about 3 % slower (8 % with 80 000 notes).
A rebuild happens once when a vault is opened, while a note is re-indexed on
every save, so the trade is clearly worth it. It matters even less once the
rebuild only touches the notes that changed.

### Rejected: an index on `path` of the search table

Measured 6.01 ms → 5.75 ms for the delete at 20 000 notes. FTS5 does not build
a usable index for such a column, so the delete still walks the table. Not
worth the change to the schema.

### Rejected: a search table that takes its content from the notes table

`fts5(title, body, content='notes', content_rowid='rowid')` deletes in 0.01 ms
instead of 6.01 ms. It is the fastest way, but it changes the schema of the
search table, needs the `'delete'` command for every removal and rewrites the
search query. The rowid of the note reaches the same goal with a much smaller
change.

### Rejected: an index on `json_extract(frontmatter, '$.type')`

For `projects()` this made the query **118 times slower**: 114 ms → 13 558 ms
with 50 000 notes. The expression index gets in the way of the plan that joins
the tasks. Not used.

### Rejected: a table with the frontmatter fields of every note

`notes_where` was to look up the frontmatter in a table of its own
(`path`, `key`, `value`). Measured 7.5 ms → 13.1 ms at 20 000 notes: the join
costs more than the scan it replaces. Not used.

### Rejected: an index on `tasks(path, line)`

`tasks()` was to read its rows in order. Measured 65 ms → 70 ms. The query
reads every task anyway, so the index does not help. Not used.

### Rejected: a range instead of `LIKE` for the delete of a folder

`path >= 'd5/' AND path < 'd50'` is as fast as the `NOCASE` index (0.011 ms
against 0.012 ms), but it needs the byte after the folder name and it changes
the meaning of the query from "case-insensitively like" to "in this range". The
`NOCASE` index keeps the meaning of the old query, so it was kept.

### Rejected: a map of the live preview blocks, keyed by their text

The blocks of the live preview were first looked up by their text in a map,
so a block that moved would keep its widget. The map costs more than it
saves: the blocks are matched by their place in the document, which needs no
lookup at all, and the map made a keystroke slower. Not used.

### Rejected: sorting the names of the tree in lower case

The names of the tree could be compared in lower case instead of with a
collator. That is faster, but it is a different order: a collator puts "a"
before "B", a comparison in lower case cannot tell them apart. The order of
the tree is what the user sees, so the collator was kept — but only one of
them, for all the comparisons.

## Still open

- `tasks()` and `projects()` read every row of their table, because the panels
  filter them themselves. That costs 19 ms and 4 ms with 20 000 notes and
  grows with the vault. Moving the filter into the query would change the
  command, so it is a separate job.
- The graph reads all notes and all links at once (27 ms with 20 000 notes).
- The frontmatter queries (`notes_where`, `projects`) parse the frontmatter of
  every note. The obvious fix was measured and made things worse, so a
  different way has to be found.
