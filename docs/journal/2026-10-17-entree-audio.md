# 2026-10-17 – Ticket 23 : entrée audio et bandes de fréquences

- Branche : `feat/23-audio-input`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : code source de `cpal` et `rustfft` (voir `docs/SOURCES.md`)

## Prompts donnés

1. « continues le projet en autonomie jusqu'à ne être à la limite de crédit » (suite)

## Plan

1. Crate `prism-audio` : analyse par bandes (testée sur des sinus), niveaux partagés sans verrou, entrée `cpal`.
2. Modèle et rendu : `Band`, `Effect::audio`, `FrameTime::audio`, niveaux lus par le moteur.
3. Interface : menu Audio, vumètres, bandes dans le menu de lien des réglages ; l'application suit les préférences.

## Résultat

- `prism-audio` : `Analyzer`, `AudioLevels`, `AudioInput`, `input_devices` ; tests (sinus par bande, décibels, retombée, gain) ; exemple `audio-meter` (`--list` vérifié : deux entrées trouvées).
- `prism-core` : `Band`, `BAND_COUNT`, `Effect::audio`.
- `prism-fx`, `prism-engine` : réglages suivant une bande ; test du moteur (luminosité d'une couleur unie suivant les graves).
- `prism-ui`, `prism-app` : menu Audio, préférences, vumètres, lien « Son : … » ; vérifié avec `ui-snapshot`.
- ADR 0019.

## Problèmes et points ouverts

- L'écoute réelle n'a pas été lancée ici (accès au micro à accorder) : à essayer à la main avec `audio-meter` puis dans l'application.
- Bandes fixes ; opacité des couches pas encore reliable au son.

## Archive

- Transcription : à archiver en fin de session.
