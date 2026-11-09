# Plugins

Jede Funktion von dvoix ist ein Plugin. Die API steht in `src/lib/plugins/types.ts`, die Verwaltung in `src/lib/plugins/manager.svelte.ts`. Die API ist so gebaut, dass später Community-Plugins darauf aufsetzen können, also gelten für Core-Plugins dieselben Grenzen.

## Regeln

- Ein Plugin = ein Ordner `src/lib/plugins/core/<id>/` mit einer `index.ts`, die ein `Plugin` exportiert (`export const <name>Plugin: Plugin = { ... }`).
- Eintragen in `corePlugins` in `src/lib/plugins/core/index.ts`. Die Reihenfolge dort ist die Lade- und Anzeigereihenfolge.
- `manifest.id`: klein, `a-z0-9-`, stabil. Sie ist der Ordnername in `.dvoix/plugins/<id>/` und der Schlüssel in `core-plugins.json`. Eine bestehende id nie umbenennen, sonst verlieren Nutzer ihre Einstellungen.
- `manifest.name` und `description` auf Deutsch, die Beschreibung ist ein Satz.
- Ein Plugin redet mit der App **nur** über den `ctx` aus `onload`: `addPanel`, `addRibbonAction`, `addCommand`, `registerFileType`, `registerEditorExtension`, `addFileFooter`, `addPaneHeader`, `addOverlay`, `registerView`, `addSettingTab`, `register`, `loadData`, `saveData`.
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
