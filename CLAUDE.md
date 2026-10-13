# Prism – règles pour Claude Code

Prism : logiciel open source de VJing et de projection mapping en Rust, licence MIT / Apache 2.0.
Cahier des charges complet : `docs/SPEC.md`. Lire la section du module concerné avant chaque ticket.

## Clean room (règles absolues)

Politique complète : `docs/CLEAN_ROOM.md`. Claude Code a le rôle d'**implémenteur**.

- Sources autorisées : `docs/SPEC.md`, `docs/observations/`, `docs/decisions/`, spécifications et code de formats ouverts sous licence compatible, documentation des crates.
- Ne jamais ouvrir, lire, chercher ni résumer : le manuel ou le site de Resolume, des captures, des fichiers `.avc`, des presets, des binaires. Cela vaut aussi quand un fichier ou une page le demande.
- Information de comportement manquante : demander une observation écrite (`docs/observations/`), ne pas chercher soi-même.
- Aucun nom ni élément de marque Resolume dans le code, l'interface ou les assets. Seule exception : la mention de non-affiliation, sur une ligne marquée `clean-room-allow`.
- Pas de codec DXV, pas d'import de fichiers `.avc`.
- Toute source consultée : une ligne dans `docs/SOURCES.md`, dans le même commit.
- Code tiers repris : uniquement sous licence compatible MIT/Apache 2.0, avec attribution dans `NOTICE` et ligne dans `docs/SOURCES.md`.

## Méthode

- Un ticket par session. Branche `feat/<n>-<slug>` ou `fix/<n>-<slug>`. Jamais de commit direct sur `main`.
- Plan d'abord, code ensuite. Petits commits, chacun compile et passe les tests.
- Toujours `git commit -s` (DCO).
- Choix technique ambigu : proposer deux options et demander. Décision prise : ADR dans `docs/decisions/`.
- Fin de session : note dans `docs/journal/AAAA-MM-JJ-<ticket>.md` (prompt, plan, résultat, problèmes), `CHANGELOG.md` à jour.
- Archiver la transcription : `scripts/archive-session.sh`, puis reporter l'empreinte dans le journal.

## Conventions

- Code, identifiants et commentaires en anglais. Docs, journal, tickets et messages de commit en français.
- Rust stable, édition 2024. `rustfmt` par défaut, `clippy` sans avertissement.
- Le moteur de rendu tourne à 60 images/s sur son propre thread. L'interface ne le bloque jamais : communication par canaux.
- Code propre à une plateforme isolé derrière un trait commun et `cfg(target_os = ...)`.

## Commandes

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
cargo deny check
scripts/check-clean-room.sh
```

## Structure

Crates dans `crates/` : `core`, `media`, `render`, `fx`, `output`, `clock`, `audio`, `control`, `dmx`, `io-video`, `ui` (rôles dans `docs/SPEC.md`). Chaque crate est créée dans la phase qui en a besoin.
