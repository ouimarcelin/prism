# Registre des sources

Toute source consultée pour le projet est inscrite ici, dans le même commit que le travail qui en découle (voir `docs/CLEAN_ROOM.md`).
Une source absente de ce registre est réputée non consultée.

Rôle : **O** = observateur, **I** = implémenteur (voir `docs/CLEAN_ROOM.md`, section 2).

| Date | Source | Licence / statut | Usage | Consultée par |
| --- | --- | --- | --- | --- |
| 2026-10-13 | Page produit publique de Resolume Avenue & Arena | Publique, protégée | Liste des fonctions à reproduire dans `docs/SPEC.md` | @ouimarcelin (O) |
| 2026-10-13 | Manuel et articles d'aide publics de Resolume (en ligne) | Publics, protégés | Logique de travail décrite dans `docs/SPEC.md` | @ouimarcelin (O) |
| 2026-10-13 | Manuel Resolume 4 en PDF, présent dans le dossier de travail initial | Public, protégé ; exclu du dépôt | Non ouvert par Claude Code. À confirmer par l'observateur : consulté ou non | @ouimarcelin (O) |
| 2026-10-13 | `docs/SPEC.md` | Projet | Base de la mise en place du dépôt | Claude Code (I) |
| 2026-10-13 | Textes de licence MIT et Apache 2.0, API GitHub `licenses` | Textes de licence standard | `LICENSE-MIT`, `LICENSE-APACHE` | Claude Code (I) |
| 2026-10-13 | W3C *Compositing and Blending Level 1* (formules générales, connues de mémoire, non relues en ligne) | Spécification publique du W3C | Formules de mixage de `crates/render` (ADR 0004) | Claude Code (I) |
| 2026-10-13 | Code source de la crate `wgpu` 26 et `wgpu-types` 26 (registre Cargo local) | MIT / Apache 2.0 | Signatures de l'API (`PollType`, `RenderPassColorAttachment`) | Claude Code (I) |
| 2026-10-13 | Spécification HAP « Hap Video » (`documentation/HapVideoDRAFT.md`, dépôt GitHub Vidvox/hap, blob `a0ab743`) | Format ouvert, dépôt sous BSD-2-Clause ; texte lu, aucun code repris | Décodeur de trames `crates/media/src/hap.rs` | Claude Code (I) |
| 2026-10-13 | Structure des fichiers QuickTime/MP4 (atomes `moov`, `stsd`, `stts`, `stsc`, `stsz`, `stco`, `co64`), connue de mémoire | Formats documentés publiquement (Apple, ISO) | Lecteur de conteneur `crates/media/src/mov.rs` et écrivain de test | Claude Code (I) |
| 2026-10-13 | Formule de conversion « Scaled YCoCg-DXT5 » (van Waveren et Castaño, 2007, cité par la spécification HAP), connue de mémoire | Article public | Conversion HAP Q dans `crates/render/src/convert.wgsl` | Claude Code (I) |
| 2026-10-13 | Outil `ffmpeg` / `ffprobe` (Homebrew), utilisé en boîte noire pour relire les clips de test | Outil local, non lié à Prism | Vérification croisée des fichiers HAP générés | Claude Code (I) |
