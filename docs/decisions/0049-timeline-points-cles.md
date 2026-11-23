# 0049 – Timeline à points clés

- Date : 2026-11-23
- Statut : Acceptée

## Contexte

@ouimarcelin demande une timeline avec des points clés temporels, en plus de l'éditeur nodal et des objets 3D.

## Décision

- **Dans les animations existantes** plutôt qu'un nouveau moteur de lecture : `Shape::Keyframes` et `Animation::keys`. Une piste est l'animation d'un réglage ; sa durée est la période de l'animation (en temps du tempo ou en secondes), en boucle ou une fois par lancement (« 1× »). Tout ce qui lit déjà les animations (effets, générateurs, opacité de couche, moteur) lit les points clés sans changement.
- **Points** : `Keyframe { time, value, ease }`, `time` de 0 à 1 sur le cycle, `value` dans les unités du réglage, `ease` vers le point suivant (linéaire, douce, entrée lente, sortie lente, palier). Au plus 32 par piste, rangés en place (`Keys`) pour que l'animation reste une valeur simple ; enregistrés en liste JSON.
- **Vue Timeline**, onglet du volet du haut : pistes du clip sélectionné (générateur, effets du clip) et de sa couche (opacité, effets) ; « + Piste » anime un réglage (deux points à sa valeur) ; clic sur la piste : un point sur la courbe ; glisser un point : temps et valeur ; clic droit : courbe, valeur exacte, suppression ; tête de lecture des pistes en boucle.

## Conséquences

- Les pistes d'un même clip peuvent avoir des durées différentes ; pas de règle commune en secondes absolues ni de lecture « hors tempo » d'un spectacle entier (la timeline des repères timecode existe à part).
- `PresetContent::Layer` est mis en boîte : une animation pèse plus lourd avec ses points.
