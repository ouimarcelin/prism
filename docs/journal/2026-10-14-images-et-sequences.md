# 2026-10-14 – Ticket 8 : images fixes et séquences PNG

- Branche : `feat/images`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : code source de `png`, `zune-jpeg` et `zune-core` (registre Cargo local)

## Prompts donnés

1. « parfait, continues, fais les 5 tickets suivants. » (deuxième des cinq)

## Résultat

- `prism-media` : `FrameSource`, `Frame`, `MediaInfo` (durée et numéro d'image pour une piste de film ou des images régulières), `StillImage`, `ImageSequence` (tri par numéro), décodage PNG (gris, gris + alpha, RVB, RVBA, palette, 16 bits) et JPEG.
- `prism-render` : pixels RGBA envoyés dans une texture `Rgba8Unorm`, convertis par la passe existante (alpha prémultiplié) ; une même texture de couche alterne HAP et pixels.
- `prism-core` : `MediaSource::ImageSequence { path, frame_rate }`, chemins relatifs gérés comme pour les fichiers.
- `prism-engine` : le thread de décodage ouvre n'importe quelle source.
- `prism-ui` : import d'images et de dossiers, réglage de la fréquence, relocalisation des dossiers ; `prism-app` : un dossier passé en ligne de commande devient une séquence.
- 13 tests de plus (media 7, render 1, engine 1, ui 2, core 1, autres), tous verts sur le GPU.
- ADR 0009.

## Points ouverts

- Décodage PNG sur le processeur : une séquence 1080p à 60 images/s n'est pas garantie.
- Choix sans observation : fréquence par défaut des séquences à 30 images/s ; une image fixe dure une seconde de « média » (sans effet visible, elle reste affichée).

## Archive

- Transcription : à archiver avec `scripts/archive-session.sh` en fin de session.
