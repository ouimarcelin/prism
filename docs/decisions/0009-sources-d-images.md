# 0009 – Sources d'images communes à tous les formats

- Date : 2026-10-14
- Statut : Acceptée

## Contexte

Le moteur ne lisait que du HAP : son thread de décodage ouvrait un `HapDecoder` et envoyait des plans compressés BC. Le cahier des charges demande aussi les images fixes, les séquences PNG, puis H.264 et ProRes.

## Options

1. **Un chemin par format dans le moteur.** Rapide pour un format de plus, mais chaque format suivant dupliquerait la gestion des threads, du cache et des tampons.
2. **Une interface commune dans `prism-media`** : chaque format fournit sa taille, sa durée et décode une image par son numéro ; la trame décodée est soit des plans HAP, soit des pixels RGBA.

## Décision

Option 2 :

- trait `FrameSource` et énumération `Frame` (`Hap` ou `Rgba`) ; `open_file` choisit le décodeur selon l'extension, `open_sequence` lit un dossier ;
- les pixels RGBA (alpha non prémultiplié) passent par la même passe de conversion GPU que le HAP, qui prémultiplie l'alpha ;
- une séquence d'images est un dossier de PNG triés dans l'ordre des nombres de leur nom, joués à une fréquence enregistrée avec le clip (30 images/s par défaut) ;
- une image fixe est un clip d'une seule image ;
- PNG décodé avec la crate `png`, JPEG avec `zune-jpeg` (licences MIT, Apache 2.0 ou Zlib).

## Conséquences

- Ajouter un format revient à écrire une `FrameSource` ; H.264 et ProRes suivront ce chemin.
- Les PNG se décodent sur le processeur à chaque image : une séquence 1080p peut ne pas tenir 60 images/s ; le HAP reste le format recommandé pour la performance.
