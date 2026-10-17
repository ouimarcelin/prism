# 0027 – Scène de sortie : écrans et découpes

- Date : 2026-10-17
- Statut : Acceptée

## Contexte

La phase 4 demande une sortie avancée : écrans virtuels, découpes (slices) de la composition, déformation, masques, edge blending. Aucune observation écrite ne décrit le flux de travail d'origine : la conception ci-dessous est propre à Prism, à partir des notions générales du mapping vidéo.

## Décision

- **Modèle** (`prism-core`, `Stage` dans la composition, enregistré dans le projet) : des écrans (nom, taille en pixels, écran du système qui l'affiche) ; sur chaque écran, des découpes dessinées dans l'ordre. Une découpe prend un rectangle de la composition (`Area`) et le pose par ses quatre coins (`Quad`) sur l'écran. Positions en fractions, depuis le coin haut gauche : elles survivent à un changement de taille. Identifiants uniques dans la scène, vérifiés au chargement.
- **Sans écran**, rien ne change : chaque sortie montre la composition entière.
- **Rendu** (`prism-render`, `StageRenderer`) : après l'image programme, dans la même soumission, chaque écran est dessiné dans sa propre image (format de travail, 16 bits flottants) : fond noir, puis ses découpes actives. Un quadrilatère quelconque est rendu en perspective par la méthode des poids des diagonales (coordonnées multipliées par q, divisées après interpolation), sans couture entre les deux triangles. Un seul tampon de sommets pour tous les écrans d'une image, agrandi seulement si besoin.
- **Sorties** : `Output::present_images` reçoit le programme et les écrans ; une fenêtre de sortie montre l'écran de la scène réglé sur son écran système, sinon le programme. Syphon et NDI publient toujours le programme.
- La déformation par grille et par courbes, les masques et l'edge blending s'ajouteront aux découpes (tickets suivants), en remplaçant les deux triangles par un maillage.

## Conséquences

- Chaque écran coûte une passe de rendu par image, même s'il n'est affiché nulle part ; un écran inutile se supprime.
- Un écran de la scène n'a pas de lien avec la taille réelle de l'écran système : l'image est ajustée avec bandes noires si elles diffèrent.
- **Ajout du 2026-10-17 (ticket 45) : transformations.** Une découpe peut tourner la zone qu'elle lit autour de son centre (angle calculé en pixels de la composition) et la retourner ; un écran peut être retourné horizontalement et verticalement, masques compris. Les sommets portent la position dans la découpe, que le fondu des bords et la mire suivent.
