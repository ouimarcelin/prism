# 0029 – Masques des découpes

- Date : 2026-10-17
- Statut : Acceptée

## Contexte

Le cahier des charges demande des masques en sortie avancée : cacher une partie d'une découpe (un obstacle, le bord d'un objet). Conception propre à Prism.

## Options

1. **Polygone sur l'écran, couverture calculée par pixel** dans le shader des découpes : distance au bord pour l'adoucissement, sans image intermédiaire.
2. **Image de masque** dessinée à part (triangulation du polygone, flou pour l'adoucissement) : plus souple pour des formes courbes, mais une passe et une texture de plus par découpe.

## Décision

Option 1 :

- un masque par découpe, polygone de 3 à 64 points en fractions de l'écran (il ne bouge pas avec la déformation : il cache une zone de la surface) ;
- la découpe se montre à l'intérieur, ou à l'extérieur si le masque est inversé ; bord adouci en pixels, centré sur le contour, au moins un pixel pour éviter l'escalier ;
- dans le shader : test pair-impair (le polygone peut se croiser), distance au côté le plus proche ; tous les masques d'une image dans deux tampons de stockage, l'index passé par l'instance du dessin ;
- les découpes sont désormais fondues sur ce qui est dessous (alpha prémultiplié) au lieu de le remplacer.

## Conséquences

- Coût par pixel proportionnel au nombre de points du masque : 64 points restent légers en 1080p.
- Pas de courbes dans le contour : un arrondi demande plus de points.
- **Ajout du 2026-10-17 (ticket 47) : image de couverture.** Mesuré à 24 ms par image pour deux écrans 1080p avec un masque de 64 points, le calcul par pixel à chaque image est remplacé par une image de couverture (8 bits) dessinée seulement quand le masque change ; la découpe la lit. Coût : 1,2 ms (`docs/benchmarks/2026-10-17-sortie-avancee.md`).
