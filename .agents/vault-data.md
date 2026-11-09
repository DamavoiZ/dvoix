# Vault, Dateiformate und `.dvoix/`

## Grundsatz

Die Dateien im Vault sind die einzige Quelle der Wahrheit. Nutzer bearbeiten sie auch mit anderen Programmen (VS Code, Obsidian, git, Sync-Tools). dvoix muss damit klarkommen und darf ihre Formatierung nicht zerstören.

- Nie eine Datei neu formatieren, die der Nutzer nicht bearbeitet hat. Nur die nötigen Zeichen ändern.
- Unbekanntes Frontmatter, unbekannte JSON-Felder und unbekannte Zeilen bleiben erhalten.
- Kompatibel bleiben mit gängigen Markdown-Konventionen (Obsidian, CommonMark, YAML-Frontmatter), damit ein Vault auch ohne dvoix lesbar ist.
- Externe Änderungen erkennt der Watcher (`watcher.rs`). Die UI muss sie übernehmen.

## `.dvoix/`

```
.dvoix/
  app.json                  Editor-Einstellungen
  appearance.json           Schriftgrößen
  core-plugins.json         welche Plugins an sind
  workspace.json            offene Tabs, aktives Panel, Breite, aufgeklappte Ordner
  plugins/<id>/data.json    Einstellungen eines Plugins
  index.sqlite              Suchindex, nur Cache
```

- Fehlt eine Datei oder ein Schlüssel, gelten Standardwerte. Kaputtes JSON darf die App nicht abstürzen lassen (`readJsonRaw` liefert dann `undefined`).
- Neue Dateien hier im README-Abschnitt „Der Ordner `.dvoix/`“ und in `src/lib/config.ts` (Kopfkommentar) dokumentieren.
- Schlüssel nicht umbenennen. Wenn ein Format sich ändern muss, alte Werte beim Laden übernehmen (Migration), nicht verwerfen.

## Aufgaben und Projekte

- Aufgabe = Markdown-Checkbox `- [ ]` / `- [x]` in einer beliebigen Notiz.
- Fälligkeit: `📅 2026-12-01` oder `due:2026-12-01`. Priorität: `!hoch` `!mittel` `!niedrig` (auch englisch) oder ⏫ 🔼 🔽. Kompatibel zum Obsidian-Tasks-Plugin halten.
- Projekt = Notiz mit `type: projekt` (oder `project`) im Frontmatter, Felder `status`, `due`, `start`, `priority`, `area`. Deutsche und englische Werte werden beide akzeptiert (`project.rs`).
- Neue Felder oder Schreibweisen: Parser im Rust-Kern (`tasks.rs`, `project.rs`) mit Tests, TS-Typen in `api.ts`, README-Tabelle.

## Neue Datenarten (Tracking usw.)

Zuerst prüfen, ob Frontmatter oder eine Markdown-Konvention reicht. Erst wenn nicht, einen eigenen Dateityp anlegen (siehe `filetypes.md`). Nie eine eigene Datenbank als Speicher.
