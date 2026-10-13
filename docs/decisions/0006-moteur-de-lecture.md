# 0006 – Moteur de lecture et thread de rendu

- Date : 2026-10-14
- Statut : Acceptée

## Contexte

Le cahier des charges impose que le moteur de rendu tourne à 60 images/s sur son propre thread et que l'interface ne le bloque jamais. Il faut décider qui possède l'état de la composition, comment l'interface communique avec le moteur, et où vit le décodage.

## Options

1. **État partagé sous verrou** : l'interface et le moteur lisent et écrivent la même composition. Simple, mais un verrou tenu par l'interface (boîte de dialogue, calcul long) peut faire attendre le moteur.
2. **Copies et messages** : l'interface possède le document et envoie au moteur une copie complète à chaque modification ; le moteur possède l'état de lecture (têtes de lecture, clips chargés) et publie son état après chaque image. Aucun des deux n'attend l'autre.

## Décision

Option 2, dans une nouvelle crate `prism-engine` (absente de la liste initiale des crates, elle relie `core`, `media` et `render`) :

- l'interface modifie sa composition puis l'envoie au moteur (`Command::SetComposition`) ; une copie pèse quelques kilo-octets ;
- déclencher un clip reste une opération du modèle (`trigger_clip`, `trigger_column`) ; un compteur de démarrages par couche (`trigger_count`, non enregistré) permet au moteur de relancer un clip déclenché à nouveau ;
- les gestes qui touchent la tête de lecture passent par des commandes : `Seek` (avec maintien pour le scratch), `Release`, `JumpToCue` ;
- le moteur applique les commandes entre deux images et publie un `EngineStatus` ; il écrit l'état avec `try_lock`, donc n'attend jamais ;
- un thread de décodage par clip lu ; le moteur lui demande l'image courante puis celles des 6 prochaines images, et lui rend les tampons après envoi au GPU ;
- une image non décodée à temps laisse la précédente à l'écran et compte comme « en retard » ; le moteur ne patiente jamais ;
- l'image mixée est copiée à chaque image dans une texture programme stable, que les sorties et l'aperçu lisent sans risque de voir une image à moitié composée ;
- les transitions entre clips sont un fondu dans le compositeur (ADR 0004), démarré à la première image du nouveau clip.

## Conséquences

- Les contrôles à venir (MIDI, OSC) passeront par la composition de l'interface et ses commandes, ou enverront eux-mêmes des commandes au moteur.
- Un clip déclenché ouvre son fichier sur son thread : le premier affichage prend quelques millisecondes, sans geler l'image. Un préchargement des clips pourra réduire ce délai.
- Le moteur peut tourner sans fenêtre : tests de bout en bout et mesures (`engine-run`).
