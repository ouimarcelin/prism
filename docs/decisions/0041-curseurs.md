# 0041 – Curseurs

- Date : 2026-10-18
- Statut : Acceptée

## Contexte

Avec le thème de l'ADR 0040, les curseurs d'egui gardaient une poignée grise cerclée de blanc et un rail fin : egui peint la poignée avec la couleur de fond des cases à cocher. @ouimarcelin a fourni une maquette : piste épaisse en pilule, remplissage bleu jusqu'à la poignée, poignée blanche ronde avec un point bleu et une ombre ; et, pour un réglage centré, un remplissage qui part d'un point d'origine marqué. Il demande aussi des « points magnétisés » (par exemple 25 %, 50 %) quand c'est pertinent.

## Décision

- **Un curseur à nous**, `prism-ui::slider::Slider`, remplace `egui::Slider` partout (21 curseurs). Même usage : `Slider::new(&mut valeur, min..=max)`, case de valeur après la piste (`show_value`), `suffix`, `max_decimals`, `logarithmic`, `text`.
- **Dessin** : piste de 16 points (`#383838`, contour `#444444`), remplissage `#0c8ce9`, poignée blanche de 16 points avec ombre et point bleu ; la poignée grossit au survol, le point pendant le glissement ; anneau bleu clair quand le curseur a le focus clavier.
- **Origine** : sur une plage autour de zéro (rotation, réglages d'effet signés), le remplissage part de zéro ; `origin(v)` le fait partir d'ailleurs (crossfader : 50 %).
- **Points magnétiques** : marqués d'un point sur la piste (bleu foncé sur le remplissage, gris ailleurs). La poignée y saute quand le pointeur passe à moins de 6 points ; Alt enfoncé, pas d'aimantation. Placés :
  - 25, 50, 75 % : opacités de couche, volumes, curseurs du tableau de bord, luminosité des luminaires ;
  - valeur par défaut : réglages d'effet et de générateur, bornes des animations ;
  - origine : crossfader (milieu), plages autour de zéro (zéro) ;
  - valeurs d'usage : vitesse 1 ×, rotation ±90°, gamma 2,2, courbe du fondu 2.
- **Clavier** : flèches gauche et droite quand le curseur a le focus (1 % de la plage, ou 1 pour un entier).

## Conséquences

- L'apparence des curseurs se règle dans `slider.rs`, qui prend ses couleurs dans `theme::color`.
- Les fonctions d'egui non reprises (curseur vertical, formatage personnalisé, pas imposé) ne servent nulle part dans Prism ; à ajouter si un besoin vient.
- Les points magnétiques sont dans l'interface seulement : MIDI, OSC, DMX et l'API réseau posent les valeurs telles qu'elles arrivent.

## Révision (ticket 87)

Nouvelle maquette de @ouimarcelin : la poignée ronde disparaît. Piste aux coins arrondis de 3 points (plus de pilule), remplissage bleu aux mêmes coins depuis l'origine jusqu'à une demi-piste après la valeur, point blanc translucide (rayon 2 points) qui marque la valeur dans le bout du remplissage, capsule blanche verticale (6 × 22 points, intérieur bleu, ombre légère) sur l'origine : bord gauche, ou zéro et milieu pour les réglages centrés. Les points magnétiques ne s'affichent plus que sur la partie vide de la piste, pour que le point de valeur reste seul dans le remplissage. Le point grossit au survol et pendant le glissement ; le focus clavier entoure la piste.
