# 2026-10-17 – Ticket 40 : déformation par grille

- Branche : `feat/40-deformation`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : aucune nouvelle (transformation projective du carré unité et courbes de Catmull-Rom, connues de mémoire)

## Prompts donnés

1. « ok continues »

## Plan

1. Modèle `Warp` : grille, perspective initiale, tracé droit ou lissé, redécoupage.
2. Rendu par maillage fin.
3. Fenêtre « Sortie avancée » : choix du tracé, cellules, points déplaçables.
4. ADR 0028.

## Résultat

- 265 tests passent : modèle (4), rendu GPU (grille identique à la perspective, grille déplacée), mise en page vérifiée par `ui-snapshot … stage`.

## Problèmes et points ouverts

- Glisser les points non essayé à la main pendant la session.
