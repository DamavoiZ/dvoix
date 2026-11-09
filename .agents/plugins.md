# Plugins

Jede Funktion von dvoix ist ein Plugin. Die API steht in `src/lib/plugins/types.ts`, die Verwaltung in `src/lib/plugins/manager.svelte.ts`. Store-Plugins (offiziell und Community) setzen auf dieselbe API auf, also gelten für Core-Plugins dieselben Grenzen.

## Core-Plugin oder Store-Plugin?

- **Core-Plugin** (`src/lib/plugins/core/`): Grundfunktionen, die fast jeder braucht und die eng mit der App verzahnt sind (Explorer, Suche, Aufgaben, Projekte, Tabs, Git, …). Wird mit der App ausgeliefert, lässt sich ausschalten.
- **Store-Plugin**: alles Zusätzliche, das nicht jeder will (z. B. Kanban, Tracking). Eigenes Repository mit Releases, Eintrag in `registry.json` von `DamavoiZ/dvoix-plugins`. Anleitung in `docs/plugins/README.md`.

## Plugin-Store (`src/lib/plugins/store/`)

- `store.svelte.ts` lädt Store-Plugins aus `.dvoix/plugins/<id>/` (`main.js` als ES-Modul über eine Blob-URL) und verwaltet `.dvoix/store.json`. Der Manager kennt sie über `plugins.loadExtra`/`unloadExtra`, nicht über `corePlugins`.
- `api.ts` ist die API für Store-Plugins: der `PluginContext`, aber mit `mount(el, props)` statt Svelte-Komponenten, SVG-Text als Icon, IDs mit `<plugin-id>:` davor und Berechtigungen (`vault:read`, `vault:write`, `editor`). Wird der `PluginContext` erweitert, hier und in `docs/plugins/README.md` nachziehen.
- Neue Berechtigung: in `PERMISSIONS` in `crates/dvoix-core/src/store.rs`, im Typ `PluginPermission` in `src/lib/api.ts`, in `PERMISSION_LABELS` und in der Doku.
- Downloads nur über `src-tauri/src/http.rs` (https, nur GitHub), URLs nur aus `store::release_url`/`latest_manifest_url`.
- Die Berechtigungen sind keine Sandbox. Nie so tun, als wären sie eine, weder in der UI noch in der Doku.

## Regeln

- Ein Plugin = ein Ordner `src/lib/plugins/core/<id>/` mit einer `index.ts`, die ein `Plugin` exportiert (`export const <name>Plugin: Plugin = { ... }`).
- Eintragen in `corePlugins` in `src/lib/plugins/core/index.ts`. Die Reihenfolge dort ist die Lade- und Anzeigereihenfolge.
- `manifest.id`: klein, `a-z0-9-`, stabil. Sie ist der Ordnername in `.dvoix/plugins/<id>/` und der Schlüssel in `core-plugins.json`. Eine bestehende id nie umbenennen, sonst verlieren Nutzer ihre Einstellungen.
- `manifest.name` und `description` auf Deutsch, die Beschreibung ist ein Satz.
- Ein Plugin redet mit der App **nur** über den `ctx` aus `onload`: `addPanel`, `addRibbonAction`, `addCommand`, `registerFileType`, `registerEditorExtension`, `addFileFooter`, `addPaneHeader`, `addOverlay`, `registerView`, `addExplorerEntries`, `addSettingTab`, `register`, `loadData`, `saveData`.
- Alles, was über `ctx` registriert wird, entfernt die App beim Ausschalten automatisch. Was ein Plugin selbst startet (Listener, Timer, Abos), gibt es per `ctx.register(dispose)` ab. Nach dem Ausschalten darf nichts vom Plugin übrig bleiben.
- Ein Plugin importiert nichts aus einem anderen Plugin-Ordner. Was mehrere brauchen, kommt nach `src/lib/` (z. B. `src/lib/tasks.ts`, `src/lib/fuzzy.ts`).
- Braucht ein Plugin etwas, das die API nicht kann, wird die API in `types.ts` + `manager.svelte.ts` erweitert (mit automatischem Aufräumen) und im README in der Plugin-Tabelle ergänzt. Kein Umweg an der API vorbei.
- Jedes Plugin muss sich ein- und ausschalten lassen, ohne dass die App kaputtgeht oder andere Plugins betroffen sind. Teste beides.

## Einstellungen eines Plugins

Mit `pluginSettings` aus `src/lib/plugins/pluginSettings.svelte.ts`, gespeichert in `.dvoix/plugins/<id>/data.json`:

```ts
// settings.ts
export const mySettings = pluginSettings({ showDone: false });

// index.ts, in onload
await mySettings.load(ctx);
ctx.addSettingTab({ component: MySettings });

// in einer Komponente
mySettings.value.showDone = true;
mySettings.save();
```

Jeder Schlüssel hat einen Standardwert, fehlende Schlüssel werden beim Laden ergänzt. Neue Schlüssel nur hinzufügen, bestehende nicht umbenennen oder umdeuten. Die Einstellungsseite baut auf `src/lib/components/settings/Setting.svelte` und `Toggle.svelte` auf. Vorlage: `core/tasks/` oder `core/file-explorer/`.

## Befehle und Tastenkürzel

`ctx.addCommand({ id, name, hotkeys, run })`. Die volle id ist `<plugin-id>:<id>`. `mod: true` heißt Strg unter Windows/Linux und Cmd unter macOS. Vor einem neuen Kürzel prüfen, dass es nicht schon belegt ist (`grep -rn "hotkeys" src/lib/plugins`). Neue Kürzel kommen ins README.
