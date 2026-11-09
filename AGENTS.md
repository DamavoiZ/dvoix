# AGENTS.md

Feste Regeln für alle KI-Agents (Claude, Codex, Copilot, …), die in diesem Repo arbeiten. Diese Datei ist die Hauptquelle; `CLAUDE.md` verweist nur hierher. Ausführliche Regeln zu einzelnen Themen stehen in [`.agents/`](.agents/). Lies vor einer Änderung die Datei zum betroffenen Thema.

## Was dvoix ist

Eine Desktop-App für Notizen, Projekte, Aufgaben und Tracking, ähnlich wie Obsidian. Alle Daten liegen als normale Dateien in einem Ordner auf der Platte (dem **Vault**). Jede Funktion ist ein **Core-Plugin**. Die Einstellungen eines Vaults liegen im Vault unter `.dvoix/`.

## Die Repos von dvoix

dvoix verteilt sich auf mehrere Repositories. Dieser Abschnitt steht gleich in der `AGENTS.md` jedes Repos. Wer ihn ändert, ändert ihn in allen.

| Repo | Wofür | Was dort hingehört |
| --- | --- | --- |
| [`DamavoiZ/dvoix`](https://github.com/DamavoiZ/dvoix) | die App | Rust-Kern, Tauri-Hülle, Svelte-Frontend, Core-Plugins, Plugin-API, Plugin-Store in der App, Entwickler-Anleitung für Store-Plugins (`docs/plugins/`), README für Entwickler |
| [`DamavoiZ/dvoix-website`](https://github.com/DamavoiZ/dvoix-website) | Produktseite und Doku | Produktseite unter `/`, Doku für Nutzer und Plugin-Entwickler unter `/docs`, Screenshots für die Seite, Docker-Image für den Server |
| [`DamavoiZ/dvoix-plugins`](https://github.com/DamavoiZ/dvoix-plugins) | Liste des Plugin-Stores | nur `registry.json` und ihre Prüfung (`scripts/validate.mjs`). Kein Plugin-Code |
| `DamavoiZ/dvoix-<name>` | je ein offizielles Store-Plugin | Code, `manifest.json`, Releases mit `manifest.json`, `main.js`, `styles.css` |

### Wo gehört was hin?

| Aufgabe | Repo |
| --- | --- |
| Grundfunktion, die fast jeder braucht (Explorer, Suche, Aufgaben, …) | `dvoix`, als Core-Plugin unter `src/lib/plugins/core/` |
| Zusatzfunktion, die nicht jeder will (Kanban, Tracking, …) | eigenes Repo `DamavoiZ/dvoix-<name>` als Store-Plugin, dazu ein Eintrag in `dvoix-plugins` |
| Plugin-API erweitern (`PluginContext`, Berechtigungen) | `dvoix`, dazu `docs/plugins/README.md` dort und die Seite `plugin-api.md` der Website |
| Erklären, wie man dvoix bedient | `dvoix-website`, Seite in `src/content/docs/` |
| Plugin im Store anbieten oder entfernen | `dvoix-plugins`, `registry.json` |
| Fehler in einem Store-Plugin | im Repo des Plugins, nicht in der App |

### Was über Repo-Grenzen zusammenhängt

- **Format der Plugin-Liste:** festgelegt in `crates/dvoix-core/src/store.rs` (`parse_registry`) der App, Beispiel in `docs/plugins/registry.example.json`, geprüft von `scripts/validate.mjs` in `dvoix-plugins`. Ändert sich das Format, ändern sich alle drei Stellen.
- **Adresse der Plugin-Liste:** Die App liest `https://raw.githubusercontent.com/DamavoiZ/dvoix-plugins/main/registry.json`. Repo-Name, Branch `main` und Dateiname nie ändern, sonst findet keine installierte App mehr Plugins.
- **Offiziell:** `"channel": "official"` gilt nur für Repos von `DamavoiZ`. Die App und die Prüfung in `dvoix-plugins` setzen das beide durch.
- **Plugin-API:** `src/lib/plugins/store/api.ts` in der App, `docs/plugins/README.md` in der App und `src/content/docs/plugin-api.md` auf der Website beschreiben dieselbe API und müssen zusammenpassen.
- **Funktionen der App:** Was die Website-Doku über die App sagt, muss zum Code in `dvoix` passen. Ändert sich die App für Nutzer sichtbar, wird die Doku nachgezogen.

### Änderungen über mehrere Repos

- Ein PR pro Repo. Die PRs verlinken sich gegenseitig in der Beschreibung.
- Reihenfolge beim Mergen: zuerst die App (`dvoix`), dann `dvoix-plugins` und `dvoix-website`. Eine Doku oder Plugin-Liste darf nichts versprechen, was die App auf `main` noch nicht kann.
- Für alle Repos gilt gleich: Code, Kommentare und Commit-Messages auf Englisch, alles für Nutzer auf Deutsch; immer die neuesten stabilen Versionen von Node.js, Abhängigkeiten und GitHub Actions; nie direkt auf `main` pushen.

## Die sechs Grundregeln

1. **Die Dateien sind die einzige Quelle der Wahrheit.** Alles andere (`.dvoix/index.sqlite`, Zustand im Speicher) ist Cache und muss sich jederzeit aus den Dateien neu aufbauen lassen. Nie Daten nur im Index, im `localStorage` oder in der App halten.
2. **Jedes Feature ist ein Plugin.** Neue Funktionen kommen als Ordner unter `src/lib/plugins/core/<id>/` und hängen sich nur über den `PluginContext` ein. Keine Sonderfälle für ein Feature in `App.svelte` oder den Komponenten unter `src/lib/components/`.
3. **Vault-Einstellungen gehören nach `<vault>/.dvoix/`.** Nur „welcher Vault war zuletzt offen“ liegt im Konfigurationsordner des Betriebssystems (`src-tauri/src/settings.rs`).
4. **Schreiben ist atomar und minimal.** Dateien werden über `Vault::write` geschrieben (Temp-Datei, dann umbenennen). Ändert eine Funktion eine Notiz (z. B. Aufgabe abhaken), ändert sie nur die betroffene Zeile und formatiert nie die ganze Datei neu.
5. **Pfade sind vault-relativ und laufen durch `Vault::resolve`.** Kein Zugriff außerhalb des Vaults, keine absoluten Pfade vom Frontend.
6. **Immer die neuesten Versionen.** Node.js (aktuelle Version, nicht eine ältere LTS), Rust stable, alle npm- und Cargo-Abhängigkeiten und alle GitHub Actions laufen auf der jeweils neuesten Version. Neue Abhängigkeiten in der neuesten Version einbauen, beim Anfassen alter Stände mit anheben und die CI, das README und `package.json` (`engines`) passend halten. Ältere Versionen nur mit Begründung im PR, z. B. wenn die neueste nachweislich kaputt ist.

## Stack

| Schicht | Technik |
| --- | --- |
| App-Hülle | Tauri 2 (`src-tauri/`), `ureq` für die Downloads des Plugin-Stores, `webkit2gtk` (nur Linux) für Einstellungen der Webview, die Tauri nicht anbietet |
| Kern | Rust-Crate `crates/dvoix-core/` ohne Tauri: `notify`, `pulldown-cmark`, `serde_yaml_ng`, `rusqlite` mit FTS5, `walkdir` |
| Frontend | Svelte 5 (Runes), TypeScript, Vite, Tailwind CSS 4, Icons aus `@lucide/svelte`, `force-graph` für die Graph-Ansicht |
| Editor | CodeMirror 6 |

## Aufbau

```
crates/dvoix-core/src/   Logik ohne UI: vault.rs, markdown.rs, tasks.rs, project.rs, git.rs, store.rs, index.rs, watcher.rs
src-tauri/src/           Tauri-Hülle: lib.rs (Setup, Handler-Liste), commands.rs (alle Befehle), http.rs (Downloads für den Store), settings.rs
src/lib/api.ts           typisierte Wrapper für jeden Rust-Befehl
src/lib/config.ts        JSON in <vault>/.dvoix/ lesen/schreiben (verzögert, gebündelt)
src/lib/*.svelte.ts      App-Zustand (vault, ui, settings, window) mit Runes
src/lib/filetypes/       Dateityp-Registry, Markdown-Editor, Fallback-Ansicht
src/lib/plugins/         Plugin-API (types.ts), Verwaltung (manager.svelte.ts), Einstellungen (pluginSettings.svelte.ts)
src/lib/plugins/core/    ein Ordner pro Core-Plugin, Liste in core/index.ts
src/lib/plugins/store/   Plugin-Store: Store-Plugins laden/installieren, ihre API (api.ts)
docs/plugins/            Anleitung und Beispiel für Store-Plugins, Beispiel der Plugin-Liste
src/lib/components/      App-Rahmen: Ribbon, SidePanel, FilePane, Einstellungen, Kontextmenü
```

## Befehle

```sh
npm install              # einmalig
npm run check            # svelte-check: Svelte + TypeScript
npm run build            # Frontend bauen (Vite)
cargo test --workspace   # Rust-Tests (Kern + Tauri-Crate)
npm run tauri dev        # App starten (braucht Tauri-Systemabhängigkeiten)
```

Vor jedem Commit müssen `npm run check`, `npm run build` und `cargo test --workspace` grün sein. Genau diese drei laufen in der CI (`.github/workflows/ci.yml`). Unter Linux braucht der Rust-Build `libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev libxdo-dev libssl-dev`. Wenn ein Check lokal nicht laufen kann, sag das ausdrücklich, statt Erfolg zu behaupten.

## Konventionen

- **Sprache:** Code, Bezeichner, Kommentare und Commit-Messages auf Englisch. Alles, was Nutzer sehen (UI-Texte, Plugin-Namen und -Beschreibungen, Fehlermeldungen aus Befehlen, README), auf Deutsch.
- **Kommentare** erklären das Warum, nicht das Was. Jede Datei beginnt mit einem kurzen Kommentar, wofür sie da ist (siehe bestehende Dateien).
- **Svelte 5 mit Runes** (`$state`, `$derived`, `$effect`, `$props`). Kein `export let`, keine Svelte-4-Stores.
- **Styling** mit Tailwind-Klassen und den CSS-Variablen aus `src/app.css`. Keine neuen Farben hart kodieren.
- **Keine neuen Abhängigkeiten** ohne guten Grund. Wenn doch, im PR begründen.
- **Kein Umformatieren** unbeteiligter Dateien.
- **Rust:** Fehler über `dvoix_core::Error` und `Result`, kein `unwrap()` außerhalb von Tests. Neue Logik im Kern bekommt Tests im selben Modul (`#[cfg(test)] mod tests`, Vault mit `tempfile::tempdir()`).
- **README aktuell halten:** Neue Funktion, neuer Dateityp, neue Datei in `.dvoix/` oder neue Plugin-API → Abschnitt im README ergänzen.

## Themen-Regeln in `.agents/`

| Datei | Wann lesen |
| --- | --- |
| [`.agents/plugins.md`](.agents/plugins.md) | neues Plugin, Plugin ändern, Plugin-Einstellungen |
| [`.agents/filetypes.md`](.agents/filetypes.md) | neuer Dateityp (z. B. `.board`, `.track`) |
| [`.agents/rust-core.md`](.agents/rust-core.md) | Änderungen im Rust-Kern, neue Tauri-Befehle, Index |
| [`.agents/frontend.md`](.agents/frontend.md) | Svelte-Komponenten, App-Zustand, Editor |
| [`.agents/vault-data.md`](.agents/vault-data.md) | Dateiformate, Frontmatter, `.dvoix/`, Aufgaben-Syntax |
| [`.agents/workflow.md`](.agents/workflow.md) | Branches, Commits, PRs, was vor dem Abschluss geprüft wird |
