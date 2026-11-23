# 0051 – Objets 3D et projecteur virtuel

- Date : 2026-11-23
- Statut : Acceptée

## Contexte

@ouimarcelin demande de pouvoir mapper un objet 3D. Les écrans de la scène savent déjà placer des découpes 2D (coins, grille, Bézier) ; un volume demande de savoir d'où le projecteur le voit.

## Décision

- **Par écran** (un écran = un projecteur) : un `Projector` (position, point visé, champ vertical, décentrement d'objectif) et des `Object3d` (forme, position, rotation en degrés x puis y puis z, échelle, zone de la composition posée sur ses coordonnées de texture, opacité, affiché). Formes : cube et plan intégrés (une face = toute la zone), ou fichier Wavefront OBJ lu par la crate `tobj` 4 (MIT ; triangulé, un seul index ; sans coordonnées de texture, la vue de face place l'image). Chemins de fichiers relatifs au projet comme ceux des médias.
- **Rendu** (`prism-render`, `objects.rs`) : après les découpes, dans l'image de l'écran, avec un tampon de profondeur ; matrices calculées à la main (vue « regarder vers », perspective profondeur 0 à 1 de wgpu, décentrement) ; le programme échantillonné aux coordonnées de texture dans la zone choisie, prémultiplié, par-dessus. Maillages chargés une fois par forme ; une erreur de lecture est gardée (`StageRenderer::mesh_error`).
- **Interface** : éditeur de scène, sous les découpes : « Projecteur » et la liste des objets (« + Cube », « + Plan », « + Fichier OBJ… »).

## Conséquences

- Pas encore de calage automatique du projecteur par points (correspondance de quelques points connus) : la position se règle à la main en regardant l'image projetée.
- Le miroir de l'écran ne s'applique pas encore aux objets ; pas d'éclairage ni d'ombrage (l'image est posée telle quelle).
- Un fichier OBJ est lu sur le fil de rendu au premier affichage.
