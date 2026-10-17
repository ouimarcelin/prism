# 2026-10-17 – Ticket 58 : réception DMX

- Branche : `feat/58-reception-dmx`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : aucune nouvelle

## Prompts donnés

1. « tout est parfait, continues »

## Résultat

- `ControlSource::Dmx`, `DmxInput` (Art-Net, sACN), préférences et menu Contrôle.
- Tests : messages des canaux changés (aucun au premier paquet), paquet Art-Net envoyé sur la machine puis reçu, canal appris puis pilotant l'opacité.
- 300 tests passent.

## Problèmes et points ouverts

- sACN : seuls les univers 1 à 16 sont rejoints.
- À essayer avec une vraie console (ou un logiciel de console, par exemple en Art-Net sur le réseau local).
