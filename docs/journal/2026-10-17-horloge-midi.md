# 2026-10-17 – Ticket 54 : horloge MIDI

- Branche : `feat/54-horloge-midi`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : aucune nouvelle (messages temps réel de la norme MIDI 1.0, connus de mémoire)

## Prompts donnés

1. « à quoi sert abelton link ? et pourquoi il ne fonctionne pas ? » (réponse dans la conversation)
2. « 4, synchro par l'horloge MIDI est parfait. »

## Plan

1. `MidiClock` : tempo des impulsions.
2. `MidiInputs` : impulsions, Start, Stop.
3. Interface : préférence, état dans le menu Contrôle, tempo et Resync en direct.
4. ADR 0032.

## Résultat

- Tests : calcul avec gigue (125 BPM à 0,2 près), port MIDI virtuel envoyant 60 impulsions à 125 BPM, suivi par l'interface (tempo au dixième, Start en Resync, rien à annuler).
- 284 tests passent.

## Problèmes et points ouverts

- À essayer avec un vrai logiciel qui envoie l'horloge (Ableton Live, Traktor, rekordbox… : réglage « envoyer l'horloge MIDI » vers un port, par exemple le bus IAC de macOS).
- Ticket 55 (même journée) : le pad virtuel envoie une horloge MIDI, pour l'essai sans logiciel de musique.
