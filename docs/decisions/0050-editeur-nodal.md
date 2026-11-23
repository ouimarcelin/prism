# 0050 – Éditeur nodal (patch)

- Date : 2026-11-23
- Statut : Acceptée

## Contexte

@ouimarcelin demande un éditeur nodal. Remplacer toute la grille de clips par un graphe changerait tout le modèle ; un graphe comme source d'un clip s'ajoute à ce qui existe et s'utilise partout où un clip va (couches, groupes, scène, aperçus).

## Décision

- **Une nouvelle source de clip**, `MediaSource::Patch { patch }`. `Patch` (dans `prism-core`) : nœuds `Generator`, `Effect`, `Mix` (deux entrées, mode de fusion, opacité) et une `Output` unique, câbles `Link` (au plus un par entrée), boucles refusées, ordre de rendu `Patch::order` (seuls les nœuds qui mènent à la sortie). Positions des nœuds enregistrées. Un patch dont le graphe change reste le même clip (`same_media`).
- **Rendu** (`prism-engine`, `PatchRender`) : un nœud générateur ou effet a sa propre `EffectChain` (les générateurs à mémoire gardent la leur), un mélange son `Compositor` et son image ; un effet sans rien en entrée ne dessine rien, un effet passé laisse passer son entrée. Les effets du clip et de la couche s'appliquent ensuite à la sortie, comme pour un générateur.
- **Interface** : onglet « Nodal » du volet du haut, avec la crate `egui-snarl` 0.12 (MIT OR Apache-2.0, compatible egui 0.36) ; le graphe de l'éditeur est reconstruit depuis le patch quand ils diffèrent, chaque action (câbler, ajouter par clic droit, supprimer, régler) va tout de suite au patch. Bouton « Patch… » d'une case vide.

## Conséquences

- Pas encore de nœud « clip » ou « couche » en entrée (images d'autres cases) : un patch fabrique ses images lui-même.
- Les réglages communs de la banque ne sont pas montrés dans les nœuds (seulement les réglages propres).
