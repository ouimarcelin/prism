# 2026-10-17 – Ticket 30 : raccourcis clavier assignables

- Branche : `feat/30-key-shortcuts`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : code source d'egui 0.36 (noms des touches) ; aucune nouvelle source externe

## Prompts donnés

1. « parfait, commence les intégrations » (suite de la phase 3)

## Plan

1. `ControlSource::Key` ; une touche apprise comme une commande MIDI.
2. Fenêtre de contrôle : touches apprises avant les raccourcis habituels ; fenêtre de sortie : mêmes noms de touches.

## Résultat

- `prism-core`, `prism-control` : `ControlSource::Key`, « Touche … ».
- `prism-ui` : `ControlPanel::press_key`, `apply_control`, relâchement ignoré pour l'apprentissage ; test.
- `prism-app` : `key_name` pour la fenêtre de sortie.
- ADR 0021 complété.

## Problèmes et points ouverts

- Les touches de la fenêtre de sortie sont nommées depuis winit (lettres, chiffres, espace, flèches, Entrée, Tab) : les autres touches ne s'y apprennent pas.

## Archive

- Transcription : `2026-10-17-091604-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `46591be94c4e5f79b9ea07b2eb13d2e5bd83aaf6e2627b8213861bc6051905bf` (même session, archivée après le ticket 32 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
