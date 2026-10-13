# Prism

Logiciel open source de VJing et de projection mapping, écrit en Rust, avec rendu entièrement sur GPU.

> **Projet indépendant, non affilié à Resolume.** <!-- clean-room-allow -->
> Prism n'est ni soutenu ni approuvé par Resolume. Toutes les marques citées appartiennent à leurs propriétaires.

## État

Phase 0 : mise en place du dépôt. Aucune fonction utilisable pour l'instant.
Feuille de route : [`docs/SPEC.md`](docs/SPEC.md), section « Plan de développement ».

| Phase | Contenu | Version |
| --- | --- | --- |
| 1 | Moteur de base : composition, couches, HAP et H.264, modes de fusion, crossfader, sortie plein écran, sauvegarde JSON | 0.1 |
| 2 | Effets GPU, générateurs, animation de paramètres, tempo, FFT | 0.2 |
| 3 | MIDI, OSC, raccourcis, entrées caméra, NDI, Syphon | 0.3 |
| 4 | Sortie avancée : slices, warping, masques, edge blending | 0.4 |
| 5 | Pixel mapping DMX, réception DMX, timecode | 0.5 |
| 6 | Portage Windows, API réseau, plugins, installeurs signés | 1.0 |

Plateforme cible : macOS 13+ (Apple Silicon et Intel) d'abord, Windows ensuite.

## Compiler

Prérequis : [Rust stable](https://rustup.rs), Xcode Command Line Tools, FFmpeg (`brew install ffmpeg pkg-config`).
Les commandes de compilation seront ajoutées avec le premier code (phase 1).

## Développement clean room

Prism est écrit uniquement à partir de sources publiques et de descriptions de comportement rédigées par l'équipe. Aucun code, binaire, fichier, visuel ou média de Resolume n'est utilisé. Détails et preuves : [`docs/CLEAN_ROOM.md`](docs/CLEAN_ROOM.md).

## Contribuer

Voir [`CONTRIBUTING.md`](CONTRIBUTING.md). Chaque commit doit porter une signature DCO (`git commit -s`).

## Licence

Au choix :

- Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

Sauf mention contraire, toute contribution soumise pour inclusion dans Prism, au sens de la licence Apache 2.0, est placée sous cette double licence, sans condition supplémentaire.
