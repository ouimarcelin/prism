# 2026-10-17 – Ticket 37 : pad MIDI virtuel

- Branche : `feat/37-pad-midi-virtuel`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : code source de `midir` 0.11 (ports virtuels), ligne dans `docs/SOURCES.md`

## Prompts donnés

1. « je n'ai pas de consol midi » (en argument de `/caveman`) ; options proposées : pad virtuel dans Prism, appli sur tablette, ou changement du critère de la phase 3. Sans choix explicite, option par défaut annoncée : le pad virtuel.

## Plan

1. Test de bout en bout : source et destination virtuelles, `MidiInputs` puis `MidiOutputs`.
2. Fenêtre de contrôle capable de dessiner sans image programme.
3. Exemple `midi-pad` : pads, boutons, faders, crossfader ; retour lumineux ; touches 1 à 8.
4. README, ADR 0021 complété, CHANGELOG.

## Résultat

- `crates/control/tests/virtual_midi.rs` : note reçue, retour reçu.
- `crates/app/examples/midi-pad.rs` (macOS et Linux), avec trois tests de l'état des commandes ; mise en page vérifiée par un rendu hors écran.
- 245 tests passent.

## Problèmes et points ouverts

- La capture d'écran de la fenêtre donne une image blanche (pas d'autorisation d'enregistrement de l'écran) : mise en page vérifiée par un rendu PNG temporaire, non conservé.
- Le critère de la phase 3 se joue alors au pad virtuel, à la souris dans sa fenêtre : c'est le contrôleur qui est simulé, Prism reste sans souris.

## Archive

- Transcription : `2026-10-17-103951-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `5604bae0e229326c88276abe62a24135830b18594e876ac04c21392682e6b0a8` (même session que les tickets 33 à 36 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
