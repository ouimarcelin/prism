# 0008 – Interface egui et document édité

- Date : 2026-10-14
- Statut : Acceptée

## Contexte

La phase 1 demande une interface de contrôle : grille de clips, réglages des couches, crossfader, déclenchement à la souris et au clavier, fichiers de projet. Le cahier des charges retient egui. Les sorties sont présentées par le thread de rendu (ADR 0007), donc egui ne peut pas posséder la boucle des fenêtres comme le fait eframe.

## Options

1. **eframe**, l'enrobage standard d'egui. Le plus simple, mais il crée son propre périphérique GPU et sa propre boucle : l'aperçu demanderait une copie de l'image, et les sorties dépendraient du thread de l'interface.
2. **egui-winit et egui-wgpu intégrés à la main** dans notre boucle winit, sur le périphérique partagé avec le moteur. Un peu plus de code, aucune copie pour l'aperçu.

## Décision

Option 2, avec une crate `prism-ui` sans dépendance aux fenêtres :

- le `ControlPanel` possède le document édité (`Document`) : composition, fichier, modifications non enregistrées ; il renvoie des `Action` (commandes du moteur, sortie, quitter) que l'application exécute ;
- l'application envoie la composition au moteur chaque fois que la révision du document change ; déclencher un clip change la révision sans marquer le projet modifié (`Composition::same_content`) ;
- l'aperçu enregistre la texture programme du moteur auprès du moteur de rendu d'egui, sans copie ;
- interface en français ; noms par défaut des couches et colonnes en français ;
- boîtes de dialogue de fichiers natives avec `rfd` ; les fichiers glissés sur la fenêtre remplissent la ligne à partir de la case sélectionnée (winit 0.30 ne donne pas la position pendant un glisser-déposer) ;
- médias manquants : localisation fichier par fichier ou recherche par nom dans un dossier ;
- raccourcis propres au projet : 1 à 9 (colonnes), Espace (tout arrêter), F (sortie), Échap (fermer la sortie), Cmd+N/O/S et Cmd+Maj+S ;
- les polices fournies par egui (Ubuntu Light, Noto Emoji, Hack, emoji-icon-font) sont sous licences libres de polices ; `cargo-deny` les autorise pour ce seul paquet et `NOTICE` les cite.

## Conséquences

- L'interface se redessine en continu au rythme de l'écran pour suivre les têtes de lecture et l'aperçu ; à alléger si la consommation pose problème.
- Quitter par le menu macOS (Cmd+Q) ferme l'application sans demander d'enregistrer : à traiter.
- Pas encore d'annulation (Cmd+Z).
