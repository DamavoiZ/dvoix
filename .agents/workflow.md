# Arbeitsablauf

## Vor der Änderung

- `AGENTS.md` und die passende Datei in `.agents/` lesen.
- Den betroffenen Code lesen, bevor du ihn änderst. Bestehende Muster übernehmen (ein ähnliches Plugin als Vorlage nehmen).
- Unklare Anforderungen kurz klären oder die naheliegendste Lesart wählen und im PR nennen.

## Während der Änderung

- Nichts nebenbei umbauen oder umformatieren.
- Versionen: neueste Version von Node.js, Rust, Abhängigkeiten und Actions (siehe Grundregel 6 in `AGENTS.md`).
- Keine Geheimnisse, Tokens oder persönlichen Pfade ins Repo.

## Vor dem Commit

```sh
npm run check
npm run build
cargo test --workspace
```

Alle drei müssen grün sein, so wie in der CI. Konnte etwas nicht laufen (z. B. fehlende Systembibliotheken), steht das im PR.

- README aktualisieren, wenn sich für Nutzer oder Entwickler etwas ändert.
- Diese Regeln (`AGENTS.md`, `.agents/`) aktualisieren, wenn sich Struktur, Befehle oder Konventionen ändern. Sie müssen immer zum Code passen.

## Commits und PRs

- Commit-Messages auf Englisch, im Imperativ, eine Zeile Zusammenfassung (z. B. „Add kanban board as a core plugin“), bei Bedarf ein kurzer Absatz mit dem Warum.
- PR-Beschreibung: was sich ändert, warum, wie geprüft. Sichtbare Änderungen mit Screenshot.
- Nie direkt auf `main` pushen, nie fremde Branches umschreiben, keine Tests abschalten, um die CI grün zu bekommen.
