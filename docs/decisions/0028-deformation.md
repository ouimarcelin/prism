# 0028 – Déformation des découpes par grille

- Date : 2026-10-17
- Statut : Acceptée

## Contexte

Le cahier des charges demande une « déformation par grille et Bézier » des découpes, pour épouser une surface qui n'est pas plane ou corriger un projecteur. Conception propre à Prism, sans observation du logiciel d'origine.

## Options

1. **Grille de points d'interpolation** : l'utilisateur déplace les points d'une grille ; entre eux, l'image suit des droites ou des courbes qui passent par les points.
2. **Carreaux de Bézier avec poignées de tangente** : plus de contrôle, mais deux fois plus de poignées à régler et des courbes qui ne passent pas par toutes les poignées.

## Décision

Option 1 :

- `Warp` dans la découpe : 1 à 32 cellules par côté, points en fractions de l'écran ; créée depuis les quatre coins en perspective, la découpe ne bouge pas à l'activation ;
- deux tracés : droit (bilinéaire par cellule) ou lissé, par des courbes de Catmull-Rom (des courbes de Bézier dont les points de contrôle se déduisent des voisins) qui passent par chaque point ; aux bords, la grille est prolongée en ligne droite ;
- changer le nombre de cellules reprend la forme courante ; « Réinitialiser » revient aux quatre coins ;
- rendu : maillage fin (4 pas par cellule, de 32 à 256 pas par côté), image interpolée linéairement dans chaque petit triangle ; la grille remplace les quatre coins tant qu'elle existe.

## Conséquences

- Pas de poignées de tangente : une courbe plus serrée demande plus de cellules.
- Une grille de 32 × 32 cellules dessine jusqu'à 400 000 sommets par image : à réserver aux surfaces qui l'exigent.
