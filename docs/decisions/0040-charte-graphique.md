# 0040 – Charte graphique

- Date : 2026-10-18
- Statut : Acceptée

## Contexte

L'interface utilisait le thème sombre par défaut d'egui (police Ubuntu, texte à 14 points, gris bleutés), avec des couleurs écrites en dur dans plusieurs fichiers (orange de sélection, vert de lecture, rouge des médias manquants). @ouimarcelin demande une interface « cohérente, moderne », qui suive le kit d'interface UI3 publié par Figma dans sa communauté (fichier `5tZBgBPJi2K8bBWsabzoL8`).

Le kit a été lu avec l'outil Figma de Claude Code : variables (couleurs en mode sombre, espacements, rayons, typographie), styles de texte et d'ombre, dimensions des composants (boutons, champs, curseurs). Aucun élément graphique (icône, image) n'a été copié : seules des valeurs (couleurs, tailles) sont reprises.

## Décision

- **Un module `prism-ui::theme`** porte toute l'apparence ; l'application l'applique à chaque contexte egui (`prism_ui::apply_theme`), avant la première image.
- **Thème sombre seul**, quel que soit le réglage du système : l'interface sert dans des salles sombres, à côté de la sortie.
- **Couleurs** (`theme::color`) : surfaces `#2c2c2c` (panneaux), `#383838` (champs, cartes), `#444444` (survol, rails), `#1e1e1e` (zone centrale, menus, infobulles) ; texte blanc, `#bfbfbf` pour les libellés (blanc à 70 %), `#808080` pour les indications (40 %) ; accent `#0c8ce9` (sélection, bascules actives, remplissage des curseurs) ; succès `#198f51` (clip en lecture), avertissement `#f3c11b`, erreur `#fca397`, troisième couleur `#f316b0` (masques de la scène).
- **Typographie** : Inter (licence SIL OFL 1.1) embarquée dans le binaire, graisses 500 et 600 (fichiers statiques de Google Fonts ; le kit utilise 450 et 550 en police variable, qu'egui ne sait pas lire). Texte à 11 points, petit texte à 9, titres à 13 en demi-gras. La police monospace reste celle d'egui. La police d'affichage du kit (Whyte) est commerciale : non reprise.
- **Tailles** : contrôles de 24 points, rayon de 5 points (13 pour les fenêtres et menus), espacements de 4, 8 et 16 points, ombres des niveaux « menu » et « fenêtre » du kit.
- **Grille de clips** dessinée à la main : cartes `#383838` avec le type de source au-dessus du nom, cases vides en contour, clip en lecture en vert, sélection en anneau bleu, barre de progression blanche en bas de la carte.
- **Emphase** : `.semibold()` (trait `theme::Emphasis`) remplace `.strong()` d'egui, qui ne changeait que la couleur.
- **Page de télécommande** (API réseau) : mêmes couleurs, rayons et police (Inter si l'appareil l'a, sinon celle du système).
- **Vérification visuelle** : l'exemple `cargo run -p prism-ui --example capture` dessine la fenêtre principale et l'éditeur de scène hors écran (egui_kittest et wgpu) et enregistre des PNG dans `target/captures`.

## Conséquences

- Une seule source pour l'apparence : changer une couleur ou une taille se fait dans `theme.rs`.
- Tout contexte egui qui dessine l'interface doit recevoir le thème : sans lui, la famille de police demi-grasse n'existe pas et egui s'arrête. Les tests de l'interface créent leurs contextes avec le thème.
- Le binaire grossit de 650 Ko (les deux fichiers Inter).
- Les curseurs d'egui gardaient une poignée grise : remplacés par un curseur dessiné à la main (ADR 0041).
- `egui_kittest` (MIT OR Apache-2.0) devient une dépendance de développement de `prism-ui`.
