# 2026-10-18 – Validation de la phase 5

- Rôles : @ouimarcelin observateur, Claude Code implémenteur

## Critère

« Une bande LED Art-Net suit l'image ; un clip démarre au bon timecode » (`docs/SPEC.md`).

## Constat

- Pas de bande LED ni de source de timecode matérielle : critère répété par des tests automatiques (paquets Art-Net et sACN produits depuis l'image, déclenchement d'un clip à son timecode MTC et LTC) et par l'exemple `midi-pad` (horloge MIDI et MTC).
- Réponse de l'observateur à la question de validation : « Oui, phase 5 validée ».

## Décision

Phase 5 tenue pour validée par l'observateur le 2026-10-18 ; la phase 6 (finition) commence par l'API réseau.

## Restes

- Essai réel sur une bande LED Art-Net et avec une source LTC.
- Version 0.5 non publiée.
