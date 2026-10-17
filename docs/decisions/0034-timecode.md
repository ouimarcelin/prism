# 0034 – Timecode et repères

- Date : 2026-10-17
- Statut : Acceptée

## Contexte

La phase 5 demande le timecode SMPTE (LTC et MIDI timecode) et des clips calés sur le timecode ; critère : « un clip démarre au bon timecode ».

## Décision

- **Timecode** (`prism-clock`) : heures, minutes, secondes, images, à 24, 25, 29,97 (drop frame) ou 30 images/s ; conversion en secondes de temps réel ; lecture et écriture `hh:mm:ss:ff` (`;` en drop frame).
- **MIDI timecode (MTC)** : `MtcReader` assemble les huit quarts d'image (`F1`) ; ils décrivent l'image où le premier a été envoyé, deux images plus tôt : le timecode lu est avancé de deux images. Le message d'image complète (`F0 7F 7F 01 01 hh mm ss ff F7`) donne l'heure d'un coup lors d'un saut. Les entrées MIDI écoutées fournissent le timecode, avancé du temps écoulé depuis le dernier message tant qu'il en arrive (silence de plus de 250 ms : arrêté).
- **Repères** (`Timeline`, dans la composition) : une heure et une cible (colonne, clip, « Tout arrêter »). Un repère se déclenche quand le timecode passe son heure en avançant ; un recul ou un saut de plus d'une seconde ne déclenche rien (on se repositionne sans lancer les repères sautés). Fenêtre Composition > Timecode….
- Les déclenchements sont de la lecture : ils ne s'annulent pas (Cmd+Z).

## Conséquences

- Précision : le timecode est regardé à chaque image de l'interface (30 à 60 par seconde) ; un repère part au plus une image de l'interface plus une image du moteur après son heure.
- « Clips calés sur le timecode » au sens de la position de lecture suivant le timecode (chase) : pas encore.
- LTC (timecode audio) : ticket suivant.
