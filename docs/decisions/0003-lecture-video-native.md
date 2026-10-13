# 0003 – Lecture vidéo native, sans FFmpeg

- Date : 2026-10-13
- Statut : Acceptée

## Contexte

La phase 1 doit lire du HAP et du H.264, avec un critère exigeant : 4 clips HAP 1080p mixés à 60 images/s pendant 30 minutes. Le scratch et la lecture aléatoire demandent un accès immédiat à n'importe quelle image. Le projet est sous double licence MIT / Apache 2.0 et sera diffusé en application macOS signée.

## Options

1. **Lecture native.** HAP lu en Rust : lecture du conteneur QuickTime/MP4, décompression Snappy, envoi direct au GPU des textures compressées BC (DXT). H.264 et ProRes décodés par VideoToolbox, le décodeur matériel d'Apple. Windows recevra plus tard sa propre implémentation (Media Foundation) derrière la même interface.
2. **FFmpeg pour tous les formats**, via la crate `ffmpeg-next`. Rapide à brancher et multiplateforme, mais :
   - le FFmpeg de Homebrew est compilé en GPL (`--enable-gpl`, `libx264`) : diffuser l'application avec obligerait à la passer en GPL ou à maintenir une compilation LGPL ;
   - FFmpeg décode le HAP en RGBA sur le processeur, ce qui annule l'intérêt du format et met en danger le critère 4 × 1080p60 ;
   - les liaisons Rust suivent les versions de FFmpeg avec retard.

## Décision

Option 1, choisie par le mainteneur.

- Interface commune `VideoDecoder` dans la crate `media`, avec une implémentation par format ou par plateforme.
- HAP, HAP Alpha et HAP Q : lecteur de conteneur maison qui indexe la table des échantillons, décompression avec une crate Snappy sous licence compatible, textures BC1 / BC3 envoyées telles quelles au GPU ; conversion YCoCg de HAP Q dans un shader.
- H.264 et ProRes : VideoToolbox sur macOS, isolé derrière `cfg(target_os = "macos")`.
- FFmpeg n'est jamais lié à Prism. La commande `ffmpeg` peut servir d'outil de développement pour générer des fichiers de test.

## Conséquences

- La spécification publique du format HAP sera consultée au ticket HAP et inscrite dans `docs/SOURCES.md`.
- La prise en charge des textures BC par le GPU (fonction `TEXTURE_COMPRESSION_BC` de wgpu) est vérifiée au démarrage ; sans elle, un repli décompresse sur le processeur.
- Le portage Windows demandera un décodeur H.264 dédié.
- Aucune dépendance GPL dans l'application diffusée.
