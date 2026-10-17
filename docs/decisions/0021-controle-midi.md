# 0021 – Contrôle MIDI et apprentissage

- Date : 2026-10-17
- Statut : Acceptée (prise en mode autonome, à revoir sur demande)

## Contexte

La phase 3 demande le mapping MIDI par apprentissage (MIDI learn), et son critère est un set complet joué au contrôleur MIDI sans souris.

## Décisions

- **Crate `prism-control`** : entrées MIDI par `midir` (Core MIDI sur macOS, licence MIT). Seuls les notes et les contrôles continus (CC) sont lus ; une note à vélocité nulle est un relâchement.
- **Correspondances dans la composition** (`Composition::mappings`, enregistrées dans le projet) : un set garde ses commandes d'un ordinateur à l'autre. Une source (note ou CC, avec son canal) pilote une cible : clip, colonne, opacité, bypass, solo et arrêt d'une couche, crossfader, curseur du tableau de bord, réglage ou activation d'un effet (de la composition, d'une couche, d'un clip, d'un générateur), tap, resync, tout arrêter.
- **Valeurs** : une cible continue suit la valeur sur toute sa plage (CC de 0 à 127, vélocité d'une note) ; les autres agissent à l'appui. Un CC utilisé comme bouton agit quand il passe la moitié en montant, une fois par appui. Bypass et solo basculent à chaque appui. Pas de reprise en douceur (« soft takeover ») pour l'instant : la valeur saute à celle du contrôleur.
- **Apprentissage** : menu Contrôle > Apprentissage MIDI. Chaque commande pilotable est couverte d'un cadre (gris : libre ; bleu : déjà reliée ; orange : choisie). Cliquer choisit ; la première commande MIDI bougée (note enfoncée ou CC) lui est reliée ; une source déjà reliée est déplacée. Les cibles sans widget (tout arrêter, tap, resync) s'apprennent depuis la fenêtre des correspondances, qui liste et retire les correspondances.
- **Entrées** : toutes les entrées MIDI présentes sont écoutées, nouvelles comprises (vérifiées toutes les 2 s), sauf celles décochées dans le menu Contrôle (préférences). Les messages réveillent la boucle d'interface, qui les applique à la composition comme une modification à la souris.

## Options écartées

- **Correspondances dans les préférences** (liées au contrôleur plutôt qu'au set) : utile pour un contrôleur toujours câblé pareil ; à ajouter (correspondances globales) si le besoin est exprimé.
- **Application sur le thread de rendu** : plus rapide de quelques millisecondes, mais l'interface et le projet ne verraient pas les changements.

## Conséquences

- Latence : celle de la boucle d'interface réveillée par le message, puis une image du moteur.
- Les correspondances vers une couche, une colonne ou un effet retirés restent dans le projet et ne font rien ; la fenêtre les montre comme « retirée ».
- Sortie MIDI (retour lumineux des pads) et OSC : tickets suivants.
- **Ajout du 2026-10-17 (ticket 30) : touches.** Une touche sans modificateur est une source comme une autre (`ControlSource::Key`, nom anglais d'egui : `A`, `1`, `Space`, `Up`…). Elle s'apprend de la même façon (appuyer pendant l'apprentissage) et agit depuis la fenêtre de contrôle comme depuis la sortie plein écran. Une touche apprise remplace son raccourci habituel (par exemple `T` pour le tap) ; les touches avec Cmd restent réservées aux commandes de l'application.
- **Ajout du 2026-10-17 (ticket 31) : retour vers le contrôleur.** Prism ouvre les sorties MIDI qui portent le nom d'une entrée écoutée (un contrôleur a les deux) et y envoie, à chaque changement, l'état de chaque commande reliée : note à 127 quand le clip ou la colonne joue, quand bypass ou solo est actif, quand l'effet est actif ; CC à la valeur de l'opacité, du crossfader, du curseur ou du réglage. Une sortie nouvellement ouverte reçoit tout. Activé par défaut (menu Contrôle).

