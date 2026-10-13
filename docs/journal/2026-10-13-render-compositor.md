# 2026-10-13 – Ticket 2 : mixage des couches sur le GPU

- Branche : `feat/render-compositor`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : `docs/SPEC.md`, formules W3C de mémoire, code source de wgpu 26 (voir `docs/SOURCES.md`)

## Prompts donnés

1. « parfait, passes à la prochaine étape. »

## Plan validé

Crate `prism-render` : mixage GPU des couches avec opacité, 4 modes de fusion et crossfader, en WGSL ; tests en rendu hors écran comparés à une référence CPU ; couches en couleurs unies, sans vidéo.

## Résultat

- `Gpu::headless` ouvre le GPU sans fenêtre ; `solid_color_texture` et `read_texture` servent aux tests et serviront aux vignettes.
- `Compositor` : deux textures `Rgba16Float` en alternance, une passe par couche, paramètres par couche dans un tampon uniforme à décalage dynamique.
- `plan_layers` traduit l'état de la composition (clip en cours, bypass, solo, crossfader) en liste de couches à dessiner.
- `reference::composite` : mêmes formules sur le CPU.
- 16 nouveaux tests : 5 sur la référence, 3 sur le plan, 8 sur le GPU (tous exécutés sur le GPU de la machine, aucun sauté). Le message du commit `a1f2056` annonce 21 tests par erreur de comptage.
- ADR 0004 : mixage dans l'espace d'affichage, alpha prémultiplié, 16 bits flottants.

## Problèmes et points ouverts

- `cargo add wgpu` a choisi wgpu 26 au lieu de 30 : la version minimale de Rust déclarée (1.85) écarte les versions récentes. Montée de version à prévoir dans un ticket dédié.
- Réseau lent vers crates.io (« transfer too slow ») : relancer `cargo fetch` avec `CARGO_HTTP_TIMEOUT=120` a suffi.
- Sur la CI macOS de GitHub, les tests GPU s'exécutent si un GPU est disponible, sinon ils affichent un message et passent.

- CI : `cargo-deny` a échoué sur l'avertissement RUSTSEC-2024-0436 (`paste` n'est plus maintenu), dépendance indirecte de wgpu 26. Ignoré avec justification dans `deny.toml` (branche `fix/deny-paste`).

## Archive

- Transcription : à archiver avec `scripts/archive-session.sh` en fin de session.
