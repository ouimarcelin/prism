# 2026-10-17 – Ticket 38 : scène de sortie (écrans et découpes)

- Branche : `feat/38-scene-de-sortie`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : aucune nouvelle (méthode des poids des diagonales pour la perspective d'un quadrilatère, connue de mémoire)

## Prompts donnés

1. « ok tout fonctionne » ; phase 3 validée, phase 4 lancée. Choix : éditeur dans une fenêtre à part ; pas de projecteur (mode de test à prévoir).

## Plan

1. Modèle `Stage` : écrans, découpes, fractions, identifiants.
2. `StageRenderer` : découpes en perspective, tests GPU.
3. Moteur : images des écrans après le programme, `OutputImages`.
4. Fenêtres de sortie liées aux écrans de la scène par l'écran système.

## Résultat

- 256 tests passent, dont : modèle (4), poids de perspective (2), rendu GPU d'une découpe placée et d'un trapèze (ligne du milieu au tiers de la hauteur), écrans suivis par le moteur.
- ADR 0027.

## Problèmes et points ouverts

- Pas encore d'interface pour créer des écrans et des découpes : fenêtre « Sortie avancée » au ticket 39.
