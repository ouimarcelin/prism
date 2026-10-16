# 2026-10-17 – Ticket 24 : préréglages d'effets et de couches

- Branche : `feat/24-presets`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : code du projet ; aucune nouvelle source

## Prompts donnés

1. « continues le projet en autonomie jusqu'à ne être à la limite de crédit » (suite)

## Plan

Fonction P1 de la phase 1 encore absente (« presets d'effets et de couches »).

1. `prism-core` : `Preset`, `PresetContent`, `LayerPreset`, enregistrement, lecture, liste.
2. `prism-ui` : `PresetStore` (dossier à côté des préférences), menus des chaînes et de la couche.

## Résultat

- `prism-core` : `preset.rs`, fichiers versionnés, noms de fichiers nettoyés, tri sans accents ; tests.
- `prism-ui` : `presets.rs`, menus « Préréglages… » ; test d'enregistrement et de relecture ; vérifié avec `ui-snapshot`.
- Décision ajoutée à l'ADR 0002.

## Problèmes et points ouverts

- Pas de renommage ni de suppression depuis l'interface (passer par le Finder).
- Incident de disque : `target/` avait grossi à 29 Go, disque plein à 95 %. iCloud (Documents synchronisé) a déchargé des fichiers du projet et de `.git` : écritures de git expirées (`index.lock … Operation timed out`), commit des docs et fusion avortés. Docs remises au propre, `cargo clean` (34,5 Go libérés), fichiers rapatriés avec `brctl download`, `git fsck` sans erreur, docs refaites.
- Recommandation : sortir le dépôt du dossier Documents synchronisé par iCloud (ou au moins le dossier `target/`).

## Archive

- Transcription : à archiver en fin de session.
