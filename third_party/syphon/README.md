# Syphon (code tiers)

Sous-ensemble du framework Syphon (serveur et client de base, client Metal, messagerie), repris sans modification depuis https://github.com/Syphon/Syphon-Framework, commit `f4761677a45b8034a3c2069ec0f3d2553da81fba` (21 septembre 2026).

- Licence : BSD 3 clauses, texte complet dans `License.txt` ; attribution dans `NOTICE` à la racine du dépôt.
- Fichiers non repris : rendu et client OpenGL, serveur Metal et ses shaders (leur compilation demande Xcode complet), projet Xcode, documentation.
- Compilés par `crates/io-video/build.rs` ; la glue de Prism (`crates/io-video/src/syphon_glue.m`) est sous la licence de Prism.
