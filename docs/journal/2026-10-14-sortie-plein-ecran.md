# 2026-10-14 – Ticket 5 : sortie plein écran

- Branche : `feat/output`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : code source de wgpu 30 et winit 0.30 dans le registre Cargo local (API des surfaces, des écrans et du plein écran)

## Prompts donnés

1. « ok, travailles sur les 3 prochains tickets, les uns après les autres » (deuxième des trois)

## Plan

Sortie plein écran sur un second écran, présentée par le thread de rendu pour qu'une interface occupée ne la fasse jamais saccader ; première version de l'application.

## Résultat

- `prism-engine` : trait `Output`, commandes `AddOutput` et `RemoveOutput`, présentation après chaque image.
- `prism-output` : `Presenter` (ajustement avec bandes noires au pixel près ou étirement, écrêtage), `OutputSurface` (synchronisation verticale, format non sRGB, reconfiguration à la volée, images sautées plutôt qu'attendues), `OutputShared` (taille et visibilité, écrites par le thread des fenêtres).
- `prism-app` (binaire `prism`) : fenêtre de contrôle avec aperçu, sortie plein écran (`F`, `Échap`, `--sortie N`), vidéos ou projet en ligne de commande, raccourcis de colonnes et d'arrêt.
- 11 tests ; essai de lancement de l'application avec 4 mires : démarre sur le GPU Metal et tourne sans erreur. La vérification visuelle reste à faire par l'observateur (capture d'écran impossible sans autorisation macOS).
- ADR 0007.

## Problèmes et points ouverts

- Une seule machine de test avec un seul écran pendant la session : la sortie sur second écran n'a pas été essayée.
- `cargo-deny` a signalé `ttf-parser` (non maintenu), tiré par les décorations Wayland de winit, propres à Linux : vérification limitée aux plateformes livrées.
- À faire : empêcher la mise en veille de l'écran ; caler le rythme sur la synchronisation de l'écran.

## Archive

- Transcription : à archiver avec `scripts/archive-session.sh` en fin de session.
