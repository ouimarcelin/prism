# 2026-10-17 – Ticket 26 : opacité suivant le son, README

- Branche : `feat/26-opacity-audio`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : code du projet ; aucune nouvelle source

## Prompts donnés

1. « continues le projet en autonomie jusqu'à ne être à la limite de crédit » (suite)

## Plan

1. `Layer::opacity_audio` : l'opacité suit une bande du son ; ordre animation, son, valeur.
2. Menu « — / Son : … » à côté de « ~ » ; préréglages de couche.
3. README à jour pour les phases 1 et 2.

## Résultat

- `prism-core` : `Layer::opacity_audio`, `effective_opacity_at(id, moment, audio)`, `LayerPreset::opacity_audio` ; test.
- `prism-engine` : niveaux du son passés au calcul de l'opacité (image et son des clips).
- `prism-ui` : `band_menu`.
- README : état et mode d'emploi.

## Problèmes et points ouverts

- Groupes de couches (« effets par groupe » du cahier des charges) : comportement non décrit ; une observation écrite est demandée avant d'en faire un ticket.

## Archive

- Transcription : `2026-10-17-020924-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `d596aaebd93c1bb7fea7d4b8077f3d945e95ac8b34e42fb6a1dbea2101a5336f` (même session, archivée après le ticket 27 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
