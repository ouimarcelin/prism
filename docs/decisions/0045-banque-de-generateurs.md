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

## Complément (ticket 97) : vagues

Les générateurs de vagues partagent `wave_shape(phase, forme, rapport cyclique, douceur)` dans `generator.wgsl` : sinus, triangle (montée sur le rapport cyclique), carré (allumé sur le rapport cyclique, bords adoucis) et dents de scie. Le temps fait avancer d'une longueur d'onde par unité (temps du tempo ou seconde, fois la vitesse). Les palettes cycliques (arc-en-ciel, néon, coucher de soleil) ne parcourent que 80 % de leur cycle : sinon 0 et 1 ont la même couleur et une forme à deux valeurs disparaît.

## Complément (ticket 104) : générateurs sans mémoire

Les 33 générateurs restants qui ne gardent rien d'une image à l'autre sont faits, sur le même modèle (`field(p, t)`, réglages propres puis communs). `bank_pixel()` donne la taille d'un pixel dans le plan du générateur, pour adoucir les bords des traits sans dérivées. Restent les quatre générateurs à mémoire (réaction-diffusion, fluide 2D, particules dans un flux, feux d'artifice), qui attendent une mémoire d'image par clip dans le moteur.
