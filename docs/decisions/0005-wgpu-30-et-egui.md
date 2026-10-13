# 0005 – wgpu 30, egui 0.36 et Rust 1.95

- Date : 2026-10-14
- Statut : Acceptée

## Contexte

Le moteur tournait sur wgpu 26, retenu parce que la version minimale de Rust déclarée (1.85) écartait les versions récentes (ADR 0004). L'interface de la phase 1 utilisera egui, dont l'intégration GPU (`egui-wgpu`) doit partager le même périphérique wgpu que le moteur, pour afficher l'aperçu sans copie. Or aucune version d'`egui-wgpu` ne cible wgpu 26 : les versions existantes ciblent wgpu 25 (egui 0.32), 27 (egui 0.33), 29 (egui 0.34 et 0.35) et 30 (egui 0.36).

## Options

1. **Redescendre à wgpu 25 et egui 0.32.** Aucune hausse de la version minimale de Rust, mais des bibliothèques vieilles d'un an dès le départ, et `paste` (non maintenu) toujours présent.
2. **Monter à wgpu 30 et egui 0.36**, les versions actuelles. Demande Rust 1.95 au minimum ; la chaîne d'outils du projet suit déjà la version stable (1.99).

## Décision

Option 2 : wgpu 30, egui 0.36 et winit 0.30 pour l'interface, `rust-version = "1.95"`.

## Conséquences

- Adaptations de l'API wgpu : `Instance::new` prend le descripteur par valeur, `PollType::wait_indefinitely()`, `multiview_mask`, `immediate_size`, lecture des tampons mappés faillible.
- `paste` disparaît des dépendances : l'exception RUSTSEC-2024-0436 est retirée de `deny.toml`.
- Les nouvelles règles de clippy liées à Rust 1.88+ s'appliquent (`as_chunks`).
- Les montées de version suivantes de wgpu et egui se feront ensemble.
