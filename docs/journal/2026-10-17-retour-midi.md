# 2026-10-17 – Ticket 31 : retour MIDI vers le contrôleur

- Branche : `feat/31-midi-feedback`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : `midir` (sorties) ; aucune nouvelle source

## Prompts donnés

1. « parfait, commence les intégrations » (suite de la phase 3)

## Plan

1. État de chaque commande reliée, en messages MIDI (note ou CC).
2. Sorties MIDI du même nom que les entrées écoutées ; envoi des seuls changements.
3. Préférence et case du menu Contrôle.

## Résultat

- `prism-control` : `feedback.rs` (`midi_feedback`, `MidiOutputs`) ; tests.
- `prism-ui` : préférence `midi_feedback` (activée), case du menu.
- `prism-app` : sorties connectées avec les entrées, envoi à chaque image.
- ADR 0021 complété.

## Problèmes et points ouverts

- Les contrôleurs qui attendent d'autres vélocités pour leurs couleurs (palettes de LED) ne reçoivent que 0 et 127 : à adapter sur observation d'un modèle précis.

## Archive

- Transcription : `2026-10-17-091604-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `46591be94c4e5f79b9ea07b2eb13d2e5bd83aaf6e2627b8213861bc6051905bf` (même session, archivée après le ticket 32 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
