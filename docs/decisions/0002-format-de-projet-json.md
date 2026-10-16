# 0002 – Format de projet JSON

- Date : 2026-10-13
- Statut : Acceptée

## Contexte

Le cahier des charges demande une sauvegarde en format ouvert (JSON) et la gestion des médias manquants. Un projet doit survivre au déplacement de son dossier (copie sur un disque de spectacle) et rester lisible par les versions suivantes de Prism.

## Options

1. **Sérialiser directement les structures du modèle**, sans version ni traitement des chemins. Simple, mais la moindre évolution casse les anciens fichiers, et un dossier déplacé perd tous ses médias.
2. **Document versionné avec chemins relatifs.** Un champ `format_version` en tête, les chemins des médias situés dans le dossier du projet stockés en relatif, et une validation des invariants au chargement.

## Décision

Option 2, implémentée dans `crates/core/src/project.rs` :

- extension `.prism`, JSON indenté pour rester lisible et comparable dans git ;
- `format_version` (actuellement `1`) et `generator` (version de Prism) en tête ; un fichier d'une version plus récente est refusé avec un message clair ;
- les médias situés dans le dossier du projet sont enregistrés en chemin relatif avec des `/`, les autres en chemin absolu ;
- au chargement, la grille est validée : identifiants uniques, une case par colonne pour chaque couche ; les valeurs hors limites (opacité, position du crossfader) sont ramenées dans leurs bornes plutôt que refusées ;
- l'état de lecture (clips en cours) n'est pas enregistré : un projet s'ouvre toujours à l'arrêt ;
- l'écriture passe par un fichier temporaire renommé ensuite : un échec ne détruit jamais le fichier existant ;
- `Project::missing_media` liste les médias introuvables avec leur case, pour que l'interface propose de les relocaliser.

## Conséquences

- Chaque changement incompatible du format augmente `FORMAT_VERSION` et ajoute une conversion depuis les versions précédentes, avec un test.
- Les champs ajoutés plus tard ont une valeur par défaut, pour que les anciens fichiers restent lisibles sans conversion.

## Ajout du 2026-10-17 (ticket 24) : préréglages

- Un préréglage est un fichier JSON à son nom, avec `format_version` (1) et `kind` : `effects` (liste d'effets) ou `layer` (opacité, fusion, côté du crossfader, transition, effets, animation de l'opacité ; ni nom, ni clips, ni état).
- Dossiers `Préréglages/Effets` et `Préréglages/Couches` à côté du fichier de préférences (sur macOS : `~/Library/Application Support/Prism`). Les chemins de fichiers d'effets (shaders ISF) y restent absolus.
- Appliquer un préréglage d'effets ajoute ses effets à la fin de la chaîne ; appliquer un préréglage de couche remplace ses réglages et ses effets. Un nom déjà pris remplace le préréglage.
