# 0046 – Mémoire d'image des générateurs

- Date : 2026-10-21
- Statut : Acceptée

## Contexte

Quatre générateurs de la banque (`docs/generateurs.md`, ◆) partent de l'image précédente : réaction-diffusion, fluide 2D, particules dans un flux, feux d'artifice. Le moteur ne gardait rien d'une image à l'autre. @ouimarcelin a demandé si cette mémoire est coûteuse : deux images de 16 bits par clip actif, de 8 Mo (960 × 540) à 33 Mo (1920 × 1080).

## Décision

- **Où** : dans la chaîne d'effets du clip (`EffectChain::memory`). Une chaîne vit avec le flux du clip : la mémoire est créée au premier rendu et libérée quand le clip s'arrête ou est remplacé. Aucune mémoire pour les clips à l'arrêt.
- **Forme** : deux textures `Rgba16Float` en alternance (une lue, l'autre écrite). Les générateurs y rangent ce qu'ils veulent (concentrations, vitesse et couleur, luminosité). Alpha à 0 veut dire « pas encore ensemencé » : les textures neuves sont vides, la première étape sème.
- **Passes** : `EffectInfo::memory` ; la première passe, `fs_step`, fait avancer l'état autant de fois par image que le premier réglage (« Pas par image », 32 au plus) ; la seconde, `fs_main`, lit l'état et passe par la couleur commune de la banque. Le pixel en cours est donné à `field` par `bank_uv`, la taille d'un texel de l'état par `bank_texel()`.
- **Résolution** : le deuxième réglage (« Résolution ») divise la taille du clip par 1, 2 ou 4 (`MEMORY_RESOLUTIONS`) ; « Demie » par défaut. Changer de résolution ou de générateur repart d'un état vide.
- Pas de pas de temps : une étape par pas, le moteur tournant à 60 images/s.

## Conséquences

- Le fluide 2D n'a pas de résolution de pression (il en faudrait plusieurs passes par itération) : le remous vient d'un bruit de rotationnel, sans divergence. Une vraie projection pourra venir plus tard.
- Relancer un clip qui ouvre un nouveau flux repart de zéro ; il n'y a pas encore de bouton « Réensemencer ».
- Les effets et les couches ne peuvent pas encore avoir de mémoire (rétroaction, traînées) ; le même mécanisme pourra servir.
