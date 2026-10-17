# 2026-10-17 – Ticket 46 : réglage fin au clavier

- Branche : `feat/46-reglage-fin`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : aucune nouvelle

## Prompts donnés

1. « continues jusqu'à la limite d'utilisation »

## Plan

1. Mémoriser le dernier point touché de la découpe choisie (coin, grille, masque).
2. Flèches : un pixel de l'écran, dix avec Maj ; Maj lue dans l'événement de la touche.

## Résultat

- Test : flèche gauche, flèche bas avec Maj, rien.
- 276 tests passent.

## Problèmes et points ouverts

- `egui::RawInput` 0.36 n'a plus de champ des touches de modification : Maj lue dans chaque événement de touche.

## Archive

- Transcription : `2026-10-17-214025-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `910ede26f8e170243d56a3b30732a99c9664a83d0e61b24de41ec830ab75b4e6` (même session, archivée après le ticket 51 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
