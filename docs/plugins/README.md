# Plugins für den Plugin-Store schreiben

dvoix hat zwei Arten von Plugins:

- **Core-Plugins** sind in die App eingebaut (Datei-Explorer, Suche, Aufgaben, Git, …) und lassen sich in den Einstellungen unter „Core-Plugins“ ausschalten. Sie liegen in `src/lib/plugins/core/`.
- **Store-Plugins** installiert man bei Bedarf über **Einstellungen → Plugin-Store**. Sie liegen danach im Vault unter `.dvoix/plugins/<id>/`. Es gibt **offizielle** Plugins (von dvoix gepflegt, immer installierbar) und **Community-Plugins** (von anderen, erst nach dem Schalter „Community-Plugins erlauben“).

Diese Seite beschreibt Store-Plugins. Ein vollständiges Beispiel steht in [`hello/`](hello/).

## Aufbau

Ein Plugin ist ein GitHub-Repository mit Releases. Jedes Release hat als Dateien (Assets):

| Datei | Pflicht | Inhalt |
| --- | --- | --- |
| `manifest.json` | ja | Name, Version, Berechtigungen (siehe unten) |
| `main.js` | ja | der Code als ES-Modul, alles in einer Datei |
| `styles.css` | nein | Styles, werden geladen, solange das Plugin an ist |

Der **Tag des Releases ist die Version** aus `manifest.json`, ohne `v` davor (z. B. `1.2.0`). Die App holt zuerst `manifest.json` aus dem neuesten Release (`releases/latest`) und dann `main.js` und `styles.css` aus dem Release mit diesem Tag.

### manifest.json

```json
{
  "id": "hello",
  "name": "Hallo Welt",
  "version": "1.0.0",
  "description": "Ein Satz, was das Plugin macht.",
  "author": "dein-name",
  "minAppVersion": "0.1.0",
  "permissions": ["vault:read"]
}
```

- `id`: klein, `a-z0-9-`, nie ändern. Sie ist der Ordnername in `.dvoix/plugins/<id>/` und muss zur `id` im Plugin-Verzeichnis passen. IDs von Core-Plugins (`tasks`, `git`, …) sind vergeben.
- `version`: `Major.Minor.Patch`, nur Zahlen.
- `minAppVersion`: älteste dvoix-Version, auf der das Plugin läuft (optional).
- `permissions`: was das Plugin von der API braucht:

| Berechtigung | erlaubt |
| --- | --- |
| `vault:read` | `dvoix.vault.list()`, `dvoix.vault.read(path)` |
| `vault:write` | `dvoix.vault.write(path, content)` |
| `editor` | `dvoix.registerEditorExtension(...)` |

Die Berechtigungen stehen im Store, bevor jemand installiert. Ruft ein Plugin etwas auf, das es nicht angegeben hat, wirft die API einen Fehler. Eine Sandbox ist das nicht: Ein Plugin läuft wie bei Obsidian in der App und könnte die API umgehen. Deshalb sind Community-Plugins hinter einem Schalter mit Warnung.

### main.js

```js
export default {
  async onload(dvoix) {
    dvoix.addCommand({ id: "hallo", name: "Hallo sagen", run: () => alert("Hallo!") });
  },
  onunload() {},
};
```

`onload` läuft beim Einschalten (oder beim Öffnen des Vaults), `onunload` beim Ausschalten. Alles, was über `dvoix` registriert wurde, entfernt die App beim Ausschalten selbst. Eigene Timer oder Listener gibt man mit `dvoix.register(() => ...)` ab.

`main.js` ist ein einzelnes ES-Modul ohne Importe. Wer Bibliotheken oder TypeScript nutzt, bündelt vorher (z. B. mit esbuild oder Vite im Library-Modus, Format `es`).

## Die API (`dvoix`)

Oberflächen sind `mount(el, props)`-Funktionen: Sie zeichnen in ein leeres Element und geben optional eine Aufräumfunktion oder `{ update(props), destroy() }` zurück. Icons sind SVG-Text (z. B. von lucide.dev kopiert). Alle IDs gelten nur im eigenen Plugin.

| Aufruf | wofür |
| --- | --- |
| `addPanel({ id, label, icon, mount })` | Panel in der Seitenleiste mit Icon in der Icon-Leiste |
| `addRibbonAction({ id, label, icon, action })` | Knopf in der Icon-Leiste |
| `addCommand({ id, name, hotkeys, run })` | Befehl mit optionalem Tastenkürzel |
| `addFileFooter({ id, mount })` | etwas unter der offenen Datei, `props.path` ist die Datei |
| `addPaneHeader({ id, mount })` | etwas über der offenen Datei |
| `addOverlay({ id, mount })` | Element auf oberster Ebene (Dialoge) |
| `registerView({ id, label, icon, mount })` | eigene Ansicht in einem Tab, öffnen mit `dvoix.openView(id)` |
| `registerFileType({ id, label, extensions, icon, template, mount })` | neuer Dateityp; `props` hat `path`, `content`, `onchange(text)`, `onopen(path)`, `update(props)` kommt bei Änderungen auf der Platte |
| `registerEditorExtension(ext)` | CodeMirror-Erweiterung, gebaut mit `dvoix.lib.codemirror.state` / `.view` (gleiche Instanz wie der Editor) |
| `addSettingTab({ mount })` | eigene Seite in den Einstellungen |
| `loadData(defaults)` / `saveData(data)` | Einstellungen in `.dvoix/plugins/<id>/data.json` |
| `vault.list()`, `vault.read(path)`, `vault.write(path, text)`, `vault.openFile(path)` | Dateien, Pfade relativ zum Vault |
| `register(fn)` | `fn` läuft beim Ausschalten |

Farben: Die CSS-Variablen von Tailwind (z. B. `var(--color-zinc-300)`, `var(--color-violet-500)`) passen zum Rest der App. Klassen in `styles.css` mit einem eigenen Präfix versehen, die Datei gilt für die ganze App.

## Lokal testen

Ohne Store: Die drei Dateien in `<vault>/.dvoix/plugins/<id>/` legen, den Vault neu öffnen und in **Plugin-Store → Installiert** einschalten. Von Hand kopierte Plugins zählen als Community-Plugin, dafür also „Community-Plugins erlauben“ einschalten.

## In den Store bringen

Die Liste der Plugins steht in [`DamavoiZ/dvoix-plugins`](https://github.com/DamavoiZ/dvoix-plugins) in `registry.json` (Beispiel: [`registry.example.json`](registry.example.json)):

```json
{
  "version": 1,
  "plugins": [
    {
      "id": "hello",
      "name": "Hallo Welt",
      "author": "damavoi",
      "description": "Ein Satz, was das Plugin macht.",
      "repo": "DamavoiZ/dvoix-plugin-hello",
      "channel": "official"
    }
  ]
}
```

Ein Community-Plugin kommt per Pull Request mit `"channel": "community"` dazu. `"official"` gilt nur für Repositories von `DamavoiZ`; bei allen anderen zeigt die App das Plugin als Community-Plugin, auch wenn in der Liste etwas anderes steht.
