# Banc de la sortie avancée – 2026-10-17

- Machine : MacBook Air M1, macOS, GPU intégré (Metal via wgpu 30)
- Outil : `cargo run --release -p prism-render --example stage-bench`
- Mesure : deux écrans 1080p dessinés depuis une image programme 1080p, 300 images d'affilée, deux en vol ; temps par image une fois le GPU saturé.

| Deux écrans 1080p | Avant | Après |
| --- | --- | --- |
| Découpes droites | 1,13 | 1,14 |
| Perspective | 0,99 | 0,99 |
| Grille lissée 8 × 8 | 1,17 | 1,12 |
| Grille lissée 32 × 32 | 23,54 | 1,46 |
| Masque de 64 points, bord de 40 px | 23,97 | 1,16 |
| Fondu des bords 20 % | 1,12 | 1,13 |

(ms par image)

## Changements

- Masque : la couverture est dessinée une fois dans une image de l'écran, quand le masque, la taille de l'écran ou son miroir changent ; à chaque image, une simple lecture. Avant : 64 côtés parcourus pour chaque pixel à chaque image.
- Grille : les sommets d'une découpe restent sur le GPU tant qu'elle ne change pas (avant : 22 Mo envoyés à chaque image pour 32 × 32) ; la courbe lissée n'évalue plus que les quatre rangées utiles.

## Conclusion

La sortie avancée coûte environ 1 ms par image pour deux écrans, quel que soit le réglage : elle tient à 60 images/s avec la marge du moteur (environ 2,3 ms de travail par image avec 4 couches HAP 1080p, ADR 0012). Pendant un réglage, une image où un masque change coûte quelques millisecondes de plus.
