# 2026-10-17 – Ticket 27 : temps GPU des effets

- Branche : `feat/27-fx-bench`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : réduction du nombre de lectures d'un flou gaussien par le filtrage linéaire (technique courante, connue de mémoire)

## Prompts donnés

1. « continues le projet en autonomie jusqu'à ne être à la limite de crédit » (suite)

## Plan

1. Exemple `fx-bench` : chaque effet seul en 1080p, temps GPU par image.
2. Corriger ce qui coûte trop.
3. Rapport dans `docs/benchmarks/`.

## Résultat

- `fx-bench`, `EffectInfo::passes`.
- Flou : poids constants et paires de lectures fusionnées, 2,16 → 0,88 ms.
- Rapport `2026-10-17-phase-2-effets.md` ; section dans `docs/benchmarks/README.md`.

## Problèmes et points ouverts

- Mesure lancée depuis Claude Code (priorité réduite) : à refaire depuis le Terminal pour des chiffres de référence.

## Archive

- Transcription : `2026-10-17-020924-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `d596aaebd93c1bb7fea7d4b8077f3d945e95ac8b34e42fb6a1dbea2101a5336f` (même session, archivée après le ticket 27 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
