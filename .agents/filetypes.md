# Dateitypen

Ein Dateityp ist ein `FileType` (`src/lib/filetypes/types.ts`) und wird von einem Plugin mit `ctx.registerFileType(...)` registriert. Beispiel aus dem Code: `src/lib/plugins/core/canvas/`.

## Regeln

- Das Format liegt als lesbarer Text auf der Platte (JSON, Markdown, YAML …). Bevorzugt ein offenes, bestehendes Format (wie JSON Canvas für `.canvas`), sonst ein einfaches eigenes, das man auch ohne dvoix versteht und von Hand bearbeiten kann.
- Die Ansicht bekommt `path`, `content`, `onchange`, `onopen` (`FileViewProps`). Sie ruft `onchange(neuerInhalt)` bei jeder Änderung auf. **Speichern macht die App**, die Ansicht schreibt nie selbst.
- Die Ansicht muss mit Inhalt umgehen, der sich von außen ändert (Sync, git, anderer Editor): neuen `content` übernehmen, ohne Eingaben zu verlieren.
- Kaputte oder unbekannte Inhalte nie stillschweigend überschreiben. Lieber eine Fehlermeldung zeigen und die Datei unangetastet lassen.
- Parsen und Serialisieren in eine eigene Datei neben der Ansicht (wie `canvas/jsonCanvas.ts`), damit die Logik ohne UI lesbar bleibt. Unbekannte Felder beim Zurückschreiben erhalten.
- `extensions` klein und ohne Punkt. Nur wenn Nutzer den Typ in der App anlegen sollen, `template` angeben, dann erscheint „<label> erstellen“ im Explorer.
- Soll der Typ durchsuchbar sein, braucht `crates/dvoix-core/src/index.rs` eine Extraktion für die Endung (siehe `is_indexable`) plus Test.
- Neuer Dateityp → README ergänzen (Abschnitt „Was schon geht“ und ggf. „Nächste Schritte“).

## Checkliste neuer Dateityp

1. `src/lib/plugins/core/<id>/` mit `<Name>View.svelte`, Format-Modul und `index.ts`.
2. In `core/index.ts` in `corePlugins` eintragen.
3. Datei anlegen, bearbeiten, von außen ändern, Plugin aus- und einschalten: alles muss funktionieren.
4. `npm run check` und `npm run build`.
