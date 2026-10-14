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
| 2026-10-14 | Code source de wgpu 30, egui 0.36, egui-winit 0.36, egui-wgpu 0.36, epaint 0.36 et winit 0.30 (registre Cargo local) ; API de crates.io pour les versions | MIT / Apache 2.0 (polices d'egui : voir `NOTICE`) | Montée de version, surfaces, intégration de l'interface | Claude Code (I) |
| 2026-10-14 | Code source des crates `png` 0.18, `zune-jpeg` 0.5 et `zune-core` 0.5 (registre Cargo local) | MIT / Apache 2.0 / Zlib | Décodage des images fixes et des séquences | Claude Code (I) |
| 2026-10-14 | Image de test `crates/media/tests/fixtures/halves-16x8.jpg` : PNG synthétique écrit par nos soins (deux couleurs unies), converti en JPEG par l'outil `sips` de macOS | Création du projet | Test du décodage JPEG | Claude Code (I) |
| 2026-10-14 | Code source des crates `objc2` 0.6, `objc2-av-foundation`, `objc2-core-media`, `objc2-core-video`, `objc2-core-foundation`, `objc2-foundation` 0.3 (en-têtes Apple traduits, registre Cargo local) | MIT / Apache 2.0 / Zlib | Décodeur AVFoundation `crates/media/src/avf.rs` | Claude Code (I) |
| 2026-10-14 | Clips de test `crates/media/tests/fixtures/rgb-thirds-*` et `red-half-alpha-prores4444.mov` : images synthétiques écrites par nos soins (couleurs unies), encodées par l'outil `ffmpeg` (libx264, prores_ks) | Création du projet | Tests H.264 et ProRes | Claude Code (I) |
| 2026-10-14 | Motif de test `testsrc2` de `ffmpeg`, encodé en H.264 1080p60 pour une mesure locale, non versionné | Outil local | Mesure de performance H.264 | Claude Code (I) |
| 2026-10-14 | Code source de `wgpu-core` 30, `wgpu-hal` 30 (backend Metal) et `wgpu::util::StagingBelt` (registre Cargo local) | MIT / Apache 2.0 | Cause des allocations à chaque image, tampons d'envoi réutilisés (ADR 0012) | Claude Code (I) |
| 2026-10-14 | Code source de la crate `libc` 0.2.190 (`thread_policy_set`, `thread_time_constraint_policy`, `pthread_mach_thread_np`) ; API Mach d'ordonnancement temps réel de macOS (`THREAD_TIME_CONSTRAINT_POLICY`, `mach_timebase_info`), connue de mémoire | MIT / Apache 2.0 ; API système publique | Thread de rendu temps réel (ADR 0012) | Claude Code (I) |
| 2026-10-14 | Outils macOS `ps`, `vm_stat` et `sysctl`, utilisés en boîte noire | Outils système | Priorité des threads et pagination de la machine pendant les mesures | Claude Code (I) |
| 2026-10-14 | Licence d'Ableton Link (GPL v2 ou plus, ou licence commerciale d'Ableton), connue de mémoire | Fait public sur une bibliothèque tierce | Link écarté pour la synchro réseau du tempo (ADR 0014) | Claude Code (I) |
| 2026-10-14 | Formules courantes de traitement d'image : luminance BT.709, rotation de teinte autour de l'axe des gris, flou gaussien séparable, repli polaire d'un kaléidoscope, rotation et mise à l'échelle autour du centre ; connues de mémoire | Savoir général | Shaders des effets de `crates/fx` (ADR 0015) | Claude Code (I) |
