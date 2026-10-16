# 0017 – Générateur de texte

- Date : 2026-10-17
- Statut : Acceptée (prise en mode autonome, à revoir sur demande)

## Contexte

Le cahier des charges liste le texte parmi les générateurs (phase 2). Les générateurs existants sont des shaders sans entrée (ADR 0015, addendum du ticket 18). Dessiner des glyphes demande une police et un rasteriseur.

## Décisions

- **Texte dans l'effet** : `Effect::text`, vide pour les autres effets ; enregistré seulement s'il n'est pas vide.
- **Rasterisation sur le processeur** en un masque de couverture (blanc prémultiplié), envoyé au GPU ; le shader du générateur le place (centre en X et Y), le met à l'échelle (hauteur d'une ligne en fraction de l'image) et le colore. Le masque est refait seulement quand le texte, la police, l'alignement ou la hauteur de ligne changent ; la hauteur de ligne est arrondie à la puissance de deux supérieure (16 à 512 px), si bien qu'une taille animée ne refait le masque qu'en changeant d'octave et que le masque n'est jamais réduit de plus de moitié.
- **Polices** : Ubuntu Light et Hack, déjà embarquées par l'interface (crate `epaint_default_fonts`, licences dans `NOTICE`). Pas de polices du système pour l'instant.
- **Lecture des polices** avec `skrifa` (déjà utilisée par egui), **couverture** avec `ab_glyph_rasterizer`. `ab_glyph` a été essayée puis écartée : elle dépend de `ttf-parser`, déclarée non maintenue (RUSTSEC-2026-0192), que `cargo deny` refuse.
- Mise en page simple : lignes séparées par les retours à la ligne, avances des glyphes sans crénage ni mise en forme complexe ; alignement à gauche, au centre (par défaut) ou à droite. Le masque s'agrandit des deux côtés autant que l'encre déborde (accents, jambages), pour que le bloc reste centré.
- La chaîne d'effets donne le masque comme image source au générateur de texte et sa hauteur de ligne dans un champ `mask_line` des réglages de passe.

## Options écartées

- **Texte dessiné par egui (epaint)** : mise en forme complète, mais atlas de glyphes à la taille de l'interface et dépendance du moteur à la bibliothèque d'interface.
- **Masque à la taille de la composition, placé sur le processeur** : un déplacement animé referait le masque à chaque image.

## Conséquences

- Crénage, écritures complexes (arabe, indien) et polices du système : à ajouter si le besoin est exprimé (`harfrust`, déjà présent via egui, pourrait faire la mise en forme).
- La rasterisation d'un texte court prend moins d'une milliseconde sur le thread de rendu ; un texte très long modifié pendant le jeu peut coûter une image.
