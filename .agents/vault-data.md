# Vault, file formats and `.dvoix/`

## Principle

The files in the vault are the only source of truth. Users also edit them with other programs (VS Code, Obsidian, git, sync tools). dvoix must cope with that and must not destroy their formatting.

- Never reformat a file the user has not edited. Change only the characters that are needed.
- Unknown frontmatter, unknown JSON fields and unknown lines are kept.
- Stay compatible with common Markdown conventions (Obsidian, CommonMark, YAML frontmatter), so a vault stays readable without dvoix.
- The watcher (`watcher.rs`) detects external changes. The UI must take them over.

## `.dvoix/`

```
.dvoix/
  app.json                  editor settings
  appearance.json           font sizes
  core-plugins.json         which plugins are on
  workspace.json            open tabs, active panel, width, expanded folders
  plugins/<id>/data.json    settings of a plugin
  index.sqlite              search index, cache only
```

- If a file or key is missing, default values apply. Broken JSON must not crash the app (`readJsonRaw` then returns `undefined`).
- Document new files here in the README section "The `.dvoix/` folder" and in the header comment of `src/lib/config.ts`.
- Do not rename keys. If a format has to change, take over old values when loading (migration), do not discard them.

## Tasks and projects

- Task = Markdown checkbox `- [ ]` / `- [x]` in any note.
- Due date: `📅 2026-12-01` or `due:2026-12-01`. Priority: `!high` `!medium` `!low` (German `!hoch` `!mittel` `!niedrig` are still accepted) or ⏫ 🔼 🔽. Keep it compatible with the Obsidian Tasks plugin.
- Project = note with `type: project` (or the German alias `projekt`) in the frontmatter, fields `status`, `due`, `start`, `priority`, `area`. English and German values are both accepted (`project.rs`). New projects are written with the English values.
- New fields or spellings: parser in the Rust core (`tasks.rs`, `project.rs`) with tests, TS types in `api.ts`, a table in the README.

## New data kinds (tracking etc.)

First check whether frontmatter or a Markdown convention is enough. Only if not, create a dedicated file type (see `filetypes.md`). Never use a separate database as storage.
