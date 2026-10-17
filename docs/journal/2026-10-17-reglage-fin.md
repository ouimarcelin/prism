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
