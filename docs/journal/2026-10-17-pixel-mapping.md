# 2026-10-17 – Ticket 57 : pixel mapping DMX

- Branches : `feat/57-pixel-mapping`, puis `feat/57b-lumiere-ui`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : aucune nouvelle

## Prompts donnés

1. « tout est parfait, continues »

## Plan

1. Modèle `Lighting` et luminaires.
2. `prism-dmx::universes` : couleurs rangées en univers.
3. `PixelSampler` : couleurs lues sur le GPU sans attendre.
4. Moteur : envoi à la cadence demandée ; état.
5. Onglet « Lumière (DMX) ».

## Résultat

- Critère de la phase 5 (première moitié) en test automatique : une bande Art-Net de 10 pixels reçoit le rouge, puis le vert quand l'image change.
- Tests : ordres de couleurs, pixels le long d'une ligne, canaux suivants, débordement d'univers, lecture GPU de 300 points, adresses.
- 298 tests passent ; mise en page vérifiée par `ui-snapshot … lighting`.

## Problèmes et points ouverts

- Pas de luminaire réel pour l'essai ; à brancher sur un contrôleur de LED Art-Net.
- Pas de correction gamma des LED ni de découverte des nœuds (ArtPoll).
