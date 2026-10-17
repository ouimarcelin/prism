# 2026-10-17 – Validation de la phase 3

- Rôles : @ouimarcelin observateur (tests à la main), Claude Code implémenteur

## Critère

« Un set complet joué au contrôleur MIDI sans souris » (`docs/SPEC.md`, plan de développement).

## Tests de l'observateur

- Pas de contrôleur MIDI matériel : set joué au pad MIDI virtuel (exemple `midi-pad`, ticket 37), commandes apprises puis jouées sans souris dans Prism. Réponse : « la midi fonctionne très bien », puis « parfait ».
- Syphon dans les deux sens avec TouchDesigner : « ok tout fonctionne » (après choix du serveur « Prism » dans l'opérateur Syphon Spout In).

## Décision

Phase 3 validée par l'observateur le 2026-10-17 ; la phase 4 (mapping) commence.

## Restes

- NDI (option `ndi`, ADR 0026) : testé en aller-retour sur ce Mac seulement, pas encore avec un autre logiciel.
- Caméra, OSC depuis une tablette, sortie sur deux écrans : non testés à la main.
- Contrôleur MIDI matériel : à essayer dès qu'il y en a un.
- Version 0.3 non publiée (choix de l'observateur).
