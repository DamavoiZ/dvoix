# Performance

What was measured for dvoix, what it was, and what came out of it. Every
number comes from `cargo run --release --example index_bench -- <notes> <bytes>`,
which builds a vault of synthetic notes and times the operations of the index,
unless a section names another source for it.

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

# The editor while it scrolls: the decorations per scroll step.
node scripts/bench-scroll.mjs

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

### The search while it is still being typed

Every word of a search is matched as a prefix (`fts_query`), because the user
has usually not finished typing it. Without an extra index, FTS5 has to walk
its term dictionary to find the beginnings: the search of a vault with many
different words can cost a few seconds, although it answers in a millisecond
for a word that is rare. An index over the first letter of every word
(`prefix='1'` on the search table, schema version 6) makes that lookup cheap.

80 000 notes of 2 kB, in milliseconds:

| Search | Before | After | |
| --- | --- | --- | --- |
| `s`, a letter of nearly every note | 155 | 90 | **1.7×** |
| a whole word | 67 | 66 | unchanged |
| one letter, with a vocabulary of 20 000 words | 4357 | 210 | **21×** |
| full rebuild | 4276 | 5026 | 18 % slower |
| 100 changes in one batch | 36 | 35 | unchanged |
| the file of the index | 491 MB | 563 MB | 15 % bigger |

All rows but the one for a big vocabulary come from `index_bench`. That row
comes from a corpus of the same size whose notes use 20 000 different words:
the text of the bench repeats the same few words, so its term dictionary stays
small and cannot show the worst case.

The rebuild is what the extra index costs: once per version of the schema, or
when the index file is deleted. The search is what happens on every keystroke.

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

### Scrolling the editor

The live preview rebuilt its decorations whenever the rendered range moved.
On Linux the webview (WebKitGTK) animates a wheel scroll over many frames, and
every frame that moved the range rebuilt the decorations of the whole window
and made the editor measure the heights again, which showed as stutter.

Now a scroll step only decorates the lines that came into view and joins them
into the set that is kept; nothing that is already drawn is touched. The full
rebuild, which also drops what scrolled away, waits until the scroll has been
quiet for 120 ms (`src/lib/editorScroll.ts`). Typing, a click and every other
transaction still decorate right away, so the syntax never lags behind an
edit, and a line that comes into view never shows its raw markers.

The work per scroll burst, from `node scripts/bench-scroll.mjs` (a window of
160 lines scrolled in steps of 8, a burst of 30 steps between two settles),
in milliseconds:

| Note | Rebuild the window every step | Cover only the new lines | |
| --- | --- | --- | --- |
| 200 sections (3 445 lines) | 6.80 | 1.53 | **4.4×** |
| 400 sections (6 885 lines) | 12.05 | 2.56 | **4.7×** |
| 1500 sections (25 805 lines) | 42.02 | 9.73 | **4.3×** |

The bench models the decoration logic on the real markdown parser; the frame
time in the app itself could not be measured outside the webview. What it
shows is the work the change takes out of the scroll frames: the height
measure of a widget that stays on screen is not paid again on every step.

An embedded image also reserves its height before it is drawn. The size of
the file is remembered when it was first loaded (`embeddedImageHeight` in
`src/lib/attachments.ts`), and the image widget reports it to CodeMirror, so
the lines below the viewport do not jump when the image arrives. The height
of an embed line off screen, measured with the real editor in a headless
browser (a Vite harness that mounts the editor with a mocked Tauri bridge and
an image of 800 × 450 px, driven through the Chrome DevTools Protocol):

| | Before | After |
| --- | --- | --- |
| height reserved for the line | 26 px (its text) | 509 px (the image) |

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

### The panels, round 2

The counters told the panels when to refresh, but a batch of changes still
moved them once per file, and several panels did their own work on every
keystroke. This round takes the bursts and the typing out of the hot paths.

**A batch moves each counter once.** The backend hands a batch of watcher
changes over as one event; the vault corrects the entries of the whole batch
and then moves each change group once. A `git checkout` of 100 files is one
refresh of every panel, not 100.

**The app does not read its own saves back.** Every save came back from the
watcher and the open file was read from disk again, even though the text was
already on screen. The app remembers what it wrote (path, size, moment of
change), skips the echo of its own write and corrects the entry of the file
right away, so views of it (an embedded image) see the new version without
waiting for the watcher.

**Typing does not re-render the sidebar.** The outline parsed the whole note
on every keystroke, and a docked copy of the open note was rendered by the
backend on every keystroke. Both wait until the typing has settled (150 ms
for the outline, 200 ms for the docked note); a note docked from disk is read
once per batch of changes, and only the properties changing does not render
it again (the same guard as in the reading view).

**The graph waits for the typing.** Every letter in the filter field rebuilt
the whole graph (all nodes, all lines, the layout). The settings that change
nodes and lines are applied 200 ms after the last input; the data load runs
once at a time.

**Git reads its state once.** The panel called `git status` for every
changed file and again every few seconds, even when the window was in the
background. Now the refresh is debounced, only one runs at a time (requests
during it become one more afterwards), the timer rests while no panel shows
the state or the window is hidden (opening the panel catches up right
away), and the history loads once per batch with a guard against
out-of-order answers.

**The tasks panel does the work once.** The filter lower-cased the text,
title and tags of every task for every query, and the due-date bucket of a
task was computed again for every row that showed it. The search text is
built once per load, the bucket once per task:

| 5 000 tasks | Before | After |
| --- | --- | --- |
| filter a query | 0.49 ms | 0.14 ms |
| build the search texts | every query | 0.97 ms once per load |

Checking off a task listed the whole vault again (37 ms with 20 000 files);
now the watcher's report is enough, and the tick shows right away, before
the file is written.

**Long lists are drawn in pages.** A folder with thousands of entries, the
tasks of a due date, the notes of a tag and the backlinks of a note were all
drawn at once, thousands of DOM rows. Folders draw 500 rows and the panels
200, with "Show more" for the rest; the open note is always revealed. The
backlinks footer lists 50 links and opens the rest behind "and N more".

| 5 000 entries in a folder | Before | After |
| --- | --- | --- |
| rows to draw on open | 5 000 | 500 |

**The table widget keeps what did not change.** Adding a row built the whole
table DOM again; now rows whose text did not change keep their DOM (and the
cell being typed in). Growing a table by a paste re-reads only the changed
lines instead of the whole table for every added row. Dragging a row or
column finds the drop place by measuring the grips on every mouse move, but
the scan stops at the first grip below the pointer and the drop marks are
only written when the drop place changes, so the layout stays clean. The
positions are measured live (and again on mouse up) on purpose: the table
can scroll during a drag, and a snapshot taken at the drag start would then
name a row the scrolled distance away from the mouse.

**Opening the vault reads in parallel.** The settings files, `store.json`
and the app version, and a store plugin's `main.js` and `styles.css` do not
depend on each other and are read at once instead of one after another.

### The index, round 2

20 000 notes of 2 kB, in milliseconds, measured with `index_bench` before
and after this round:

| Operation | Before | After | |
| --- | --- | --- | --- |
| full rebuild | 1359 | 1028 | **1.3×** |
| re-index one note | 2.96 | 0.32 | **9×** |
| write and re-index one note | 2.39 | 0.36 | **7×** |
| sync (nothing changed) | 65 | 50 | |
| remove a folder of 100 notes | 4.57 | 2.59 | |
| 100 changes one by one | 216 | 64 | **3.4×** |
| the same 100 changes in one batch | 20.9 | 13.9 | |
| graph | 21 | 17 | |
| search / tasks / tags | ~unchanged | | |

**Renaming a folder only moves paths.** The rename deleted the old rows and
read, parsed and indexed every note of the folder again, although only the
paths change. Now the paths are moved with SQL updates; the notes below the
destination are read only when the index does not know them yet (a rename
out of a hidden folder), and known notes are skipped by their moment of
change and size. Names with `%` or `_` are escaped, and a rename that only
changes the case works too. A test compares the result with a full rebuild
of the same vault.
The search table holds no path of its own (see below), so a rename does not
reach it at all. With 20 000 notes in the index the rename of a folder of 100
notes costs 3.9 ms; while the table still carried a copy of every path the same
rename cost up to 96 ms, and at 80 000 notes between 284 and 371 ms.

**The write path prepares once.** Every tag, link and task of a note was
written with a freshly prepared statement; the statements are cached now,
and empty frontmatter and tag lists take a fast path. The inline scanner
collected every text run into a `Vec<char>` (four bytes per character) just
to look for `#`; it walks the string directly now. Ticking a task parsed
every task of the note to find one line; only that line is parsed now.

**The watcher does less per event.** The kind of an event is decided before
the file is asked for its metadata, removals need no metadata at all, and a
batch keeps only the last change per path, but a Created survives a
following Modified and a Removed starts the story over (a folder moved
into the vault is still new, its notes must be indexed; the index merges a
raw batch by the same rules). The index reuses the metadata the watcher already
collected instead of asking the file system again.

**SQLite is tuned for a cache.** `synchronous=NORMAL` (the index can always
be rebuilt from the files, rule 1), a larger page cache, memory temp store
and a memory-mapped read window. The secondary indexes are dropped before a
full rebuild and built once over the finished tables, which measured faster
than maintaining them per row. `PRAGMA optimize` was tried and rejected: its
statistics made the planner leave the `NOCASE` path index for folder deletes
and backlinks (two plan tests fail). Nothing collects statistics anymore, so
the planner decides by the shape of the indexes, which is what this schema
is built for; a test pins that no statistics table exists.

**Links are unique.** A `UNIQUE(source, target)` index (schema version 5,
one rebuild on the first open) lets the graph read a plain covering index
scan instead of sorting and deduplicating the whole link table.

### The shell, round 2

**Git state costs one process.** `git status` spawned up to eight `git`
processes (discovery, status, branch, upstream, remote, ahead/behind, two
branch lists) on every refresh. One `git status --porcelain=v2 --branch -z`
now carries the changes and the branch headers, the remote is derived from
the upstream, and the branch list is one `for-each-ref`. Unmerged paths are
reported like before. `--untracked-files=all` is kept on purpose: `normal`
reports untracked folders as one entry, which breaks the per-file panel.

**Downloads keep their connection.** Every store download built a new HTTP
agent and paid a full TLS handshake; one shared agent is reused now, and an
install fetches `main.js` and `styles.css` at the same time.

**Plugin processes stream in windows.** Output was sent to the webview once
per 8 kB read; it is collected per stream over 25 ms now (nothing is lost
when the streams switch). The exit was polled every 50 ms; the reaper waits
on the process now, and a kill goes through the stored handle or the pid.

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

### Kept: the search table keeps no copy of the path

The rowid made the delete cheap, but the table still stored a copy of every
path, and a copy has to be written: the insert of a note paid for it, and a
folder rename had to rewrite it for every note below. Isolating the statements
of a rename measured the batch over the four tables at 8 to 13 ms for 800 rows
and the same batch with the rewrite of the copy added at 87 to 93 ms, so the
copy alone costs about ten times what the real work costs.

The rename of a folder of 100 notes, measured by `index_bench` on the same
vault before and after the column went away:

| Vault | With the copy | Without it |
| --- | --- | --- |
| 20 000 notes | 25 to 96 ms | 3.9 ms |
| 80 000 notes | 284 to 371 ms | 14.6 ms |

The path is never searched, so the column can go: `search()` reads it through
the rowid instead.

```
SELECT notes.path, notes_fts.title, snippet(notes_fts, 1, '«', '»', ' … ', 12)
FROM notes_fts JOIN notes ON notes.rowid = notes_fts.rowid
WHERE notes_fts MATCH ?1 ORDER BY rank LIMIT ?2
```

The join costs nothing measurable: a search over 20 000 notes runs in 10.23 ms
with the column and 10.22 ms with the join, and a query that matches a single
note is below a hundredth of a millisecond either way. The file of the index
grows by about a megabyte for the copy (measured on 20 000 notes of 2 kB with
nothing but the notes and the search table in the file: 107 MB against 106 MB),
because the body of a note dwarfs its path. The win is the writing, not the
size.

One trap: dropping the first column moves the others, so the body of the note
is column 1 and not 2, and `snippet()` has to be told. A test checks the exact
snippet text and the exact path of a hit, because a wrong column still returns
a hit and only the text gives it away.

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
worth the change to the schema. The column itself went later, for the cost of
writing it rather than reading it (see above).

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

### Rejected: prefix indexes over two and three letters

`prefix='1 2 3'` was measured before `prefix='1'` was kept. It answers the
same searches at the same speed (4357 ms → 212 ms for the one-letter case),
but it costs 58 % more on a rebuild (6959 ms → 10994 ms) and 45 % more on the
file (445 MB → 648 MB), and re-indexing a batch of notes gets 74 % slower
(1507 ms → 2630 ms with 80 000 notes). Two- and three-letter beginnings are
already cheap enough in the plain dictionary not to need their own index: a
search for `wor` costs 43 ms without any prefix index and 41 ms with the
three-level one.

### Rejected: sorting the names of the tree in lower case

The names of the tree could be compared in lower case instead of with a
collator. That is faster, but it is a different order: a collator puts "a"
before "B", a comparison in lower case cannot tell them apart. The order of
the tree is what the user sees, so the collator was kept — but only one of
them, for all the comparisons.

### Rejected: sorting the search results in the query instead of by rank

FTS5 answers the search `ORDER BY rank`, which is its own BM25 score. Measured
on a corpus where one word is in every note, the sort costs the whole query:
0.4 ms without it and 37 ms with it, because the score of every match has to
be known before the best fifty can be picked. Taking the first 400 matches and
sorting only those is 4 ms, but the fifty that come out are a different set:
not one of the fifty best overlapped. The order of the results is what makes
the search useful, so the honest sort was kept and the cheaper wrong one
rejected. What was kept instead is the prefix index above, which is the part
that actually grows with the vault.

## Still open

- `tasks()` and `projects()` read every row of their table, because the panels
  filter them themselves. That costs 19 ms and 4 ms with 20 000 notes and
  grows with the vault. Moving the filter into the query would change the
  command, so it is a separate job.
- The graph reads all notes and all links at once (27 ms with 20 000 notes).
- The frontmatter queries (`notes_where`, `projects`) parse the frontmatter of
  every note. The obvious fix was measured and made things worse, so a
  different way has to be found.
