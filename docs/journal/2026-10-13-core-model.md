# 2026-10-13 – Ticket 1 : modèle de données et sauvegarde JSON

- Branche : `feat/core-model`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : `docs/SPEC.md` uniquement (aucune nouvelle source)

## Prompts donnés

1. « on oublies github pour le moment ça ne pose pas de problème ? je veux que tu te concentre sur le création du logiciel. »
2. « vas y. »
3. « A » (choix de la lecture vidéo native, ADR 0003)

## Plan validé

Workspace Cargo, crate `prism-core` : composition, couches, colonnes, clips, crossfader, déclenchement ; fichier projet JSON versionné avec chemins relatifs, écriture atomique et détection des médias manquants ; tests ; ADR 0002.

## Résultat

- Workspace Cargo (édition 2024, `rust-toolchain.toml` sur stable), crate `crates/core` publiée sous le nom `prism-core` (le nom `core` est réservé par Rust).
- Modèle : `Composition`, `Layer`, `Column`, `Clip`, `Playback`, `Crossfader` (courbes plein-centre, linéaire, puissance constante), identifiants stables, grille toujours rectangulaire.
- Déclenchement : un clip sur sa couche ; une colonne entière comme une scène (les couches vides dans cette colonne s'arrêtent).
- `effective_opacity` combine opacité, bypass, solo et crossfader : prêt pour le moteur de rendu.
- Projet `.prism` : voir ADR 0002.
- 34 tests (26 unitaires, 8 d'intégration sur disque), clippy sans avertissement, compilation release.
- ADR 0003 : lecture vidéo native choisie.

## Problèmes et points ouverts

- `cargo-deny` n'a pas pu être installé en local (« transfer too slow ») ; il tournera dans la CI.
- Choix faits sans observation, à confirmer à l'usage : déclencher une case vide ne fait rien ; déclencher une colonne arrête les couches vides dans cette colonne ; courbe du crossfader par défaut « plein-centre ».

## Archive

- Transcription : à archiver avec `scripts/archive-session.sh` en fin de session.
