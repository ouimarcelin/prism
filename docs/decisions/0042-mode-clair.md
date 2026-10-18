# 0042 – Mode clair

- Date : 2026-10-18
- Statut : Acceptée

## Contexte

@ouimarcelin demande un mode clair inspiré du kit UI3 (ADR 0040), avec un interrupteur dans le menu « Fichier ».

## Décision

- **Deux palettes** dans `theme.rs` (`DARK`, `LIGHT`) ; `theme::palette(ui)` rend celle du mode affiché. Le mode clair reprend les valeurs claires du kit : panneaux blancs, zone centrale et champs `#f5f5f5`, survol `#e6e6e6`, texte `#1a1a1a` (noir à 90 %), libellés `#808080`, indications `#b3b3b3`, accent `#0d99ff`, succès `#14ae5c`, erreur `#dc3412` ; ombres plus légères ; cartes de clips blanches sur la zone `#f5f5f5`.
- **Préférence** `light_theme` (sombre par défaut), appliquée à la fenêtre principale et à l'éditeur de scène à chaque image ; quel que soit le réglage du système.
- **Interrupteur** « Mode clair » dans le menu Fichier ; ce menu reste ouvert quand on le bascule, ses actions le ferment.
- Restent sombres : les couleurs des poignées et contours de l'éditeur de scène, dessinés sur l'image noire de la composition.

## Conséquences

- Toute couleur dessinée à la main passe par `palette(ui)`.
- La page de télécommande garde son thème sombre.
