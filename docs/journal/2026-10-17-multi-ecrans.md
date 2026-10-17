# 2026-10-17 – Ticket 36 : sortie sur plusieurs écrans

- Branche : `feat/36-multi-ecrans`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : aucune nouvelle (documentation de winit déjà citée)

## Prompts donnés

1. « continues » (suite de la phase 3 ; NDI attend l'installation du SDK par l'utilisateur)

## Plan

1. Préférences : liste d'écrans, reprise de l'ancien écran unique.
2. Interface : une case à cocher par écran dans le menu Sortie, fermeture de toutes les sorties.
3. Application : liste de fenêtres de sortie, `F` et `Échap` sur toutes, `caffeinate` unique.
4. ADR 0025, README, CHANGELOG.

## Résultat

- `prism-ui` : `Action::ToggleScreen`, `FrameInfo::output_screens`, `Preferences::output_screens` ; test de reprise de l'ancien champ.
- `prism-app` : `Running::outputs`, `default_screens`, `close_output(screen)`, `close_outputs`.
- 241 tests passent, clippy sans avertissement.

## Problèmes et points ouverts

- Non testé avec deux écrans réels : un seul écran disponible pendant la session. À vérifier à la main, surtout avec deux écrans de fréquences différentes (ADR 0025).

## Archive

- Transcription : `2026-10-17-103042-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `d8160720cbba976922ba23452ece6282c724fffddb5a7645b903240dac718b33` (même session que les tickets 33 et 34 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
