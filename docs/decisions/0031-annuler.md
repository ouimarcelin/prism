# 0031 – Annuler et rétablir

- Date : 2026-10-17
- Statut : Acceptée

## Contexte

Régler un mapping, une chaîne d'effets ou une grille demande de pouvoir revenir en arrière. En spectacle, annuler ne doit jamais arrêter ni relancer un clip.

## Options

1. **Instantanés de la composition sans la lecture** : à chaque modification, l'état précédent est gardé ; annuler le remet en place en y reportant ce qui joue.
2. **Commandes inversibles** : chaque modification décrite comme une commande avec son inverse. Plus économe en mémoire, mais chaque endroit de l'interface qui modifie la composition devrait passer par une commande.

## Décision

Option 1, dans `Document` :

- la lecture (clip joué et compteur de déclenchement de chaque couche, position du crossfader) est retirée des instantanés et conservée à l'annulation ;
- les modifications espacées de moins de 400 ms forment une seule étape (un glissé, un curseur) ;
- les changements venus des contrôleurs (MIDI, OSC, touches apprises) ne créent pas d'étape : `commit_live` ;
- 200 étapes au plus ; Cmd+Z et Cmd+Maj+Z dans les deux fenêtres, menu Édition ; les champs de texte gardent ces touches pour leur texte.

## Conséquences

- Mémoire : une composition est petite (quelques dizaines de kilo-octets) ; 200 étapes restent de l'ordre du mégaoctet.
- Une annulation après un passage au contrôleur remet aussi les valeurs déplacées au contrôleur entre-temps (opacités, réglages) à leur valeur de l'instantané, sauf le crossfader.
