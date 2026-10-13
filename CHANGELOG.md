# Changelog

Toutes les modifications notables de Prism sont listées ici.
Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/). Numérotation `0.x` pendant le développement (voir `docs/SPEC.md`, section « Versions »).

## [Non publié]

### Ajouté

- Crate `prism-render` : mixage des couches sur le GPU (wgpu, Metal), opacité, modes normal, add, multiply et screen, crossfader appliqué via la composition ; images intermédiaires en 16 bits flottants ; tests GPU comparés à une référence CPU (ADR 0004).
- Workspace Cargo et crate `prism-core` : composition (résolution, fréquence), grille couches × colonnes, crossfader A/B à trois courbes, réglages de couche (opacité, mode de fusion, solo, bypass, transition), réglages de lecture des clips, déclenchement d'un clip ou d'une colonne.
- Fichier projet `.prism` en JSON versionné : chemins relatifs au dossier du projet, écriture atomique, détection des médias manquants (ADR 0002).
- ADR 0003 : lecture vidéo native (HAP en Rust, VideoToolbox pour H.264), sans FFmpeg.
- Mise en place du dépôt : cahier des charges (`docs/SPEC.md`), règles pour Claude Code (`CLAUDE.md`), double licence MIT / Apache 2.0, `NOTICE`, `README.md`.
- Politique et attestation clean room (`docs/CLEAN_ROOM.md`), registre des sources (`docs/SOURCES.md`), dossiers `docs/observations/`, `docs/decisions/` et `docs/journal/`.
- Guide de contribution avec DCO, modèles de pull request et de tickets.
- CI GitHub Actions : contrôle clean room, vérification DCO, format, clippy, tests et compilation release sur macOS, `cargo-deny`.
