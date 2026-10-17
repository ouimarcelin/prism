# 0020 – Son des clips

- Date : 2026-10-17
- Statut : Acceptée (prise en mode autonome, à revoir sur demande)

## Contexte

Le cahier des charges demande la lecture audio des clips (phase 2). Les clips se lisent en avant, en arrière, à vitesse variable, en boucle, en aller-retour, au hasard et au scratch ; le son doit suivre l'image sans dériver.

## Décisions

- **Décodage complet en mémoire** : à l'ouverture d'un clip, un thread décode toute sa première piste son par AVFoundation (tous les conteneurs que macOS lit, HAP compris), en stéréo 16 bits à 48 kHz, 15 minutes au plus (environ 170 Mo). Lire à n'importe quelle position, dans les deux sens, devient immédiat.
- **Le rendu commande, le mixeur suit** : à chaque image, le moteur envoie au mixeur la position du clip, sa vitesse (négative en arrière) et son volume. Dans le rappel audio, chaque voix avance d'elle-même entre deux images ; un écart de moins de 100 ms est rattrapé doucement (deux fois l'écart par seconde), au-delà la voix saute (boucle, point de repère, saut). Changer de vitesse change la hauteur, comme une bande.
- **Volume** : volume du clip × opacité effective de sa couche (crossfader, solo et bypass compris) × volume général. Pendant une transition, l'ancien et le nouveau clip se fondent. Les changements de volume sont lissés (5 ms) : pas de clic.
- **Silences** : pause, fin d'un clip joué une fois et lecture aléatoire sont muettes. Le scratch sonne à la vitesse à laquelle la tête est déplacée.
- **Sortie** : la sortie par défaut du système, ouverte au démarrage ; « Son des clips » et le volume général sont dans le menu Audio (préférences).

## Options écartées

- **Décodage en continu** : moins de mémoire, mais lecture arrière et sauts coûteux, et un thread de plus à caler.
- **Son lié à la cadence d'affichage** (le rendu remplit un tampon) : sensible aux images en retard.
- **Étirement temporel sans changement de hauteur** : bien plus lourd ; pas demandé.

## Conséquences

- Mémoire : environ 11 Mo par minute de son.
- Le son n'est pas décalé pour compenser la latence de la sortie (environ 10 ms).
- Choix de la sortie audio et routage par couche : plus tard, sur demande.
