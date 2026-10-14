# 0004 – Espace de mixage et pipeline de composition

- Date : 2026-10-13
- Statut : Acceptée ; la passe par couche est remplacée par l'ADR 0013 (quatre couches par passe)

## Contexte

Le moteur de rendu doit mélanger jusqu'à 4 couches 1080p, et plus tard davantage, à 60 images/s, avec opacité, modes de fusion et crossfader. Il faut choisir l'espace de couleur du mixage, la précision des images intermédiaires et la façon d'appliquer les modes de fusion.

## Options

1. **Mixage en lumière linéaire.** Physiquement juste pour les fondus. Mais les modes multiply et screen n'y ont pas l'aspect attendu par les graphistes, habitués aux logiciels d'image qui mélangent les valeurs telles qu'affichées.
2. **Mixage dans l'espace d'affichage** (valeurs encodées gamma, comme stockées dans les vidéos). Les modes de fusion se comportent comme dans les logiciels d'image courants, et les vidéos n'ont besoin d'aucune conversion à l'entrée ni à la sortie.

Pour l'application des modes : par le mélange fixe du GPU (rapide, mais limité aux modes simples), ou par un shader qui lit le résultat des couches inférieures (tous les modes possibles, y compris overlay ou difference plus tard).

## Décision

- Mixage dans l'**espace d'affichage** (option 2). Les textures des couches sont dans des formats non-sRGB, lues telles que stockées.
- Couleurs **prémultipliées par l'alpha** du début à la fin.
- Images intermédiaires en **`Rgba16Float`** : précision sur de nombreuses couches, et le mode add peut dépasser 1.0 sans être écrêté avant la sortie.
- **Une passe par couche** sur deux textures en alternance : le shader lit le résultat des couches inférieures et écrit le nouveau résultat. Fond noir opaque.
- Formules de mélange : forme générale du compositing « source over » avec fonction de mélange B(Cb, Cs), comme décrite dans la spécification publique du W3C *Compositing and Blending Level 1*. Add (B = Cb + Cs) n'y figure pas et suit la même forme.
- Une implémentation CPU de référence (`crates/render/src/reference.rs`) reproduit exactement le shader ; les tests comparent les deux.
- wgpu 26 : dernière version compatible avec la version minimale de Rust déclarée (1.85). Montée de version dans un ticket dédié.

## Conséquences

- La sortie (écran, NDI, Syphon) doit écrêter les valeurs au-delà de 1.0.
- Ajouter un mode de fusion revient à ajouter une ligne au shader et à la référence CPU, avec son test.
- Un clip dont la résolution diffère de la composition est étiré pour la remplir, en attendant les transformations de couche.
