# Frontend (Svelte 5, TypeScript, CodeMirror)

## Rules

- **Svelte 5 runes:** `$state`, `$derived`, `$effect`, `$props`. No `export let`, no `writable` stores, no Svelte 4 syntax (`on:click` → `onclick`).
- Shared state lives in `*.svelte.ts` modules in `src/lib/` (`vault.svelte.ts`, `ui.svelte.ts`, `settings.svelte.ts`) or in the plugin folder. No global variables in components.
- No file name may differ from another one in the same folder only by case, including the extension `.svelte.ts` (e.g. `ContextMenu.svelte` and `contextMenu.svelte.ts`). Windows and macOS do not distinguish this; the import `./contextMenu.svelte` then lands in the component and the app stays white.
- The window API (`@tauri-apps/api/window`) is only used through `src/lib/window.svelte.ts`. On Windows and Linux the window has no system frame (`src-tauri/tauri.windows.conf.json`, `src-tauri/tauri.linux.conf.json`); the title bar is the top row of the ribbon, sidebar and file pane. These areas attach `use:dragRegion`; buttons, tabs and other clickable elements inside them are excluded automatically. New window functions need the matching `core:window:allow-*` permission in `src-tauri/capabilities/default.json`.
- Call Rust only through `src/lib/api.ts`, never `invoke(...)` directly in components.
- Open, save and create files through `vault.svelte.ts`. Saving is automatic; views report changes only through `onchange`.
- Never store persistent settings in `localStorage`. App-wide vault settings go through `config.ts` (`readJson`/`writeJson`) into `.dvoix/*.json`, plugin settings through `pluginSettings`. Layout/tabs go into `workspace.json` via `ui.svelte.ts`.
- A view without its own file (e.g. the graph) is registered by its plugin with `ctx.registerView(...)` and opened with `vault.openViewTab(id)`. It runs in a tab like a file and appears in `workspace.json` as `dvoix://view/<id>`. When the plugin is switched off, its tabs close by themselves.
- Components in the app frame (`src/lib/components/`) stay feature-neutral. They render what plugins have registered (panels, footers, headers, overlays) and know no individual plugins.
- Markdown editor extensions come through `ctx.registerEditorExtension(...)` from the respective plugin, not directly into `src/lib/filetypes/markdown/editor.ts`.
- A note has three views: Read, Edit (live preview) and Source. Which one is active is stored in `settings.app.viewMode` (`.dvoix/app.json`); the views themselves live in `src/lib/filetypes/markdown/`. The live preview hides the Markdown syntax with CodeMirror decorations (`livePreview.ts`); new syntax rules belong there, not in the renderer. The properties (frontmatter) are shown by `Properties.svelte` in both views; changes to them run through `frontmatter.ts`, which changes only the affected lines. No YAML parser that rewrites the frontmatter. Tables are edited in the live preview in their cells (`tableWidget.ts`); the changes to the Markdown are computed by `markdownTable.ts`, which changes only the affected cell or row. New table operations go as pure functions into `markdownTable.ts` (testable without the editor), the interaction (selection, dragging, keys) into `tableWidget.ts`.
- Select lists use `src/lib/components/Select.svelte`, never a native `<select>`. Its open list is painted by the system; under Linux (WebKitGTK) it stays white despite CSS and `"theme": "Dark"`.
- Icons from `@lucide/svelte`. Styling with Tailwind and the CSS variables from `src/app.css`, so light and dark stay consistent.
- UI texts in English, short and without jargon. Label keyboard shortcuts with Ctrl/Cmd.
- The keyboard must work: dialogs close with Esc, lists can be used with the arrow keys and Enter.

## Checks

```sh
npm run check   # must pass without errors and without new warnings
npm run build
```

`npm run dev`, `npm run check` and `npm run build` first check with `scripts/check-deps.mjs` whether all dependencies from `package.json` are installed. If a package is missing, the message says so with a hint to `npm install`, instead of "Failed to resolve import …" from Vite.

For visible changes, start the app with `npm run tauri dev` and try it yourself. If that is not possible in your environment, say so in the PR.
