# 2026-10-14 – Ticket 4 : moteur de lecture

- Branches : `chore/wgpu-30`, puis `feat/engine`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : `docs/SPEC.md`, API de crates.io (versions d'egui et de wgpu), code source de wgpu 30 dans le registre Cargo local

## Prompts donnés

1. « ok, travailles sur les 3 prochains tickets, les uns après les autres » (moteur de lecture, sortie plein écran, interface)

## Plan

Moteur sur son propre thread, têtes de lecture, décodage sur threads dédiés, transitions ; avant cela, montée de wgpu imposée par egui.

## Résultat

- `chore/wgpu-30` : aucune version d'egui ne cible wgpu 26 ; montée à wgpu 30 (egui 0.36 à venir), Rust 1.95 minimum, `paste` disparaît (ADR 0005). La CI n'annule plus une exécution sur `main` quand un nouveau push arrive.
- Compositeur : fondu entre deux clips d'une couche en une passe, `Compositor::encode` pour une seule soumission GPU par image.
- `prism-core` : compteur de démarrages par couche.
- `prism-engine` : `Playhead` (logique pure, 13 tests), worker de décodage par clip, `ClipStream` (cache, anticipation, envoi GPU), `EngineState` (pilotable image par image), `Engine` (thread, commandes, état publié), exemple `engine-run`.
- 8 tests de bout en bout sur le GPU : lecture en temps réel, pause, lecture arrière, scratch, points de repère, relance, média manquant, fondu, arrêt, cadence du thread.
- Mesure sur Apple M1 avec `engine-run` et 4 mires 1080p (HAP, HAP Alpha, HAP Q, HAP Q Alpha) : 60,0 images/s, 0 image en retard, environ 4 ms de travail par image sur le thread de rendu.

## Problèmes et points ouverts

- Les mires se compressent bien mieux que de vraies vidéos : la mesure est à refaire avec des fichiers réels (ticket « benchmark »).
- Un clip déclenché ouvre son fichier à ce moment-là : préchargement à envisager.
- Choix faits sans observation : la lecture aléatoire saute à une image au hasard une fois par image du média (multiplié par la vitesse) ; un clip « une fois » garde sa dernière image à l'écran ; un redéclenchement relance le clip sans fondu.

## Archive

- Transcription : `2026-10-14-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `955a36a2600b154657f197381787cd82ef89bef31efd45eb9d96556c2a8b97cd` (session Claude Code unique des tickets 1 à 11, archivée le 2026-10-14 à 12 h 11 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
