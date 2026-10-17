# 2026-10-17 – Ticket 50 : niveau de noir

- Branche : `feat/50-niveau-de-noir`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : aucune nouvelle

## Prompts donnés

1. « continues jusqu'à la limite d'utilisation »

## Résultat

- `EdgeBlend::black`, shader (relèvement hors des fondus, rampe au bord), réglage « Noir », test GPU (relevé hors du fondu, nul au bord extérieur).
- 280 tests passent.

## Problèmes et points ouverts

- À régler à l'œil avec de vrais projecteurs : la simulation suppose un noir nul.
