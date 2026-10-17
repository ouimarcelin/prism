# 0032 – Tempo suivi de l'horloge MIDI

- Date : 2026-10-17
- Statut : Acceptée

## Contexte

Ableton Link, prévu par le cahier des charges pour suivre le tempo d'autres logiciels, n'existe qu'en code GPL ou sous licence commerciale (ADR 0014). L'observateur a choisi, le 2026-10-17, la synchronisation par l'horloge MIDI (MIDI Clock), norme MIDI 1.0 libre que la plupart des logiciels de musique et de DJ savent envoyer.

## Décision

- L'horloge MIDI envoie 24 impulsions (`0xF8`) par temps, `Start` (`0xFA`) au début du morceau, `Stop` (`0xFC`) à l'arrêt.
- `MidiClock` (crate `prism-clock`) : tempo moyen sur les 48 dernières impulsions (deux temps), datées par le pilote MIDI, ou à leur arrivée quand le message n'a pas de date (ports virtuels) ; une pause de plus de 500 ms recommence la mesure.
- `MidiInputs` lit l'horloge de toutes les entrées écoutées ; les messages d'horloge ne passent pas par les correspondances.
- Menu Contrôle > « Suivre l'horloge MIDI » (préférence) : le tempo de la composition suit l'horloge, arrondi au dixième de BPM ; un `Start` commence une mesure (Resync). Ces changements viennent d'un contrôleur : ils ne s'annulent pas avec Cmd+Z.
- La position dans la mesure n'est prise qu'au `Start` (pas de « Song Position Pointer »).

## Conséquences

- Moins précis que Link pour la phase : la mesure se cale au `Start`, puis suit le tempo. Un logiciel qui ne renvoie pas `Start` en cours de morceau demande un Resync à la main.
- Ableton Link reste possible plus tard comme module séparé (ADR 0014).
