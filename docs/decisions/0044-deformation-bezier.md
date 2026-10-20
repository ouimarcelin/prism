# 0044 – Déformation par Bézier à poignées

- Date : 2026-10-20
- Statut : Acceptée

## Contexte

Le cahier des charges demande une « déformation par grille et Bézier ». L'ADR 0028 avait retenu une grille de points, droite ou lissée (Catmull-Rom), sans poignées de tangente. @ouimarcelin demande maintenant des poignées réglables à la main. Conception propre à Prism.

## Décision

- **Troisième tracé** `WarpCurve::Bezier` : chaque cellule de la grille est un carreau de Bézier bicubique. Ses 16 points de contrôle viennent des quatre points de la cellule et de leurs poignées ; les quatre points intérieurs sont posés sans torsion (point + poignée horizontale + poignée verticale).
- **Poignées** (`Handles`) : par point, un décalage vers la colonne suivante (`across`) et un vers la ligne suivante (`down`) ; les poignées vers la colonne et la ligne précédentes sont leurs opposées, ce qui garde la surface lisse au passage du point.
- **Passage à Bézier** : `ensure_handles` prend les poignées sur la forme dessinée jusque-là (tiers de la tangente sur une cellule) : l'image ne saute pas. Sans poignées pour chaque point (fichier incomplet), le tracé lissé les remplace.
- **Changer le nombre de cellules** reprend la forme : nouveaux points sur la surface, poignées prises sur sa tangente. Les points tombent exactement sur l'ancienne forme ; entre eux, l'écart reste faible (pas de torsion).
- **Éditeur** (fenêtre Sortie avancée) : choix « Bézier » dans « Déformation » ; le point choisi montre ses deux paires de poignées (carrés jaunes) ; tirer une extrémité tourne et allonge la paire.
- Rendu inchangé : le maillage fin de l'ADR 0028 suit `Warp::evaluate`.

## Conséquences

- Les projets existants ne changent pas (`handles` omis tant qu'il est vide).
- Pas de poignées indépendantes de part et d'autre d'un point (cassure volontaire) ni de réglage de la torsion des carreaux.
