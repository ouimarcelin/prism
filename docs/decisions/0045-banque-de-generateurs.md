# 0045 – Banque de générateurs procéduraux : socle commun

- Date : 2026-10-20
- Statut : Acceptée

## Contexte

@ouimarcelin veut une banque de vidéos générées en temps réel, bruits et vagues de toutes sortes, chacune avec beaucoup de réglages. Feuille de route : `docs/generateurs.md` (55 générateurs, 8 catégories). Il faut d'abord un socle, pour que chaque générateur reste court et que tous se règlent de la même façon.

## Décision

- **Réglages communs** (`COMMON_PARAMS`, 16) après les réglages propres de chaque générateur de la banque : zoom, position x/y, rotation, étirement ; vitesse, unité de temps (temps du tempo ou secondes), graine ; palette (gris, arc-en-ciel, feu, glace, océan, néon, coucher de soleil), teinte, saturation, contraste, luminosité, inversion, niveaux, noir transparent. `EffectInfo::common` donne l'indice du premier ; l'inspecteur les replie sous « Espace, temps et couleur ».
- **32 réglages par effet** au lieu de 16 (`MAX_PARAMS`, uniforme `array<vec4<f32>, 8>`, ISF compris).
- **Bibliothèque WGSL** `shaders/bank/generator.wgsl`, ajoutée après le prélude avec `const COMMON` : hachage entier pcg3d, bruit de valeur, Perlin et simplex en 3D (le temps est le troisième axe), somme fractale (fBm, turbulence, crêtes pondérées, gonflé), cellules de Worley (trois distances), palettes en cosinus, rotation de teinte. Un générateur n'écrit que `fn field(p, t) -> f32` (valeur de 0 à 1) ; le socle place le plan, fait tourner le temps et colore.
- **Premiers générateurs** : « Bruit fractal » (base valeur/Perlin/simplex × mode fBm/turbulence/crêtes/gonflé, fréquence, octaves, lacunarité, gain, évolution, dérive et son sens, finesse des crêtes), « Cellules » (F1, bords, pleines, F2, produit ; distance euclidienne, Manhattan ou Tchebychev ; densité, aléa, mouvement, épaisseur des bords), « Distorsion » (distorsion de domaine à 1–3 niveaux). Ils couvrent les numéros 1 à 11 de la feuille de route.
- **Vérification** : un test valide avec naga tous les shaders du catalogue, sans GPU ; l'exemple `gallery` dessine des variantes de la banque.

## Conséquences

- Un générateur de la banque coûte un fichier WGSL court et une entrée de catalogue.
- Les générateurs en couleur directe (pas une valeur passée dans une palette) demanderont une seconde forme de champ ; ceux à mémoire (◆) une mémoire par clip dans le moteur.
