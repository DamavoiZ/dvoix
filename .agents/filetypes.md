# File types

A file type is a `FileType` (`src/lib/filetypes/types.ts`) and is registered by a plugin with `ctx.registerFileType(...)`. Example from the code: `src/lib/plugins/core/canvas/`.

## Rules

- The format is readable text on disk (JSON, Markdown, YAML …). Prefer an open, existing format (like JSON Canvas for `.canvas`); otherwise a simple custom one that people can understand without dvoix and edit by hand.
- The view receives `path`, `content`, `onchange`, `onopen` (`FileViewProps`). It calls `onchange(newContent)` on every change. **The app saves**; the view never writes by itself.
- The view must cope with content that changes from outside (sync, git, another editor): take over the new `content` without losing input.
- Never silently overwrite broken or unknown content. Better to show an error and leave the file untouched.
- Parsing and serializing live in their own file next to the view (like `canvas/jsonCanvas.ts`), so the logic stays readable without UI. Keep unknown fields when writing back.
- `extensions` short and without a dot. Only give a `template` if users should be able to create the type in the app; then "Create <label>" appears in the explorer.
- If the type should be searchable, `crates/dvoix-core/src/index.rs` needs an extraction for the extension (see `is_indexable`) plus a test.
- New file type → update the README (section "What already works" and, if needed, "Next steps").

## Checklist for a new file type

1. `src/lib/plugins/core/<id>/` with `<Name>View.svelte`, a format module and `index.ts`.
2. Register it in `corePlugins` in `core/index.ts`.
3. Create, edit, change from outside, turn the plugin off and on: everything must work.
4. `npm run check` and `npm run build`.
