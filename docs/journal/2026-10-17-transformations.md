# 2026-10-17 – Ticket 45 : transformations d'entrée et de sortie

- Branche : `feat/45-transformations`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : aucune nouvelle

## Prompts donnés

1. « continues jusqu'à la limite d'utilisation »

## Plan

1. Modèle : rotation et miroirs de la découpe, miroirs de l'écran.
2. Rendu : point lu dans la composition calculé par la découpe ; position dans la découpe portée par les sommets ; miroir de l'écran appliqué aux sommets et aux masques.
3. Fenêtre « Sortie avancée » : réglages, contour tourné.

## Résultat

- Tests : modèle (miroir, demi-tour, quart de tour sur une zone carrée en pixels), GPU (demi-tour, double miroir qui s'annule, fondu qui suit la découpe sur un écran retourné).
- 275 tests passent. Le cahier des charges de la phase 4 est couvert : écrans, découpes, déformation par grille, masques, transformations, edge blending.

## Problèmes et points ouverts

- Rotation de la zone lue : la poignée de la vue de la composition reste celle du rectangle non tourné.
- Pas de rotation de l'écran par quarts de tour (un projecteur en portrait se règle par la taille de l'écran et les coins de la découpe).

## Archive

- Transcription : `2026-10-17-211211-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `ae720a909bd047894e3505348c6dd7ef7e29179ce6f7c91b9226e34fb3d60a77` (même session que les tickets 33 à 45, archivée après le ticket 45 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
