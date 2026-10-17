# 0030 – Fondu des bords et mire

- Date : 2026-10-17
- Statut : Acceptée

## Contexte

Le critère de la phase 4 : deux projecteurs dont la zone de recouvrement est invisible. Dans cette zone, la lumière des deux projecteurs s'additionne : chacun doit s'assombrir vers son bord pour que la somme reste égale au reste de l'image. Réglage demandé par le cahier des charges : largeur et gamma.

## Décision

- Fondu par découpe, de chaque côté (gauche, droite, haut, bas), largeur en fraction de la découpe ; les côtés se multiplient dans les coins.
- Courbe en lumière : `0,5 (2t)^p` sur la première moitié, `1 − 0,5 (2(1−t))^p` ensuite, de sorte que `f(t) + f(1 − t) = 1` ; `p` (courbe, 2 par défaut) garde plus de lumière au milieu. La valeur de pixel est `f^(1/γ)` (γ des projecteurs, 2,2 par défaut) : les deux projecteurs s'additionnent en lumière, pas en valeurs de pixels.
- Mire par écran : les découpes montrent une grille de 8 × 8, une croix au centre et une bordure, teintées par découpe, à la place de la composition ; le fondu et le masque s'appliquent aussi, pour régler le recouvrement sur la mire.
- Le shader reçoit pour chaque découpe un bloc de 64 octets (masque, zone de la composition, fondu, mire) dans un tampon de stockage, nommé par l'instance du dessin.

## Conséquences

- Pas de réglage du niveau de noir (le noir d'un projecteur n'est jamais nul : la zone de recouvrement reste plus claire dans les noirs) : à ajouter si le test réel le demande.
- Le fondu suit la découpe déformée : il épouse la grille.
