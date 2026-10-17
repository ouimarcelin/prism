# 0014 – Tempo et horloge musicale

- Date : 2026-10-14
- Statut : Acceptée

## Contexte

La phase 2 cale des effets et des animations sur le tempo. Son critère de validation : un effet cadencé sur le BPM reste synchrone après 10 minutes. Il faut donc une position en temps (beats) exacte sur la durée, un tempo réglable à la main ou au tap tempo, et un moyen de recaler le début de la mesure.

## Décisions

- **Crate `prism-clock`**, sans dépendance. `BeatClock` calcule la position en temps à partir d'un seul point d'ancrage : un instant de l'horloge monotone du système et la position à cet instant. La position n'est jamais additionnée d'image en image. Elle ne dérive donc pas, et une image sautée ou en retard ne la décale pas. En `f64`, l'erreur reste sous le milliardième de temps après 10 minutes.
- **Le tempo appartient à la composition** (`Bpm`, de 20 à 999, 120 par défaut) et s'enregistre dans le projet. Le changer fait repartir la position de l'instant de la dernière image, sans saut.
- **Tap tempo dans l'interface** (`TapTempo`). Le tempo est l'intervalle moyen entre les 8 dernières frappes ; une pause de plus de 2 s commence une nouvelle série. Chaque frappe envoie au moteur son instant et le tempo mesuré : un temps tombe sur la frappe, sans changer le compte des mesures. La première frappe d'une série ne fait que placer le temps. Le bouton compte à l'appui, pas au relâchement.
- **Resync** : la mesure commence à l'instant de la commande.
- Une mesure a 4 temps ; l'interface montre le temps courant de la mesure.
- Le fondu automatique du crossfader peut durer un nombre de temps au tempo de la composition (« transition au tempo » du cahier des charges).

## Options écartées

- **Ableton Link** pour se synchroniser avec d'autres logiciels du réseau. Le cahier des charges cite la bibliothèque `rusty_link`, qui enveloppe le code d'Ableton. Ce code est sous licence GPL (ou sous licence commerciale), incompatible avec la double licence MIT / Apache 2.0 du projet. Link attendra une implémentation compatible ou une décision sur la licence.
- **Position additionnée image par image** : simple, mais elle accumule les erreurs d'arrondi et suit les images sautées.

## Conséquences

- Le tempo vient de l'horloge du Mac. Une musique jouée par une autre machine dérive lentement par rapport à lui : quelques dizaines de millisecondes en 10 minutes, l'écart habituel entre deux horloges à quartz. Les tap et resync servent à recaler. La synchro par horloge MIDI viendra avec le contrôle MIDI, en phase 3.
- Les paramètres calés sur le tempo (phase 2) liront la position en temps publiée par le moteur à chaque image.
- **Ajout du 2026-10-17 (ticket 54) :** en attendant Link, le tempo peut suivre l'horloge MIDI (ADR 0032).
