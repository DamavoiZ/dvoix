# Rust-Kern und Tauri-Befehle

## Aufteilung

- `crates/dvoix-core/` enthält die Logik und kennt Tauri nicht. Alles, was sich ohne Fenster testen lässt, gehört hierhin.
- `src-tauri/` ist nur die Hülle: hält den offenen Vault (`AppState`/`Workspace` in `lib.rs`), verbindet Watcher und Index und stellt Befehle bereit (`commands.rs`). Dort keine Fachlogik.

## Regeln im Kern

- Jeder Dateizugriff geht über `Vault` (`vault.rs`). Pfade kommen vault-relativ mit `/` rein und werden mit `Vault::resolve` geprüft. Nie selbst Pfade zusammenbauen.
- Git sitzt in `git.rs` und ruft das `git`-Programm des Systems auf (kein zusätzlicher Rust-Baustein). Das Arbeitsverzeichnis ist immer `Vault::root()`, nie ein Pfad aus dem Frontend.
- Schreiben nur über `Vault::write` (atomar) bzw. `create`, `rename`, `trash`. Löschen heißt in `.trash/` verschieben, nie endgültig löschen.
- `.dvoix/`-Dateien über `read_config`/`write_config`/`config_path`, nicht über `write`.
- Ordner und Dateien, die mit `.` beginnen, werden nie indexiert und stehen nicht in `Vault::list` (`Vault::is_ignored`, `index::is_indexable`). Das bleibt so. Anzeigen darf sie nur das Plugin „Versteckte Dateien“ über `Vault::list_hidden`. Der Watcher meldet sie mit `hidden: true` und verwirft `.git`/`.dvoix` ganz.
- Fehler über `crate::Error` und `crate::Result`. Kein `unwrap()`/`expect()` außerhalb von Tests und dem App-Start.
- Änderungen an Dateien sind minimal: `tasks::set_done` ändert genau eine Zeile. Neue Funktionen, die Notizen ändern, arbeiten genauso.
- Neue Logik bekommt Tests im selben Modul (`#[cfg(test)] mod tests`). Für Dateisystem-Tests `tempfile::tempdir()`.

## Index (`index.rs`)

- `index.sqlite` ist nur Cache. Er muss sich jederzeit mit `rebuild` vollständig aus den Dateien erzeugen lassen.
- Ändert sich das Schema, `SCHEMA_VERSION` erhöhen. Die Migration löscht dann die Tabellen und baut neu auf. Keine Daten, die nicht aus Dateien kommen, im Index speichern.
- Frontmatter landet als JSON im Index und ist über `notes_where(key, value)` abfragbar.

## Neuer Tauri-Befehl

1. Funktion in `src-tauri/src/commands.rs` mit `#[tauri::command]`, Rückgabe `CmdResult<T>`, Zugriff auf den Vault über `with_ws`. Nach Schreibzugriffen `reindex(ws, &path)` aufrufen.
2. In `tauri::generate_handler![...]` in `src-tauri/src/lib.rs` eintragen.
3. Typisierten Wrapper in `src/lib/api.ts` ergänzen. Rust-Structs mit `#[serde(rename_all = "camelCase")]`, die TS-Interfaces spiegeln sie genau.
4. Fehlermeldungen an Nutzer auf Deutsch (wie „Kein Vault geöffnet“).
5. Braucht der Befehl neue Rechte, `src-tauri/capabilities/default.json` so eng wie möglich erweitern.

## Prüfen

```sh
cargo test --workspace
cargo clippy --workspace   # sollte ohne neue Warnungen durchlaufen
```
