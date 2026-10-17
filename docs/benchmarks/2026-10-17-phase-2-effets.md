# 2026-10-17 – Phase 2 : temps GPU des effets en 1080p

- Machine : MacBook Pro M1, 8 Go, macOS (même machine que la phase 1)
- Commande : `cargo run --release -p prism-fx --example fx-bench` (300 images par effet, lancé par Claude Code : priorité réduite, chiffres un peu pessimistes)
- Version : branche `feat/27-fx-bench`, après réécriture du flou (7 lectures par passe au lieu de 13 ; 2,16 ms avant)

| Effet | Passes | ms par image (GPU) |
| --- | --- | --- |
| Copie (inversion à 0) | 1 | 0.19 |
| Couleur | 1 | 0.18 |
| Inversion | 1 | 0.17 |
| Transformation | 1 | 0.18 |
| Miroir | 1 | 0.17 |
| Kaléidoscope | 1 | 0.16 |
| Flou | 2 | 0.94 |
| Pixellisation | 1 | 0.16 |
| Postérisation | 1 | 0.17 |
| Seuil | 1 | 0.16 |
| Contours | 1 | 0.35 |
| Décalage RVB | 1 | 0.18 |
| Glitch | 1 | 0.17 |
| Vague | 1 | 0.17 |
| Tourbillon | 1 | 0.17 |
| Mosaïque | 1 | 0.30 |
| Vignette | 1 | 0.18 |
| Stroboscope | 1 | 0.17 |
| Colorisation | 1 | 0.17 |
| Couleur unie | 1 | 0.15 |
| Dégradé | 1 | 0.15 |
| Lignes | 1 | 0.16 |
| Cercles | 1 | 0.16 |
| Damier | 1 | 0.16 |
| Bruit | 1 | 0.37 |
| Texte | 1 | 0.15 |

## Lecture

- Un effet d'une passe coûte environ 0,16 ms en 1080p, presque autant qu'une copie : la bande passante mémoire domine, pas le calcul.
- Le flou (deux passes) coûte 0,9 ms ; contours, mosaïque et bruit un peu plus qu'une copie.
- Avec 4 couches HAP 1080p (2,2 ms de GPU, voir `2026-10-14-phase-1.md`), une chaîne de 4 ou 5 effets par couche et quelques effets de composition restent sous 10 ms : la marge pour 60 images/s est large sur ce M1.
