# 2026-10-17 – Ticket 39 : fenêtre « Sortie avancée »

- Branche : `feat/39-sortie-avancee`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : aucune nouvelle

## Prompts donnés

1. Suite de la phase 4 ; choix de l'observateur : éditeur dans une fenêtre à part.

## Plan

1. Moteur : images des écrans partagées avec l'interface.
2. Fenêtre de contrôle : plusieurs images enregistrées auprès d'egui.
3. `prism-ui` : éditeur (écrans, découpes, vues entrée et sortie, poignées).
4. Application : seconde fenêtre ouverte par le menu Sortie.

## Résultat

- Éditeur en conception propre : composition à gauche avec les zones des découpes, écran à droite avec leurs coins, aperçus en direct ; un clic choisit la découpe sous le pointeur.
- Tests de la géométrie de l'éditeur (coins, déplacement, point dans un quadrilatère, ajustement) ; mise en page vérifiée par `ui-snapshot … stage`.
- 260 tests passent.

## Problèmes et points ouverts

- Les flèches ↑ ↓ n'existent pas dans les polices de l'interface : remplacées par ⬆ ⬇.
- Glisser dans la fenêtre non essayé à la main pendant la session.

## Archive

- Transcription : `2026-10-17-211211-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `ae720a909bd047894e3505348c6dd7ef7e29179ce6f7c91b9226e34fb3d60a77` (même session que les tickets 33 à 45, archivée après le ticket 45 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
