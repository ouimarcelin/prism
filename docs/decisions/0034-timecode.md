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
- Clips calés : voir l'ajout du ticket 62.
- LTC : voir l'ajout du ticket 61.
- **Ajout du 2026-10-17 (ticket 61) : LTC.** Le timecode audio est lu sur le premier canal de l'entrée audio (menu Audio) : passages par zéro avec un peu d'hystérésis, intervalles classés en demi-bit ou bit entier par rapport à la longueur de bit suivie en continu, mot de synchronisation de fin de trame, heure en BCD ; la cadence vient de la longueur des bits (drop frame par son drapeau). Comme pour le MTC, l'heure lue est celle de la fin de la trame. La fenêtre Timecode choisit la source : MIDI (MTC) ou audio (LTC). La lecture à l'envers n'est pas reconnue.
- **Ajout du 2026-10-17 (ticket 62) : clips calés.** Un clip peut avoir une heure de départ (`Playback::timecode_start`) : tant que le timecode est suivi, sa position de lecture vaut « timecode − départ », calculée par le moteur à chaque image depuis la dernière heure reçue (transmise par l'interface avec son instant) ; quand le timecode s'arrête, le clip s'immobilise ; quand le calage est retiré, il repart librement. Inspecteur : « Timecode : Calé », heure de départ, « Maintenant ».
