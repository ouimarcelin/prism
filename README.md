# Prism

Logiciel open source de VJing et de projection mapping, écrit en Rust, avec rendu entièrement sur GPU.

> **Projet indépendant, non affilié à Resolume.** <!-- clean-room-allow -->
> Prism n'est ni soutenu ni approuvé par Resolume. Toutes les marques citées appartiennent à leurs propriétaires.

## État

Phases 1 et 2 écrites : lecture et mixage des clips, sortie plein écran, effets, générateurs, animation des réglages, tempo, entrée audio et son des clips. La phase 2 attend son test à la main (un effet cadencé sur le tempo reste synchrone après 10 minutes).
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

Les médias sont facultatifs : ils vont dans la première colonne, un par couche, et la première colonne démarre. Sinon, glissez-les sur la fenêtre ou utilisez Fichier > Importer des médias.

Formats : HAP, HAP Alpha, HAP Q et HAP Q Alpha (recommandés : lecture arrière et scratch instantanés), H.264, ProRes et les autres codecs de macOS, images PNG et JPEG, dossiers d'images PNG joués en séquence.

Dans la fenêtre de contrôle :

- cliquer une case lance son clip, cliquer l'en-tête d'une colonne lance toute la colonne ;
- chaque couche a son opacité, son mode de fusion, B (bypass), S (solo) et son côté du crossfader ;
- le panneau de droite montre l'aperçu, puis les réglages de la couche et du clip sélectionnés (sens, vitesse, boucle, scratch, points de repère) ;
- le menu Sortie ouvre une sortie plein écran sur chaque écran coché ; `F` rouvre les écrans de la dernière fois (sinon le premier écran libre), `Échap` ferme toutes les sorties ;
- `1` à `9` lancent les colonnes, `Espace` arrête tout ; la liste complète est dans Aide > Raccourcis clavier.
- le tempo se règle en bas à gauche (BPM, Tap ou `T`, Resync ou `R`) ;
- « Ajouter un effet… » ajoute un effet à la composition, à la couche ou au clip (18 effets, ou un shader ISF) ; « Préréglages… » enregistre et réutilise des chaînes d'effets et des réglages de couche ;
- une case vide propose « Générateur… » : couleur, dégradé, lignes, cercles, damier, bruit, texte ou shader ISF (exemples dans `examples/isf/`) ;
- à côté de chaque réglage : `~` l'anime (onde au tempo ou en secondes, ou « une fois » au lancement du clip), le menu `—` le relie à un curseur du tableau de bord du clip ou à une bande du son ;
- le menu Audio règle le son des clips et l'écoute d'une entrée audio (graves, médiums, aigus, niveau) ;
- le menu Contrôle relie un contrôleur MIDI ou des messages OSC : « Apprentissage MIDI », cliquer une commande, puis bouger le bouton ou le fader. En OSC, sans rien apprendre : `/prism/layer/1/opacity` (0 à 1), `/prism/layer/1/clip/2`, `/prism/column/3`, `/prism/crossfader`, `/prism/tap`, `/prism/stop`…

`prism --help` liste les options de lancement (`--sortie N` ouvre la sortie au démarrage).

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
