# Frontend (Svelte 5, TypeScript, CodeMirror)

## Regeln

- **Svelte 5 Runes:** `$state`, `$derived`, `$effect`, `$props`. Kein `export let`, keine `writable`-Stores, keine Svelte-4-Syntax (`on:click` → `onclick`).
- Geteilter Zustand liegt in `*.svelte.ts`-Modulen in `src/lib/` (`vault.svelte.ts`, `ui.svelte.ts`, `settings.svelte.ts`) bzw. im Plugin-Ordner. Keine globalen Variablen in Komponenten.
- Kein Dateiname darf sich von einem anderen im selben Ordner nur in Groß-/Kleinschreibung unterscheiden, auch nicht über die Endung `.svelte.ts` (z. B. `ContextMenu.svelte` und `contextMenu.svelte.ts`). Windows und macOS unterscheiden das nicht, der Import `./contextMenu.svelte` landet dort in der Komponente und die App bleibt weiß.
- Fenster-API (`@tauri-apps/api/window`) nur über `src/lib/window.svelte.ts`. Unter Windows hat das Fenster keinen Systemrahmen (`src-tauri/tauri.windows.conf.json`), die Titelleiste ist die obere Zeile von Ribbon, Seitenleiste und Dateiansicht. Diese Bereiche hängen `use:dragRegion` an; Knöpfe, Tabs und andere klickbare Elemente darin werden automatisch ausgenommen. Neue Fenster-Funktionen brauchen die passende `core:window:allow-*`-Berechtigung in `src-tauri/capabilities/default.json`.
- Rust nur über `src/lib/api.ts` aufrufen, nie `invoke(...)` direkt in Komponenten.
- Dateien öffnen, speichern, anlegen über `vault.svelte.ts`. Speichern ist automatisch; Ansichten melden Änderungen nur per `onchange`.
- Persistente Einstellungen nie in `localStorage`. App-weite Vault-Einstellungen über `config.ts` (`readJson`/`writeJson`) in `.dvoix/*.json`, Plugin-Einstellungen über `pluginSettings`. Layout/Tabs in `workspace.json` über `ui.svelte.ts`.
- Eine Ansicht ohne eigene Datei (z. B. der Graph) registriert ihr Plugin mit `ctx.registerView(...)` und öffnet sie mit `vault.openViewTab(id)`. Sie läuft in einem Tab wie eine Datei und steht in `workspace.json` als `dvoix://view/<id>`. Wird das Plugin ausgeschaltet, schließen ihre Tabs von selbst.
- Komponenten im App-Rahmen (`src/lib/components/`) bleiben feature-neutral. Sie rendern, was Plugins registriert haben (Panels, Footer, Header, Overlays), und kennen keine einzelnen Plugins.
- Markdown-Editor-Erweiterungen kommen über `ctx.registerEditorExtension(...)` aus dem jeweiligen Plugin, nicht direkt in `src/lib/filetypes/markdown/editor.ts`.
- Eine Notiz hat drei Ansichten: Lesen, Bearbeiten (Live-Vorschau) und Quelltext. Welche aktiv ist, steht in `settings.app.viewMode` (`.dvoix/app.json`), die Ansicht selbst liegt in `src/lib/filetypes/markdown/`. Die Live-Vorschau blendet die Markdown-Syntax mit CodeMirror-Decorations aus (`livePreview.ts`); neue Syntax-Regeln gehören dorthin, nicht in den Renderer.
- Icons aus `@lucide/svelte`. Styling mit Tailwind und den CSS-Variablen aus `src/app.css`, damit hell/dunkel einheitlich bleiben.
- UI-Texte auf Deutsch, kurz und ohne Fachjargon. Tastenkürzel mit Strg/Cmd beschriften.
- Tastatur muss gehen: Dialoge mit Esc schließen, Listen mit Pfeiltasten und Enter bedienbar.

## Prüfen

```sh
npm run check   # muss ohne Fehler und ohne neue Warnungen durchlaufen
npm run build
```

`npm run dev`, `npm run check` und `npm run build` prüfen vorher mit `scripts/check-deps.mjs`, ob alle Abhängigkeiten aus `package.json` installiert sind. Fehlt ein Paket, kommt das als Meldung mit dem Hinweis auf `npm install`, nicht als „Failed to resolve import …“ aus Vite.

Bei sichtbaren Änderungen die App mit `npm run tauri dev` starten und es selbst ausprobieren. Geht das in deiner Umgebung nicht, sag es im PR.
