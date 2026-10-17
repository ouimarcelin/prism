# 2026-10-17 – Ticket 47 : banc et optimisation de la sortie avancée

- Branche : `feat/47-banc-sortie`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : aucune nouvelle

## Prompts donnés

1. « continues jusqu'à la limite d'utilisation »

## Plan

1. Outil `stage-bench` : six cas, deux écrans 1080p.
2. Corriger ce qui dépasse quelques millisecondes.

## Résultat

- Grille 32 × 32 : de 23,5 à 1,5 ms (sommets gardés sur le GPU, courbe sur quatre rangées).
- Masque 64 points : de 24 à 1,2 ms (image de couverture dessinée au changement).
- 276 tests passent ; rapport dans `docs/benchmarks/`.

## Problèmes et points ouverts

- Le banc mesure le régime établi ; une image où un masque change coûte quelques millisecondes de plus.
