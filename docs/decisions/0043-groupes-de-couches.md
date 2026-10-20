# 0043 – Groupes de couches

- Date : 2026-10-20
- Statut : Acceptée

## Contexte

Le cahier des charges prévoit des effets « par clip, couche, groupe et composition » ; les groupes manquaient. @ouimarcelin a choisi, parmi deux options proposées, le **sous-mixage** (plutôt que de simples réglages partagés) et un **en-tête de groupe dans la colonne des couches** (plutôt qu'une fenêtre à part).

## Décision

- **Modèle** (`prism-core`) : `Group` (nom, opacité, mode de fusion, bypass, solo, effets, replié) dans `Composition::groups` ; chaque couche porte `group: Option<GroupId>`. Les couches d'un groupe restent voisines dans la pile : refus au chargement sinon, et chaque modification (`group_with_above`, `remove_from_group`, `move_layer`, `remove_layer`) garde un groupe d'un seul tenant et retire les groupes vides. Une couche retirée du milieu d'un groupe passe juste au-dessus ; une couche déplacée entre deux couches d'un groupe le rejoint. Grouper deux couches de groupes différents fusionne dans le groupe du bas.
- **Rendu** (`prism-engine`) : les couches visibles d'un groupe sont mélangées dans une image à elles, partant du noir transparent (`Compositor::over_transparent`), puis passent par les effets du groupe ; cette image entre dans le mélange principal comme une couche, avec l'opacité et le mode de fusion du groupe. Coût : une image à la taille de la composition et une passe par groupe, plus ses effets.
- **Bypass et solo** du groupe s'appliquent à ses couches ; s'il y a un solo quelque part, seules les couches en solo et celles des groupes en solo sont visibles.
- **Contrôle** : `EffectOwner::Group` ; les réglages des effets d'un groupe s'apprennent en MIDI ou OSC comme les autres.
- **Interface** : ligne d'en-tête au-dessus des couches du groupe (repli, nom, B, S, opacité ; clic droit : renommer, dissoudre), couches du groupe décalées avec une barre bleue ; menu d'une couche : « Grouper avec la couche du dessus », « Retirer du groupe ». Section « Groupe » dans l'inspecteur quand la couche choisie est groupée.

## Conséquences

- Les projets sans groupe ne changent pas (`groups` et `group` omis à l'enregistrement).
- Pas encore : opacité de groupe animée ou suivant le son, cibles MIDI propres au groupe (opacité, bypass, solo), préréglages de groupe ; l'aperçu de référence (`plan_layers`) ignore les groupes.
