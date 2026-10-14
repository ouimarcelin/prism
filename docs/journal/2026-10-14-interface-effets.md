# 2026-10-14 – Ticket 15 : interface des effets

- Branche : `feat/15-effects-ui`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : code du projet, d'egui 0.36 et d'egui-wgpu 0.36, déjà inscrits ; aucune nouvelle source

## Prompts donnés

1. « c'est parfait ! travailles sur les 5 prochains tickets. » (troisième des cinq)

## Plan

1. Éditeur de chaîne d'effets : activer, monter, descendre, retirer, ajouter ; réglages lus dans le catalogue.
2. Sections dans l'inspecteur pour la composition, la couche et le clip.
3. Vérifier la mise en page à l'œil.

## Résultat

- `prism-ui` : module `effects` (`effect_section`, éditions séparées en `ChainEdit` testées à part), sections dans l'inspecteur ; réglages non touchés jamais écrits dans le projet ; `ControlPanel::select`.
- Outil `ui-snapshot` (crate `prism-app`) : la fenêtre de contrôle dessinée dans une image PNG avec un projet d'exemple. Il a montré deux défauts, corrigés : les flèches « ↑ ↓ » absentes de la police d'egui (carrés vides, remplacées par « ⏶ ⏷ »), et le crossfader limité à 100 points dans une zone de 320.
- Tests : éditions de chaîne, affichage des chaînes sans modifier le projet (type inconnu compris). 183 tests au total.

## Problèmes et points ouverts

- Pas de glisser-déposer pour réordonner les effets : boutons monter et descendre.
- Pas encore de presets d'effets (prévus au cahier des charges dans le module Projet).

## Archive

- Transcription : archivée à la fin de la série de tickets 13 à 17, même session ; empreinte reportée à ce moment-là.
