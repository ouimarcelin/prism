# 0018 – Shaders ISF

- Date : 2026-10-17
- Statut : Acceptée (prise en mode autonome, à revoir sur demande)

## Contexte

Le cahier des charges demande des générateurs à partir de « shaders personnalisés (format ISF) ». ISF (Interactive Shader Format) est un format ouvert : un fragment shader GLSL précédé d'un en-tête JSON qui décrit ses entrées. Le moteur d'effets compile du WGSL (ADR 0015).

## Décisions

- **Effet de genre `isf` et fichier** (`Effect::file`), enregistré relatif au projet comme un média. Le même fichier sert de filtre (s'il a une entrée image `inputImage`) ou de générateur (sinon) ; rien n'empêche d'utiliser l'un comme l'autre.
- **Traduction par naga** (déjà dépendance de wgpu) : le code GLSL est enveloppé dans un shader GLSL 450 qui définit, par des macros, les noms d'ISF (`gl_FragColor`, `gl_FragCoord`, `isf_FragNormCoord`, `TIME`, `RENDERSIZE`, `IMG_NORM_PIXEL`, `IMG_THIS_PIXEL`, `IMG_PIXEL`, `IMG_SIZE`, `texture2D`…), puis traduit en WGSL et validé. Les liaisons sont celles des effets intégrés ; le vertex shader est celui du prélude.
- **Conventions** : ISF compte les coordonnées depuis le bas et travaille en alpha droit ; l'enveloppe retourne les coordonnées et les lectures d'image, déprémultiplie les lectures et reprémultiplie la sortie.
- **Entrées** : `float` (plage), `bool` et `event` (non / oui), `long` (liste, valeurs quelconques), `color` (4 réglages), `point2D` (2 réglages : dans la plage donnée, sinon de 0 à 1 converti en pixels). Au plus 16 réglages par effet : la limite des effets passe de 8 à 16.
- **Non pris en charge** : plusieurs passes, images importées, autres images que `inputImage`, entrées audio, vertex shader propre. Le fichier est refusé avec un message clair. `TIMEDELTA` vaut 1/60, `FRAMEINDEX` est déduit du temps, `DATE` vaut zéro.
- **Chargement** : un fichier est lu, traduit et gardé en mémoire par chemin et date de modification ; enregistrer le fichier le recharge à l'image suivante. Le pipeline GPU est créé au premier usage. Les descriptions de réglages ne sont jamais libérées (une par version du fichier), ce qui reste petit.
- Les erreurs (lecture, en-tête, compilation avec le numéro de ligne du fichier) s'affichent dans l'inspecteur, sous le nom de l'effet.

## Options écartées

- **GLSL donné directement à wgpu** (fonction `glsl` de wgpu) : même traducteur, mais erreurs moins accessibles et pas de contrôle avant la création du pipeline.
- **Écrire un traducteur GLSL vers WGSL** : hors de portée.

## Conséquences

- Les shaders qui reposent sur des conversions implicites de GLSL ES (entier vers flottant) peuvent être refusés par naga.
- Multi-passe et images importées : ticket à part si le besoin est exprimé.
