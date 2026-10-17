# Guide : la sortie avancée

La sortie avancée envoie la composition sur un ou plusieurs projecteurs ou écrans, en la découpant, en la déformant pour épouser une surface, en masquant des zones et en fondant les bords de projecteurs qui se recouvrent.

Ouvrir : menu **Sortie > Sortie avancée…** (fenêtre à part, qui peut rester sur un autre écran pendant le spectacle).

## Les notions

- **Écran** : l'image envoyée à un projecteur ou à un écran, de la taille de celui-ci. Sans écran, chaque sortie montre la composition entière.
- **Découpe** : un rectangle de la composition posé sur un écran. Un écran peut en avoir plusieurs, dessinées dans l'ordre de la liste (la plus haute couvre les autres).
- **« sur »** : l'écran du Mac qui affiche l'écran de la scène. La sortie plein écran ouverte sur cet écran du Mac (menu Sortie, touche `F`) montre alors l'écran de la scène au lieu de la composition entière.

## Régler une découpe

Dans la fenêtre, à gauche la **composition** avec la zone de chaque découpe, à droite l'**écran** avec ses coins.

- Cliquer une forme la choisit.
- Glisser l'intérieur déplace ; glisser un coin redimensionne (à gauche) ou met en perspective (à droite).
- Les flèches du clavier déplacent d'un pixel le dernier point touché à droite (Maj : 10 pixels).
- Le panneau de gauche donne les valeurs en pixels, la rotation et les miroirs de la zone lue, et les boutons « Plein écran » et « Comme l'entrée ».
- Cmd+Z annule, Cmd+Maj+Z rétablit ; ce qui joue continue de jouer.

## Déformer

« Déformation » : **Grille droite** ou **Grille lissée** pose une grille de points sur la découpe ; chaque point se glisse, l'image suit (lissée : des courbes passent par les points). « Cellules » change la finesse de la grille sans changer la forme ; « Réinitialiser la grille » revient aux quatre coins.

## Masquer

Cocher **Masque** : un polygone (rose) sur l'écran ; la découpe ne se montre qu'à l'intérieur, ou qu'à l'extérieur avec « Inversé ». « Bord adouci » fond le contour ; « + Point » ajoute un point ; clic droit sur un point le retire. Le masque reste en place sur l'écran quand la découpe est déformée : il cache une zone de la surface.

## Deux projecteurs qui se recouvrent

1. Créer deux écrans (« + Écran »), de la taille des projecteurs, chacun « sur » son projecteur.
2. Donner à la composition la largeur totale : deux fois la largeur d'un projecteur moins le recouvrement (menu Composition).
3. Régler « recouvrement » (en pixels d'un projecteur), puis cliquer **Répartir** : chaque écran reçoit sa part de la composition et le fondu des bords sur le côté partagé.
4. Cocher **Mire** sur chaque écran pour aligner physiquement les projecteurs : les grilles doivent se superposer dans le recouvrement. Ajuster les coins ou la grille de déformation, au pixel avec les flèches.
5. Décocher la mire, afficher une image unie : la bande de recouvrement ne doit plus se voir. Sinon, ajuster dans « Fondu des bords » la **Courbe**, le **Gamma** (2,2 en général) et, sur une image noire, le **Noir**.

## Tester sans projecteur

Cocher **Simulation** : sous les deux vues, les écrans apparaissent côte à côte, recouverts du « recouvrement », leur lumière additionnée comme sur un mur. Avec « Répartir » et une image unie (générateur « Couleur unie »), la simulation doit être uniforme ; sans fondu, la bande de recouvrement apparaît nettement plus claire. C'est le critère de la phase 4 répété sur l'écran du Mac (le même contrôle est fait par un test automatique).

## Réutiliser une salle

« Exporter… » enregistre les écrans et découpes dans un fichier `.prism-scene` ; « Importer… » les reprend dans un autre projet.
