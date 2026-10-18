# 2026-10-18 – Ticket 72 : portage Windows

- Rôles : @ouimarcelin observateur, Claude Code implémenteur

## Prompt

« complètes la phase 6 en autonomie. Je ferais les tests à la fin. »

## Plan

- Faire compiler l'espace de travail pour Windows et le vérifier sans PC : tâche `rust (Windows)` de la CI, puis vérification locale (`--target x86_64-pc-windows-msvc`).
- Thread de rendu : priorité et minuteur Windows.
- Spout en sortie et en entrée ; NDI sous Windows.
- Limites écrites dans l'ADR 0037.

## Résultat

- Premier passage de la CI Windows (avant les changements) : une seule erreur, un champ inutilisé hors macOS.
- La CI ne démarre plus ensuite : « recent account payments have failed or your spending limit needs to be increased » (facturation GitHub du compte). Vérification faite en local avec la cible Windows : clippy sans avertissement, avec et sans NDI. Ni édition des liens ni tests sous Windows.
- `readback.rs` : lecture de l'image programme sur le processeur, partagée par NDI et Spout.
- `spout.rs` : émetteur et récepteur ; tests de la liste des noms et de la taille de `SharedTextureInfo` ; test d'enregistrement d'un émetteur (sauté sans GPU Direct3D 11).
- `MediaSource::Spout`, menu « Spout… », libellés Spout sous Windows, écran gardé allumé.

## Problèmes

- Disque plein pendant le ticket (0 octet libre) : cache incrémental de compilation (`target/debug/incremental`, 11 Go) effacé ; 12 Go libres ensuite.
- API du crate `windows` 0.62 : `Error::from_win32` n'existe plus (`from_thread`) ; clippy demande `as_chunks` pour des tranches de taille constante.
- Vidéos autres que HAP et caméras sous Windows : laissées pour un ticket Media Foundation.
