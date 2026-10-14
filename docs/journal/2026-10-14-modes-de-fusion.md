# 2026-10-14 – Ticket 7 : modes de fusion supplémentaires

- Branche : `feat/blend-modes`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : formules générales du W3C *Compositing and Blending Level 1*, de mémoire (déjà inscrites dans `docs/SOURCES.md`)

## Prompts donnés

1. « parfait, continues, fais les 5 tickets suivants. » (premier des cinq : modes de fusion, images fixes et séquences PNG, H.264 et ProRes, finitions, benchmark et endurance)

## Résultat

- `BlendMode` : `Lighten`, `Darken`, `Difference`, `Subtract`, `Overlay`, `Exclusion`, en plus de normal, addition, produit et écran (le cahier des charges cite « add, multiply, screen, lighten… »).
- Shader de mixage et référence CPU ; le test GPU existant compare automatiquement chaque mode à la référence ; nouveau test des valeurs attendues.
- Noms français dans l'interface : Éclaircir, Assombrir, Différence, Soustraction, Incrustation, Exclusion.

## Points ouverts

- Les modes s'appliquent aux couleurs en espace d'affichage, comme les quatre premiers (ADR 0004).

## Archive

- Transcription : à archiver avec `scripts/archive-session.sh` en fin de session.
