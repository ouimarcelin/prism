# 2026-10-17 – Ticket 42 : fondu des bords et mire

- Branche : `feat/42-fondu-des-bords`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : aucune nouvelle (courbe de fondu complémentaire et correction gamma, connues de mémoire)

## Prompts donnés

1. « ok continues » (suite de la phase 4)

## Plan

1. Modèle `EdgeBlend` et mire par écran.
2. Shader : bloc d'informations par découpe, fondu, mire.
3. Fenêtre « Sortie avancée » : réglages.
4. ADR 0030.

## Résultat

- Tests : somme des deux fondus égale à 1 en lumière, rendu GPU du fondu comparé à la courbe, mire avec bordure blanche.
- 269 tests passent.

## Problèmes et points ouverts

- Pas de niveau de noir.
- Validation réelle sans projecteur : ticket 43 (mode de test à deux fenêtres).

## Archive

- Transcription : `2026-10-17-211211-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `ae720a909bd047894e3505348c6dd7ef7e29179ce6f7c91b9226e34fb3d60a77` (même session que les tickets 33 à 45, archivée après le ticket 45 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
