# Shaders ISF d'exemple

Deux shaders écrits pour Prism, à charger avec « Shader ISF… » :

- `plasma.fs` : générateur (dans le menu « Générateur… » d'une case vide) ;
- `decalage.fs` : filtre (dans « Ajouter un effet… »).

Prism lit les fichiers ISF à une passe : en-tête JSON, entrées `float`, `bool`, `event`, `long`, `color`, `point2D` et l'image `inputImage` des filtres (voir `docs/decisions/0018-shaders-isf.md`). Un fichier modifié et enregistré est relu aussitôt.
