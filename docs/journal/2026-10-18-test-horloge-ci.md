# 2026-10-18 – Ticket 67 : test d'horloge MIDI instable en CI

## Prompt

« complètes la phase 6 en autonomie. Je ferais les tests à la fin. » Avant de commencer : la CI (job `rust (macOS)`) échoue depuis le ticket 54.

## Plan

- Cause : `a_midi_clock_gives_the_tempo` envoyait 60 impulsions puis lisait le tempo ; sur une machine de CI lente, plus de 500 ms passaient avant la lecture et l'horloge était tenue pour arrêtée (tempo absent).
- Correction : impulsions envoyées par un thread jusqu'à ce que le tempo soit lu ; attente des conditions portée de 2 à 5 s.

## Résultat

- Test vert en local ; à confirmer sur la CI après fusion.

## Problèmes

- Les échecs de CI n'avaient pas été remarqués pendant 15 fusions : vérifier l'état de la CI après chaque fusion.
