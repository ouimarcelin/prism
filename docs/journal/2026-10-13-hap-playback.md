# 2026-10-13 – Ticket 3 : lecture HAP

- Branche : `feat/hap-playback`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : spécification publique HAP (dépôt Vidvox/hap), formats QuickTime/MP4 et YCoCg-DXT de mémoire, outil `ffmpeg` en boîte noire (voir `docs/SOURCES.md`)

## Prompts donnés

1. « go »

## Plan validé

Crate `prism-media` : lecteur de conteneur maison avec accès direct à toute image, décompression Snappy, textures BC envoyées telles quelles au GPU ; HAP, HAP Alpha, HAP Q (YCoCg) ; fichiers de test générés en Rust (ADR 0003).

## Résultat

- `prism-media` (sans GPU) : `mov` (atomes `moov`/`trak`/`stbl`, tables `stsz`, `stts`, `stsc`, `stco`/`co64`), `hap` (sections courtes et longues, Snappy, blocs avec table de tailles et de décalages, images multiples), `HapDecoder` (décode n'importe quelle image en réutilisant ses tampons).
- Fonctionnalité `test-support` : écriture de trames HAP et de fichiers QuickTime ; exemple `make-test-clip`.
- `prism-render` : demande la compression BC au GPU ; `FrameUploader` et `VideoTexture` convertissent une trame en texture de couche (alpha prémultiplié, YCoCg, alpha séparé, marge des blocs retirée).
- Exemple `hap-check` : propriétés, vitesse, capture PNG.
- 32 nouveaux tests (5 + 11 dans `prism-media`, 8 GPU de conversion, plus les tests du compositeur inchangés), tous exécutés sur le GPU de la machine.
- Vérification croisée : les 4 variantes de mires générées sont relues avec les bonnes couleurs par le décodeur HAP de `ffmpeg`.
- Mesure sur Apple M1, mire 1080p : décodage 0,20 ms (HAP) à 0,58 ms (HAP Q Alpha) par image ; envoi et conversion avec attente du GPU 1,8 à 2,0 ms. Budget pour 4 couches à 60 images/s : 4,17 ms par couche. Les mires se compressent bien mieux que de vraies vidéos : à confirmer avec des fichiers réels.

## Problèmes et points ouverts

- `ffmpeg` refuse les sections inconnues dans les instructions de décodage HAP, alors que la spécification demande de les ignorer. Notre décodeur suit la spécification (test unitaire dédié) ; l'écrivain de test n'en produit pas, pour rester lisible par les autres outils.
- Nom de champ `layout` refusé par WGSL (mot réservé) : renommé `color_layout`.
- Pas encore pris en charge : HAP R (BC7) n'a pas de test faute d'encodeur BC7 de test ; HapA (alpha seul) et HapH (HDR) sont refusés avec un message clair ; pas de repli CPU si le GPU n'a pas la compression BC (prévu par l'ADR 0003, à faire).
- Le décodage des blocs pourrait se faire en parallèle (c'est le but du découpage HAP) : à envisager si les fichiers réels l'exigent.
- Test à la main : lancer `hap-check` sur vos propres fichiers HAP, HAP Alpha et HAP Q 1080p, vérifier la capture PNG et la ligne `total`.

## Archive

- Transcription : à archiver avec `scripts/archive-session.sh` en fin de session.
