# Banque de générateurs procéduraux

Feuille de route des générateurs : images calculées par le GPU en temps réel, sans fichier, tous réglables, animables (« ~ »), liables au tempo, au son, au tableau de bord et au MIDI. Les algorithmes sont publics ; chaque source est notée dans `docs/SOURCES.md` au moment de coder.

Légende — coût : ● shader simple, ●● shader lourd, ◆ garde une mémoire d'une image à l'autre (demande une mémoire par clip dans le moteur). Priorité : P1 socle de la banque, P2 enrichissement, P3 plus tard. État : ✅ fait, — à faire.

Déjà là avant la banque : couleur unie, dégradé, lignes, cercles, damier, bruit (simple), texte.

## Socle commun ✅

Réglages partagés par tous les générateurs de la banque, après leurs réglages propres (section « Couleur, espace et temps » de l'inspecteur) :

- **Espace** : zoom, position x/y, rotation, étirement.
- **Temps** : vitesse, unité (temps du tempo ou secondes), graine.
- **Couleur** : palette (gris, arc-en-ciel, feu, glace, océan, néon, coucher de soleil), teinte, saturation, contraste, luminosité, inversion, niveaux (postérisation), noir transparent.

Bibliothèque WGSL (`generator.wgsl`) : hachage entier, bruit de valeur, Perlin et simplex en 3D (le temps est le troisième axe), fBm et ses variantes, Worley. Chaque générateur n'écrit qu'un champ `field(p, t) -> f32` ; le socle fait l'espace, le temps et la couleur.

## 1. Bruits

| # | Générateur | Principe | Paramètres propres | Coût | Prio | État |
|---|---|---|---|---|---|---|
| 1 | Bruit de valeur | valeurs aléatoires lissées sur grille | lissage | ● | P1 | ✅ (Bruit fractal, base « valeur ») |
| 2 | Perlin | bruit de gradient | fréquence, évolution | ● | P1 | ✅ (Bruit fractal, base « Perlin ») |
| 3 | Simplex | gradient sur grille triangulaire | fréquence, évolution | ● | P1 | ✅ (Bruit fractal, base « simplex ») |
| 4 | fBm fractal | somme d'octaves | octaves 1–10, lacunarité, gain | ● | P1 | ✅ (Bruit fractal, mode « fBm ») |
| 5 | Turbulence | fBm en valeur absolue | octaves, lacunarité, gain | ● | P1 | ✅ (Bruit fractal, mode « turbulence ») |
| 6 | Crêtes (ridged) | multifractal à crêtes | octaves, finesse des crêtes | ● | P1 | ✅ (Bruit fractal, mode « crêtes ») |
| 7 | Gonflé (billow) | crêtes inversées, aspect nuageux | octaves | ● | P2 | ✅ (Bruit fractal, mode « gonflé ») |
| 8 | Distorsion de domaine | bruit déformé par du bruit | niveaux 1–3, force, échelle | ●● | P1 | ✅ |
| 9 | Worley F1 | distance au point le plus proche | densité, aléa, métrique | ● | P1 | ✅ (Cellules, mode « F1 ») |
| 10 | Worley F2−F1 | bords de cellules | épaisseur, métrique, mode | ● | P1 | ✅ (Cellules, mode « bords ») |
| 11 | Voronoï plein | cellules à couleur unie | densité, bordure, mouvement | ● | P1 | ✅ (Cellules, mode « plein ») |
| 12 | Curl noise | champ tourbillonnant | échelle, force, octaves | ●● | P2 | ✅ |
| 13 | Bruit blanc / grain | neige TV | taille du grain, cadence, couleur/mono | ● | P1 | ✅ |
| 14 | Bruit rose / brun | grain filtré | pente spectrale, taille | ● | P3 | ✅ |
| 15 | Bruit de Gabor | bruit orienté, fibreux | orientation, fréquence, dispersion | ●● | P3 | ✅ |

## 2. Vagues et oscillations

| # | Générateur | Principe | Paramètres propres | Coût | Prio | État |
|---|---|---|---|---|---|---|
| 16 | Onde plane | sinus dans une direction | forme, angle, longueur d'onde, rapport cyclique | ● | P1 | ✅ |
| 17 | Ondes concentriques | cercles qui s'éloignent | centre, longueur d'onde, amortissement, forme | ● | P1 | ✅ |
| 18 | Interférences | 2 à 8 sources d'ondes | sources, disposition, mouvement, phase | ● | P1 | ✅ |
| 19 | Plasma | somme de sinus | termes, fréquences, complexité | ● | P1 | ✅ |
| 20 | Moiré | deux trames superposées | type, écart d'angle, écart de pas | ● | P1 | ✅ |
| 21 | Ondes stationnaires | figures de Chladni | modes n/m, forme, netteté | ● | P2 | ✅ |
| 22 | Lissajous | courbe de rapports de fréquences | rapports, déphasage, épaisseur, traînée | ● | P2 | ✅ |
| 23 | Spirale | bras qui tournent | bras, enroulement, type | ● | P1 | ✅ |
| 24 | Balayage radar | secteur tournant et rémanence | largeur, rémanence, anneaux | ● | P2 | ✅ |
| 25 | Océan | vagues de Gerstner | hauteur, vent, écume, perspective | ●● | P2 | ✅ |
| 26 | Oscilloscope | trace de signal | forme, fréquence, bruit, lueur | ● | P2 | ✅ |
| 27 | Vagues de lignes | lignes déplacées par une onde | lignes, amplitude, perspective | ● | P1 | ✅ |

## 3. Fluides et matières

| # | Générateur | Principe | Paramètres propres | Coût | Prio | État |
|---|---|---|---|---|---|---|
| 28 | Caustiques | lumière sous l'eau | profondeur, netteté, dispersion | ●● | P1 | ✅ |
| 29 | Fumée | fBm qui monte | densité, montée, turbulence | ●● | P1 | ✅ |
| 30 | Feu | turbulence étirée | hauteur, intensité, vent | ●● | P1 | ✅ |
| 31 | Aurore | rideaux ondulants | rideaux, ondulation, scintillement | ●● | P2 | ✅ |
| 32 | Nuages | fBm à seuil, éclairé | couverture, densité, vent | ●● | P2 | ✅ |
| 33 | Marbre / bois | sinus perturbé | type, veines, turbulence | ● | P2 | ✅ |
| 34 | Métaballes / lave | champs additionnés à seuil | boules, taille, seuil | ● | P1 | ✅ |
| 35 | Réaction-diffusion | Gray-Scott | f, k, diffusion, ensemencer | ◆ | P2 | — |
| 36 | Fluide 2D | advection de couleur | viscosité, injection | ◆ | P3 | — |

## 4. Motifs géométriques

| # | Générateur | Principe | Paramètres propres | Coût | Prio | État |
|---|---|---|---|---|---|---|
| 37 | Tunnel | coordonnées polaires inversées | forme, vitesse, torsion, motif | ● | P1 | ✅ |
| 38 | Vortex | rotation selon le rayon | torsion, bras | ● | P2 | ✅ |
| 39 | Truchet | tuiles aléatoires | type, taille, épaisseur | ● | P1 | ✅ |
| 40 | Hexagones / triangles | pavage pulsé | pavage, espacement, pulsation | ● | P1 | ✅ |
| 41 | Rayons | secteurs alternés | rayons, rotation, douceur | ● | P2 | ✅ |
| 42 | Grille déformée | grille perturbée par du bruit | pas, épaisseur, déformation | ● | P2 | ✅ |
| 43 | Polygones répétés | formes emboîtées | côtés, répétitions, décalage | ● | P2 | ✅ |

## 5. Fractales

| # | Générateur | Principe | Paramètres propres | Coût | Prio | État |
|---|---|---|---|---|---|---|
| 44 | Mandelbrot | zoom fractal | centre, zoom, itérations | ●● | P2 | ✅ |
| 45 | Julia | ensemble de Julia animé | constante c, itérations | ●● | P1 | ✅ |
| 46 | Pli fractal | repliements répétés | plis, angle, échelle | ●● | P2 | ✅ |
| 47 | Sierpinski | fractale géométrique | niveaux, forme | ● | P3 | ✅ |

## 6. Signal et glitch

| # | Générateur | Principe | Paramètres propres | Coût | Prio | État |
|---|---|---|---|---|---|---|
| 48 | Mire et barres | barres de test | type, dérive | ● | P2 | ✅ |
| 49 | Signal analogique | balayage, décrochage | décrochage, bruit de ligne | ● | P2 | ✅ |
| 50 | Blocs numériques | macroblocs aléatoires | taille, densité, cadence | ● | P2 | ✅ |

## 7. Particules et champs

| # | Générateur | Principe | Paramètres propres | Coût | Prio | État |
|---|---|---|---|---|---|---|
| 51 | Champ d'étoiles | vitesse lumière | densité, vitesse, traînées | ● | P1 | ✅ |
| 52 | Pluie / neige | chute par couches | densité, angle, vitesse | ● | P2 | ✅ |
| 53 | Points en grille | points suivant un bruit | pas, taille, bruit | ● | P1 | ✅ |
| 54 | Particules dans un flux | traînées sur un curl noise | nombre, durée de vie | ◆ | P3 | — |
| 55 | Feux d'artifice | gerbes au tempo | gerbes, taille, gravité | ◆ | P3 | — |

## Ordre

1. Socle commun et premiers bruits (fait).
2. Vagues P1 (fait) : formes d'onde communes (sinus, triangle, carré, dents de scie) dans `generator.wgsl`.
3. Tous les générateurs sans mémoire d'image, P1 à P3 (fait, ticket 104).
4. Mémoire d'image par clip (◆).
5. P2, puis P3.
