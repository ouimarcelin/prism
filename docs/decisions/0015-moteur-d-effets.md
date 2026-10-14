# 0015 – Moteur d'effets

- Date : 2026-10-14
- Statut : Acceptée

## Contexte

La phase 2 demande 15 à 20 effets GPU, appliqués par clip, par couche et à la composition, en chaînes réordonnables, et plus tard animés au tempo. Il faut décider où vivent les effets dans le modèle, comment ils s'écrivent et où ils s'insèrent dans le rendu.

## Décisions

- **Modèle** (`prism-core`) : un `Effect` nomme un type (`kind`, par exemple `blur`) et garde ses réglages par nom de paramètre, avec un bypass. Le clip, la couche et la composition ont chacun une liste d'effets, enregistrée dans le projet. Le modèle ne sait pas ce que fait un effet : un type inconnu (projet d'une version plus récente) reste dans le fichier et n'est pas rendu ; un paramètre absent prend sa valeur par défaut. Le format de projet ne change pas de version.
- **Catalogue** (`prism-fx`) : chaque effet décrit son type, son nom affiché, ses paramètres (bornes, défaut, ou choix entre options), son shader et ses passes. L'interface lira ce catalogue pour afficher les réglages.
- **Un effet = un shader WGSL** de fragment après un prélude commun. Il lit l'image jusque-là, ses 8 réglages au plus, la taille de l'image, les secondes écoulées et la position en temps du tempo. Les couleurs restent prémultipliées, dans l'espace d'affichage (ADR 0004). Un effet peut avoir plusieurs passes (le flou en a deux).
- **Chaîne** : `EffectChain` applique les effets actifs l'un après l'autre, une passe plein écran par étape, sur deux images en alternance à la taille de l'image traitée. La dernière passe peut écrire directement dans une destination.
- **Place dans le rendu** : les effets du clip puis ceux de sa couche s'appliquent à l'image de chaque clip joué, avant le mixage ; pendant un fondu, au clip sortant comme au clip entrant. Les effets de la composition s'appliquent à l'image mixée et écrivent dans l'image programme. Sans effet, rien ne change dans le rendu : pas de passe, pas de texture en plus.

## Options écartées

- **Effets au format ISF** (shaders GLSL avec en-tête JSON) dès maintenant. Le cahier des charges les prévoit pour les générateurs et les shaders personnalisés ; ils demanderont une traduction du GLSL vers WGSL. Les effets intégrés restent en WGSL natif, plus simple à tester.
- **Effets de couche appliqués après le mélange du fondu** : plus exact pour un effet non linéaire pendant un fondu, mais il faudrait une texture de plus par couche et sortir le fondu du compositeur. Écart visible seulement pendant un fondu, et seulement pour certains effets.

## Conséquences

- Chaque effet actif coûte une passe plein écran (deux pour le flou) à la taille de l'image traitée, à chaque image, même quand le clip n'a pas changé : les effets animés le demandent.
- Les noms des types d'effets (`color`, `invert`, `transform`, `mirror`, `kaleidoscope`, `blur`…) et de leurs paramètres sont des identifiants de fichier : les changer demande une conversion des projets.
- L'animation des paramètres (ticket 17) modifiera les valeurs avant l'appel du catalogue, sans toucher aux shaders.
