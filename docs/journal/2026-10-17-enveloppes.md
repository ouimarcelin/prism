# 2026-10-17 – Ticket 20 : enveloppes

- Branche : `feat/20-envelopes`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : code du projet ; aucune nouvelle source

## Prompts donnés

1. « continues le projet en autonomie jusqu'à ne être à la limite de crédit »

## Plan

1. Modèle : mode « une fois » des animations, compté depuis le lancement du clip ; type `Moment` (temps, secondes, lancement).
2. Moteur : lancement noté par flux (ouverture et relance), passé aux chaînes, à l'opacité et aux effets de la composition.
3. Interface : bouton « une fois » dans les réglages d'une animation.

## Résultat

- `prism-core` : `Animation::once`, `Moment`, `value_at(Moment)`, `Composition::effective_opacity_at(id, Moment)` ; tests des formes, des secondes et de l'aléatoire par lancement.
- `prism-fx` : `FrameTime::{launch_beat, launch_seconds}` et `moment()`.
- `prism-engine` : `ClipStream::launched`, lancement pris à la dernière image ; test d'une rampe d'une seconde à 60 BPM, relancée.
- `prism-ui` : bouton « une fois » ; vérifié avec `ui-snapshot`.
- Décision ajoutée à l'ADR 0016 (choix fait sans demander, mode autonome : enveloppe = un cycle d'animation, pas d'ADSR).

## Problèmes et points ouverts

- Pas de phase de relâchement à l'arrêt d'un clip (ADSR complet) : à voir sur observation si le besoin existe.
- Le bouton « une fois » touche le bord de l'inspecteur à 1440 px de large.

## Archive

- Transcription : à archiver en fin de session.
