# 2026-10-14 – Ticket 17 : animation des paramètres

- Branche : `feat/17-animation`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : formes d'onde courantes d'un LFO et hachage SplitMix64, connus de mémoire (voir `docs/SOURCES.md`)

## Prompts donnés

1. « c'est parfait ! travailles sur les 5 prochains tickets. » (cinquième des cinq)

## Plan

1. Modèle : animation d'un réglage d'effet, valeur calculée à partir du temps.
2. Rendu : réglages animés à chaque image.
3. Interface : démarrer, régler, arrêter une animation.
4. Tester le critère de la phase 2.

## Résultat

- `prism-core` : `Animation` (forme, période en temps ou en secondes, valeurs basse et haute, phase), `Effect::animations`.
- `prism-fx` : `EffectInfo::values_at`, `FrameTime` en `f64`.
- `prism-ui` : bouton « ~ » par réglage, forme, durée du cycle, « au tempo », valeurs de… à… ; réglage grisé pendant l'animation. Vérifié à l'œil avec `ui-snapshot`.
- Test du critère de la phase 2 sans GPU : à 128 BPM et 60 images/s, chaque image de la dixième minute montre un effet en carré d'un temps exactement calé ; l'animation reste juste au-delà de 4 millions de temps.
- ADR 0016.

## Problèmes et points ouverts

- Critère de la phase 2 à vérifier à la main : un effet cadencé sur le BPM d'une musique, 10 minutes.
- Restent pour la phase 2 : générateurs (couleur unie, dégradés, lignes, bruit, texte, shaders ISF), animation de l'opacité des couches et d'autres réglages, enveloppes, tableau de 8 curseurs par clip, audio et FFT.

## Archive

- Transcription : `2026-10-14-153155-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `81637a22cd6a8019b0a61ba75e31d3437ce01309cbbe2824f28a6953e968cd39` (même session, archivée à 15 h 31 après le ticket 17 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
