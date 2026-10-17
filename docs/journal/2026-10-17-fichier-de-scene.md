# 2026-10-17 – Ticket 49 : fichiers de scène

- Branche : `feat/49-fichier-de-scene`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : aucune nouvelle

## Prompts donnés

1. « continues jusqu'à la limite d'utilisation »

## Plan

1. `save_stage` / `load_stage` : fichier JSON versionné comme les projets, écriture atomique.
2. Fenêtre « Sortie avancée » : « Importer… », « Exporter… ».

## Résultat

- Test : scène enregistrée et relue à l'identique ; version plus récente refusée.
- 279 tests passent.

## Problèmes et points ouverts

- Les noms d'écrans du système enregistrés dans la scène peuvent ne pas exister sur une autre machine : à rechoisir dans la fenêtre.

## Archive

- Transcription : `2026-10-17-214025-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `910ede26f8e170243d56a3b30732a99c9664a83d0e61b24de41ec830ab75b4e6` (même session, archivée après le ticket 51 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
