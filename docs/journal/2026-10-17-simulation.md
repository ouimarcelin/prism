# 2026-10-17 – Ticket 43 : simulation des projecteurs

- Branche : `feat/43-simulation`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : aucune nouvelle

## Prompts donnés

1. « ok continues », puis « continues jusqu'à la limite d'utilisation ».
2. Choix antérieur : pas de projecteur, mode de test à prévoir.

## Plan

1. `Stage::simulation` : activation, recouvrement, gamma ; disposition côte à côte.
2. `SimulationRenderer` : accumulation de la lumière, puis retour aux valeurs de pixels.
3. Moteur : image de simulation partagée avec l'interface.
4. Fenêtre « Sortie avancée » : case « Simulation » et aperçu sous les vues.
5. Test de bout en bout du critère.

## Résultat

- Critère de la phase 4 répété en test automatique : composition unie à 0,6, deux écrans partageant 16 pixels avec un fondu de 40 % : toute la ligne reste à 0,6 ± 0,02 ; sans fondu, la bande monte au-dessus de 0,75.
- 274 tests passent.

## Problèmes et points ouverts

- La simulation suppose des projecteurs parfaits (noir nul, même luminosité) : le test réel sur deux projecteurs reste à faire.
- Disposition horizontale seulement.
