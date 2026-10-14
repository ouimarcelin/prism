# 2026-10-14 – Ticket 16 : effets supplémentaires

- Branche : `feat/16-more-effects`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : formules courantes de traitement d'image (filtre de Sobel, postérisation, seuil adouci, conversion TSV vers RVB, hachage pseudo-aléatoire par sinus), connues de mémoire (ligne générale déjà inscrite dans `docs/SOURCES.md`)

## Prompts donnés

1. « c'est parfait ! travailles sur les 5 prochains tickets. » (quatrième des cinq)

## Plan

Atteindre les 15 à 20 effets demandés pour la phase 2, avec des noms et des réglages génériques, et en caler certains sur le tempo.

## Résultat

- Douze effets : pixellisation, postérisation, seuil, contours, décalage RVB, glitch, vague, tourbillon, mosaïque, vignette, stroboscope, colorisation. 18 effets en tout.
- Glitch, vague et stroboscope lisent la position en temps du tempo : ils restent calés sur le BPM, quelle que soit la durée.
- Exemple `gallery` : planche de tous les effets sur une mire. Elle a montré un défaut, corrigé : une bande verte au bord du décalage RVB.
- Tests : cinq tests de plus (réglages neutres, niveaux, mise en page, stroboscope sur le temps, couleurs) ; 188 tests au total.

## Problèmes et points ouverts

- Pas encore d'effet à mémoire d'image (traînée, rétroaction) : il faudrait garder l'image précédente par chaîne.
- Générateurs (couleur unie, dégradés, texte, shaders ISF) : dans un prochain lot.

## Archive

- Transcription : `2026-10-14-153155-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `81637a22cd6a8019b0a61ba75e31d3437ce01309cbbe2824f28a6953e968cd39` (même session, archivée à 15 h 31 après le ticket 17 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
