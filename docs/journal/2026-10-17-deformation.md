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

## Archive

- Transcription : `2026-10-17-211211-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `ae720a909bd047894e3505348c6dd7ef7e29179ce6f7c91b9226e34fb3d60a77` (même session que les tickets 33 à 45, archivée après le ticket 45 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
