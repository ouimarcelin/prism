# Prism

Logiciel open source de VJing et de projection mapping, écrit en Rust, avec rendu entièrement sur GPU.

> **Projet indépendant, non affilié à Resolume.** <!-- clean-room-allow -->
> Prism n'est ni soutenu ni approuvé par Resolume. Toutes les marques citées appartiennent à leurs propriétaires.

## État

Phase 1 en cours : modèle de données et sauvegarde des projets. Aucune application utilisable pour l'instant.
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

Prérequis : [Rust stable](https://rustup.rs) et les Xcode Command Line Tools.

```bash
cargo build --workspace --release
cargo test --workspace
```

Avant chaque pull request :

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
scripts/check-clean-room.sh
```

## Lancer Prism

```bash
cargo run --release -p prism-app -- media/a.mov media/b.mov media/c.mov media/d.mov
```

Les vidéos HAP vont dans la première colonne, une par couche, et la première colonne démarre. La fenêtre de contrôle montre l'aperçu ; `F` ouvre la sortie plein écran sur le second écran (ou `--sortie N` au lancement), `Échap` la ferme, `1` à `9` déclenchent les colonnes, `Espace` arrête tout. `prism --help` liste les options.

## Tester la lecture HAP

Générer une mire HAP 1080p de 4 secondes (variantes : `hap`, `hap-alpha`, `hap-q`, `hap-q-alpha`) :

```bash
cargo run --release -p prism-media --features test-support --example make-test-clip -- media/mire.mov hap-q
```

Vérifier un fichier HAP (le vôtre ou la mire) : propriétés, vitesse de décodage et d'envoi au GPU, capture PNG d'une image :

```bash
cargo run --release -p prism-render --example hap-check -- media/mire.mov 30 media/mire.png
```

Faire tourner le moteur sans fenêtre sur 1 à 4 fichiers HAP (une couche par fichier, 1080p60) et vérifier qu'il tient 60 images/s :

```bash
cargo run --release -p prism-engine --example engine-run -- --seconds 30 media/a.mov media/b.mov media/c.mov media/d.mov
```

## Développement clean room

Prism est écrit uniquement à partir de sources publiques et de descriptions de comportement rédigées par l'équipe. Aucun code, binaire, fichier, visuel ou média de Resolume n'est utilisé. Détails et preuves : [`docs/CLEAN_ROOM.md`](docs/CLEAN_ROOM.md).

## Contribuer

Voir [`CONTRIBUTING.md`](CONTRIBUTING.md). Chaque commit doit porter une signature DCO (`git commit -s`).

## Licence

Au choix :

- Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

Sauf mention contraire, toute contribution soumise pour inclusion dans Prism, au sens de la licence Apache 2.0, est placée sous cette double licence, sans condition supplémentaire.
