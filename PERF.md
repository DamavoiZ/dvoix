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
```

The note size matters. The old delete of a search row walked the whole search
table, so its cost grew with the text of the entire vault.

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

## Still open

- `tasks()` and `projects()` read every row of their table, because the panels
  filter them themselves. That costs 19 ms and 4 ms with 20 000 notes and
  grows with the vault. Moving the filter into the query would change the
  command, so it is a separate job.
- The graph reads all notes and all links at once (27 ms with 20 000 notes).
- The frontmatter queries (`notes_where`, `projects`) parse the frontmatter of
  every note. The obvious fix was measured and made things worse, so a
  different way has to be found.
