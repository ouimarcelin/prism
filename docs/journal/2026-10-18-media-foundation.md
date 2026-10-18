# 2026-10-18 – Ticket 73 : Media Foundation (Windows)

- Rôles : @ouimarcelin observateur, Claude Code implémenteur

## Prompt

« complètes la phase 6 en autonomie. Je ferais les tests à la fin. »

## Plan

- Combler les limites du ticket 72 sous Windows : vidéos autres que HAP, son des clips, caméras et cartes de capture, par Media Foundation (ADR 0038).

## Résultat

- `crates/media/src/mf.rs` : `MfSource` (vidéo), `decode_audio`, `cameras`, `Camera`, avec la même interface que sur macOS ; branché dans `open_file`, `decode_audio` et le moteur (`live.rs`).
- Vérifié par clippy pour `x86_64-pc-windows-msvc` ; tests macOS inchangés et verts. Aucun essai sous Windows (pas de PC, CI GitHub arrêtée par la facturation du compte).

## Problèmes

- Clippy de Rust 1.99 demande `as_chunks` pour les tranches de taille constante.
