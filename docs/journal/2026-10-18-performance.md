# 2026-10-18 – Ticket 70 : performance

- Rôles : @ouimarcelin observateur, Claude Code implémenteur

## Prompt

« complètes la phase 6 en autonomie. Je ferais les tests à la fin. »

## Plan

- Refaire la mesure de la phase 1 (4 couches HAP 1080p à 60 images/s) après les phases 2 à 5.
- Ajouter des effets à la mesure : option `--effects N` de `engine-run`.
- Optimiser si la mesure montre un goulot d'étranglement.

## Résultat

- Rapport `docs/benchmarks/2026-10-18-phase-6.md`.
- Processeur : 1,5 à 3 ms par image en moyenne, effets compris.
- GPU : saccades isolées, autant sans effet qu'avec ; la machine est chargée (swap, disque presque plein, autres applications sur le GPU). Pas d'optimisation à l'aveugle ; trois pistes notées pour après la mesure en conditions propres.

## Problèmes

- Disque presque plein (1,4 Go libres pendant la mesure) : clips de test réduits à 30 images, effacés après la mesure.
