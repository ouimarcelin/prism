# 2026-10-13 – Phase 0 : mise en place du dépôt et de la clean room

- Ticket : aucun (mise en place préalable aux tickets)
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur

## Prompts donnés

1. « c'est le début du projet, par quoi commençons nous ? »
2. « comment obtenir la traçabilité clean room ? »
3. « créé le github et la certif clean room. »

## Plan validé

Phase 0 avant tout code : squelette du dépôt, politique clean room, CI, puis dépôt GitHub privé avec étiquettes et jalons.

## Résultat

- Cahier des charges déplacé dans `docs/SPEC.md` (premier commit, seul) ; `CLAUDE.md` réécrit en version courte.
- Licences MIT et Apache 2.0, `NOTICE`, `README.md` avec mention de non-affiliation, `CHANGELOG.md`, `CONTRIBUTING.md`.
- `docs/CLEAN_ROOM.md` (politique, preuves, attestation), `docs/SOURCES.md`, `docs/observations/`, ADR 0001.
- `scripts/check-clean-room.sh`, `scripts/archive-session.sh`, `deny.toml`.
- CI : `clean-room`, `dco`, puis `rust (macOS)` et `cargo-deny` dès que le workspace Cargo existe.
- Modèles de pull request et de tickets.

## Problèmes et points ouverts

- Rust et FFmpeg pas encore installés sur la machine : la CI Rust est conditionnelle.
- Pas de clé de signature git : commits non signés pour l'instant.
- Le manuel PDF présent dans le dossier est exclu du dépôt et n'a pas été ouvert par Claude Code.

## Archive

- Transcription : à archiver avec `scripts/archive-session.sh` en fin de session.
