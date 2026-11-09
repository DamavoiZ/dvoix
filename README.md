# dvoix

Eine Desktop-App für Notizen, Projektmanagement, Tracking und alles, was später dazukommt. Wie bei Obsidian liegen alle Daten als ganz normale Dateien in einem Ordner auf deinem PC (dem **Vault**). Die Dateien sind die einzige Quelle der Wahrheit; alles andere ist ein Cache, der jederzeit neu aufgebaut werden kann.

![dvoix](docs/screenshot.png)

## Was schon geht

- Vault-Ordner wählen, der zuletzt geöffnete wird beim Start wieder geladen
- Seitenleiste wie in Obsidian: schmale Icon-Leiste ganz links (Dateien, Suche, Tags, Aufgaben, Projekte), daneben das Panel, in der Breite ziehbar und per Klick aufs aktive Icon einklappbar
- Eigene Titelleiste unter Windows: kein heller Systemrahmen, die Tab-Leiste und die obere Zeile der Seitenleiste verschieben das Fenster (Doppelklick maximiert), Minimieren, Maximieren und Schließen sitzen rechts neben den Tabs. Schmale, dunkle Scrollbalken überall in der App
- Datei-Explorer sortiert natürlich: `2 - Notes` kommt vor `10 - Archiv`
- Datei-Explorer mit Ordnerbaum: Ordner auf- und zuklappen (wird gemerkt), Rechtsklick-Menü zum Anlegen, Umbenennen und Löschen (landet in `.trash/` im Vault), Dateien per Drag & Drop in Ordner verschieben
- Tabs: Klick im Datei-Explorer öffnet im aktiven Tab, Strg/Cmd + Klick oder mittlere Maustaste in einem neuen; Tabs per Drag & Drop verschieben, mit mittlerer Maustaste oder × schließen. Offene Tabs werden pro Vault gemerkt
- Schnellöffner (Strg/Cmd + O) mit unscharfer Suche über alle Dateien: `mtg plan` findet `Projekte/Meeting-Planung`. Enter öffnet, Strg + Enter in neuem Tab, gibt es die Notiz nicht, wird sie angelegt
- Tastenkürzel: Strg + T neuer Tab, Strg + W Tab schließen, Strg + Tab / Strg + Umschalt + Tab (oder Strg + Bild ab/auf) Tab wechseln, Strg + Enter Änderungen committen, Strg + Umschalt + H Verlauf öffnen
- Markdown-Editor (CodeMirror 6) mit YAML-Frontmatter, `[[Wikilinks]]` (Strg/Cmd + Klick öffnet oder legt die Notiz an) und `#tags`
- Drei Ansichten wie in Obsidian: **Lesen** (die Notiz als gerenderte Seite mit den Eigenschaften aus dem Frontmatter), **Bearbeiten** (Live-Vorschau: die Markdown-Syntax ist nur auf der Zeile mit dem Cursor sichtbar, `## ` macht sofort eine Überschrift) und **Quelltext** (das rohe Markdown). Umschalten über die Knöpfe oben rechts in der Notiz, mit Strg + E zwischen Lesen und Bearbeiten und Strg + Umschalt + E zwischen Quelltext und Bearbeiten. In welcher Ansicht eine Notiz geöffnet wird, steht in den Einstellungen.
- Automatisches Speichern, atomar geschrieben (erst Temp-Datei, dann umbenennen)
- Änderungen von außen (VS Code, Sync-Tools, git) erscheinen sofort in der App
- Volltextsuche, Tag-Übersicht und Backlinks („Verlinkt von“)
- Aufgaben: ganz normale Markdown-Checkboxen in jeder Notiz, optional mit Fälligkeit und Priorität. Im Editor sind sie anklickbar, die Aufgabenübersicht in der Seitenleiste sammelt alle Aufgaben des Vaults (nach Fälligkeit oder nach Notiz gruppiert)
- Projekte: Notizen mit `type: projekt` im Frontmatter, mit Status, Fälligkeit und Fortschritt aus ihren Checkboxen in einer eigenen Projektliste
- Graph-Ansicht (Strg/Cmd + G oder das Netz-Symbol in der Icon-Leiste) in einem eigenen Tab: Notizen als Punkte, verbunden über Links, über ihre Ordner (Ordner als eigene Knoten) und/oder über gemeinsame Tags (Tags als Knoten oder direkte Linien). Einstellbar sind Filter (`projekte #idee -archiv`), einzelne Notizen, Anhänge und noch fehlende Notizen ein/aus, Farben nach Ordner oder Tag, Größe nach Anzahl der Verbindungen, Pfeile, Linienstärke, ab welcher Zoomstufe Titel erscheinen und die Kräfte des Layouts. Klick öffnet die Notiz in einem neuen Tab, Hover hebt die Nachbarn hervor
- Alles ist ein Plugin, wie bei Obsidian: Datei-Explorer, Suche, Tags, Backlinks, Aufgaben, Projekte, Canvas, Graph, Schnellöffner, Tabs, Ansichten und versteckte Dateien (Ordner wie `.obsidian`, anfangs aus) lassen sich in den Einstellungen (Zahnrad unten in der Icon-Leiste oder Strg/Cmd + ,) einzeln ausschalten
- Plugin-Store in den Einstellungen: zusätzliche Plugins suchen, installieren, aktualisieren und wieder entfernen. Offizielle Plugins gehen sofort, Community-Plugins erst nach einem eigenen Schalter mit Warnung (siehe „Plugin-Store“ unten)
- Einstellungen: Editor (Zeilenlänge, Zeilennummern, Rechtschreibprüfung), Darstellung (Schriftgrößen) und eigene Optionen je Plugin
- Alle Einstellungen eines Vaults liegen im Vault selbst unter `.dvoix/` (siehe unten), wandern also mit, wenn du den Ordner kopierst oder synchronisierst
- Git: der Vault wird über das `git`-Programm des Systems versioniert. Ein Panel in der Seitenleiste zeigt, was sich geändert hat (gestaged, geändert, unversioniert), man kann einzelne Dateien stagen, Änderungen verwerfen, committen (Strg/Cmd + Enter), Branches wechseln und anlegen, pushen und ziehen. Ein eigener Tab zeigt den Verlauf mit den Dateien je Commit und dem Diff dazu
- Erster eigener Dateityp: `.canvas` im offenen [JSON-Canvas-Format](https://jsoncanvas.org) (Karten verschieben, Doppelklick legt eine Karte an)

## Tech-Stack

| Schicht | Technik |
| --- | --- |
| App-Hülle | [Tauri 2](https://v2.tauri.app), `ureq` (Downloads für den Plugin-Store) |
| Kern (Rust) | `notify` (Dateiwächter), `pulldown-cmark` + `serde_yaml_ng` (Markdown & Frontmatter), `rusqlite` mit FTS5 (Index & Suche), `git` (das Programm des Systems, für das Git-Plugin) |
| Frontend | Svelte 5, TypeScript, Vite, Tailwind CSS 4, [force-graph](https://github.com/vasturiano/force-graph) (Graph-Ansicht auf Canvas) |
| Editor | CodeMirror 6 |

## Aufbau

```
crates/dvoix-core/      Rust-Kern ohne Tauri, eigenständig testbar
  src/vault.rs          Lesen/Schreiben im Vault, Pfade immer relativ und abgesichert
  src/markdown.rs       Frontmatter, Titel, Tags, Links, Überschriften, Suchtext, HTML für die Leseansicht
  src/tasks.rs          Checkbox-Aufgaben lesen und abhaken
  src/project.rs        Frontmatter-Schema für Projekte
  src/git.rs            Git-Befehle für den Vault (über das git-Programm des Systems)
  src/store.rs          Plugin-Store: Plugin-Liste, manifest.json, Installieren in .dvoix/plugins/
  src/index.rs          SQLite-Index in <vault>/.dvoix/index.sqlite (nur Cache)
  src/watcher.rs        meldet Änderungen auf der Platte
src-tauri/              Tauri-App: hält den offenen Vault, verbindet Watcher, Index und UI
  src/commands.rs       alle Befehle, die das Frontend aufrufen kann
  src/http.rs           Downloads für den Plugin-Store (nur https, nur GitHub)
src/                    Svelte-Frontend
  lib/api.ts            typisierte Aufrufe der Rust-Befehle
  lib/vault.svelte.ts   App-Zustand: offener Vault, offene Datei, Speichern
  lib/config.ts         JSON-Dateien in <vault>/.dvoix/ lesen und schreiben
  lib/settings.svelte.ts  Editor- und Darstellungs-Einstellungen
  lib/ui.svelte.ts      Workspace: aktives Panel, Breite, aufgeklappte Ordner, offene Tabs
  lib/window.svelte.ts  eigene Titelleiste: Fenster ziehen, Fenster-Knöpfe
  lib/filetypes/        Dateityp-Registry und der eingebaute Markdown-Editor
  lib/plugins/          Plugin-API (types.ts) und Plugin-Verwaltung (manager.svelte.ts)
  lib/plugins/core/     die Core-Plugins, je eins pro Ordner
  lib/plugins/store/    Plugin-Store: Plugins laden, installieren, ihre API (api.ts)
docs/plugins/           Anleitung für Store-Plugins, Beispiel-Plugin, Beispiel der Plugin-Liste
  lib/components/       App-Rahmen: Ribbon, Seitenpanel, Dateiansicht, Einstellungen
```

## Der Ordner `.dvoix/`

Wie `.obsidian/` bei Obsidian. Alles, was zum Vault gehört, liegt hier; nur „welcher Vault war zuletzt offen“ steht im Konfigurationsordner des Betriebssystems.

```
.dvoix/
  app.json                  Editor-Einstellungen (Zeilenlänge, Standardansicht)
  appearance.json           Schriftgrößen
  core-plugins.json         welche Plugins an sind: { "tasks": true, "canvas": false, ... }
  workspace.json            offene Tabs, aktives Panel, Breite, aufgeklappte Ordner
  plugins/<id>/data.json    Einstellungen eines Plugins (z. B. plugins/tasks/data.json)
  plugins/<id>/manifest.json, main.js, styles.css   ein Plugin aus dem Store
  store.json                Store-Plugins: an/aus, woher sie kommen, Community-Plugins erlaubt ja/nein
  index.sqlite              Suchindex, nur Cache
```

Fehlt eine Datei, gelten die Standardwerte. `index.sqlite` kannst du jederzeit löschen, er wird neu aufgebaut. Ordner, die mit `.` beginnen, werden nicht indexiert und standardmäßig nicht angezeigt (siehe „Versteckte Dateien“ unten). Das Layout, das frühere Versionen im Browser-Speicher der App hatten, wird beim ersten Öffnen eines Vaults automatisch nach `workspace.json` übernommen.

## Plugins

Jede Funktion ist ein Plugin mit Manifest, das sich über eine kleine API in die App einhängt. Alles, was es dort registriert, wird beim Ausschalten automatisch wieder entfernt. Ein Plugin kann:

| Aufruf | wofür |
| --- | --- |
| `ctx.addPanel(...)` | Panel in der Seitenleiste mit Icon in der Icon-Leiste |
| `ctx.addRibbonAction(...)` | Knopf in der Icon-Leiste, der etwas ausführt |
| `ctx.addCommand(...)` | Befehl mit optionalem Tastenkürzel |
| `ctx.registerFileType(...)` | neuer Dateityp mit eigener Ansicht |
| `ctx.registerEditorExtension(...)` | CodeMirror-Erweiterung für den Markdown-Editor |
| `ctx.addFileFooter(...)` / `ctx.addPaneHeader(...)` | etwas unter oder über der offenen Datei (Backlinks, Tab-Leiste) |
| `ctx.addOverlay(...)` | Dialog auf oberster Ebene (Schnellöffner) |
| `ctx.addExplorerEntries(...)` | zusätzliche Einträge nur im Datei-Explorer (versteckte Ordner) |
| `ctx.registerView(...)` | eigene Ansicht, die wie eine Datei in einem Tab läuft (Graph); öffnen mit `vault.openViewTab(id)` |
| `ctx.addSettingTab(...)` | eigene Seite in den Einstellungen |
| `ctx.loadData()` / `ctx.saveData()` | eigene Daten in `.dvoix/plugins/<id>/data.json` |

### Versteckte Dateien

Das Plugin „Versteckte Dateien“ ist anfangs aus. Eingeschaltet zeigt es Ordner und Dateien, deren Name mit einem Punkt beginnt (z. B. `.obsidian`, `.trash`, `.gitignore`), etwas blasser im Datei-Explorer. Dort lassen sie sich öffnen, bearbeiten, umbenennen und löschen wie alles andere. In Suche, Tags, Backlinks, Graph und Schnellöffner tauchen sie nie auf, und auch Notizen darin werden nicht indexiert.

Einstellungen (in `.dvoix/plugins/hidden-files/data.json`): anzeigen ja/nein (auch über den Befehl „Versteckte Dateien ein-/ausblenden“), ob auch einzelne Punkt-Dateien erscheinen, ob der eigene Ordner `.dvoix` erscheint (aus) und welche Namen trotzdem verborgen bleiben (Standard: `.git`). Pro Vault werden höchstens 10.000 versteckte Einträge aufgelistet. Änderungen in `.git` und `.dvoix` meldet der Dateiwächter nicht, damit ein Commit die App nicht mit Ereignissen flutet; ihr Inhalt im Explorer wird beim nächsten Umschalten oder beim nächsten Anlegen bzw. Löschen einer Datei neu geladen.

### Plugin-Store

Neben den eingebauten Core-Plugins gibt es Plugins, die man nur bei Bedarf installiert: **Einstellungen → Plugin-Store**. Unter „Entdecken“ steht die Plugin-Liste mit Suche, ein Klick zeigt Details, Version und Berechtigungen und installiert das Plugin. Unter „Installiert“ lassen sie sich ein- und ausschalten, aktualisieren („Nach Updates suchen“) und deinstallieren. Beim Deinstallieren bleiben die Einstellungen des Plugins (`data.json`) liegen.

- **Offizielle Plugins** pflegt dvoix selbst. Sie lassen sich immer installieren.
- **Community-Plugins** schreiben andere. Sie gehen erst, wenn „Community-Plugins erlauben“ an ist, und dafür kommt vorher eine Warnung. Schaltet man den Schalter wieder aus, werden sie sofort entladen.

Die Plugin-Liste steht als `registry.json` im Repository [`DamavoiZ/dvoix-plugins`](https://github.com/DamavoiZ/dvoix-plugins), die Plugins selbst kommen aus den GitHub-Releases ihrer eigenen Repositories. Heruntergeladen wird nur über https von GitHub. „Offiziell“ zeigt die App nur für Repositories von `DamavoiZ` an, egal was in der Liste steht.

Jedes Plugin gibt in seinem Manifest an, was es braucht (`vault:read`, `vault:write`, `editor`). Das steht im Store vor dem Installieren, und die API verweigert alles, was nicht angegeben ist. Eine Sandbox ist das nicht: Wie bei Obsidian laufen Plugins in der App und könnten die API umgehen. Deshalb die Trennung in offiziell und Community.

Wie man ein Plugin für den Store schreibt, steht in [`docs/plugins/README.md`](docs/plugins/README.md), mit Beispiel-Plugin in `docs/plugins/hello/`.

## Ein neues Feature hinzufügen

Ein neues Feature ist ein neues Plugin. Beispiel Kanban-Board als `.board`:

1. Ordner `src/lib/plugins/core/board/` anlegen mit einer `BoardView.svelte`. Sie bekommt `content` (Dateiinhalt als Text) und ruft `onchange(neuerInhalt)` auf, wenn sich etwas ändert. Speichern übernimmt die App.
2. Daneben `index.ts`:
   ```ts
   export const boardPlugin: Plugin = {
     manifest: { id: "board", name: "Kanban", description: "Boards als .board-Dateien.", version: "1.0.0" },
     icon: Kanban, // beliebiges Icon aus @lucide/svelte
     onload(ctx) {
       ctx.registerFileType({
         id: "board",
         label: "Board",
         extensions: ["board"],
         icon: Kanban,
         view: BoardView,
         template: () => JSON.stringify({ columns: [] }, null, 2),
       });
     },
   };
   ```
3. In `src/lib/plugins/core/index.ts` in die Liste `corePlugins` eintragen.

Damit erscheint das Plugin in den Einstellungen zum Ein- und Ausschalten, „Board erstellen“ in der Werkzeugleiste und im Rechtsklick-Menü des Explorers und `.board`-Dateien öffnen sich in der neuen Ansicht. Soll der Typ auch durchsuchbar sein, bekommt `crates/dvoix-core/src/index.rs` eine Extraktion für die Endung (siehe `is_indexable`).

Für Projekte und Tracking ist Frontmatter der Baustein: `status: active`, `due: 2026-12-01` usw. landen im Index und lassen sich mit `notes_where("status", "active")` abfragen.

## Aufgaben und Projekte

Eine Aufgabe ist eine Checkbox in irgendeiner Notiz. Fälligkeit und Priorität stehen direkt in der Zeile, die Emoji-Schreibweise ist dieselbe wie beim Obsidian-Tasks-Plugin:

```markdown
- [ ] Angebot schreiben 📅 2026-12-01 ⏫ #kunde
- [ ] Anrufen due:2026-11-20 !hoch
  - [x] Unteraufgabe
```

| | Schreibweise |
| --- | --- |
| Fälligkeit | `📅 2026-12-01` oder `due:2026-12-01` |
| Priorität | `!hoch` `!mittel` `!niedrig` (auch `!high` usw.) oder ⏫ 🔼 🔽 |

Ein Projekt ist eine Notiz mit diesem Frontmatter, seine Aufgaben sind die Checkboxen darin:

```yaml
---
type: projekt          # oder: project
status: aktiv          # idee | aktiv | wartend | erledigt | abgebrochen
due: 2026-12-01        # optional
start: 2026-11-01      # optional
priority: hoch         # optional: hoch | mittel | niedrig
area: Arbeit           # optional, frei wählbar
---
```

Englische Werte (`active`, `waiting`, `done`, ...) gehen auch. Abhaken in der Aufgabenübersicht ändert genau die eine Zeile in der Datei.

## Git

Das Git-Plugin versioniert den Vault über das `git`-Programm, das auf dem Rechner installiert sein muss (z. B. mit Git für Windows, macOS oder Linux). Im Kern steckt dabei kein zusätzlicher Rust-Baustein: `crates/dvoix-core/src/git.rs` ruft `git` auf und übersetzt die Ausgabe. Es ist ein Core-Plugin wie die anderen und lässt sich in den Einstellungen abschalten.

Im Panel in der Seitenleiste steht, was sich geändert hat:

| Gruppe | Bedeutung |
| --- | --- |
| Gestaged | Änderungen, die beim nächsten Commit mitgehen |
| Geändert | Änderungen an der Datei, die noch nicht gestaged sind |
| Unversioniert | Dateien, die git noch nicht kennt |

Dateien lassen sich einzeln stagen, aus dem Index nehmen, verwerfen (unversionierte Dateien werden dabei gelöscht) und im Diff ansehen. Der Commit nimmt die gestageden Änderungen; sind keine gestaged, kommen alle Änderungen hinein (in den Einstellungen abschaltbar). Branches lassen sich im Panel wechseln und anlegen, dazu kommen Push und Pull.

Der Verlauf (Strg/Cmd + Umschalt + H) zeigt die letzten Commits; aufgeklappt zeigt jeder Commit die geänderten Dateien, ein Klick darauf den Diff.

Der Diff zeigt die alte Fassung links und die neue rechts, beide scrollen gemeinsam. Geänderte Zeilen sind farbig hinterlegt (rot entfernt, grün neu), die geänderten Zeichen darin kräftiger. Lange unveränderte Abschnitte sind zugeklappt und lassen sich per Klick öffnen. Oben rechts lässt sich auf „Untereinander“ umschalten: dann steht alles in einer Spalte, entfernte Zeilen über den neuen. Die Wahl merkt sich das Plugin in `.dvoix/plugins/git/data.json`.

Dateien, die dvoix selbst laufend neu schreibt, bleiben aus Git heraus: der Suchindex (`.dvoix/index.sqlite`), die offenen Tabs (`.dvoix/workspace.json`) und die Temp-Dateien beim Speichern (`*.dvoix-tmp`). Sie stehen nicht in der Liste der Änderungen und gehen bei „alles stagen“ nicht mit. Die übrigen Einstellungen in `.dvoix/` werden ganz normal versioniert. Notizen mit Windows-Zeilenenden (CRLF) behalten diese beim Speichern, damit sich nicht jede Zeile ändert.

Liegt der Vault in einem Ordner eines größeren Repository, bleiben Dateien außerhalb des Vaults unsichtbar: gestaged und committet wird nur, was im Vault liegt. Wichtig ist auch, dass `user.name` und `user.email` gesetzt sind, sonst meldet git beim Commit, dass es die Absenderdaten nicht kennt.

## Entwickeln

Voraussetzungen: [Rust](https://rustup.rs) (aktuelles stable, wird über `rust-toolchain.toml` automatisch gewählt), Node.js 26 (siehe `.nvmrc`) und die [Tauri-Systemabhängigkeiten](https://v2.tauri.app/start/prerequisites/) für dein Betriebssystem.

```sh
npm install
npm run tauri dev      # App mit Hot-Reload starten
npm run tauri build    # Installer bauen
```

Nach einem Pull, der neue Abhängigkeiten bringt, einmal `npm install` ausführen. `npm run dev`, `npm run check` und `npm run build` prüfen vorher selbst, ob alle Pakete aus `package.json` installiert sind, und sagen sonst, was fehlt – statt später in Vite mit „Failed to resolve import …“ aufzutauchen.

Prüfen:

```sh
npm run check          # Svelte/TypeScript
cargo test --workspace # Rust-Kern
```

## Nächste Schritte (Ideen)

- Geteilte Ansichten (zwei Tabs nebeneinander)
- Kanban-Board (`.board`) und Tracking (`.track`) als offizielle Plugins im Store
- `tantivy` statt FTS5, falls die Suche mehr können muss

## Lizenz

dvoix steht wahlweise unter der [MIT-Lizenz](LICENSE-MIT) oder der [Apache-Lizenz 2.0](LICENSE-APACHE), nach Wahl der Nutzerin oder des Nutzers.

Beiträge, die du zu diesem Projekt einreichst, stehen ohne weitere Bedingungen unter denselben beiden Lizenzen, sofern du nicht ausdrücklich etwas anderes angibst.
