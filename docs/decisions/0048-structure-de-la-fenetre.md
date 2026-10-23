# 0048 – Structure de la fenêtre de contrôle

- Date : 2026-10-23
- Statut : Acceptée (remplace la disposition grille + inspecteur à droite)

## Contexte

@ouimarcelin a donné un schéma : les calques en haut sur toute la largeur ; en bas, quatre colonnes : moniteurs (sortie au-dessus, aperçu dessous), réglages, réglages d'effets, banque d'effets. Choix faits avec lui : l'aperçu montre le clip de la case sélectionnée avant de le lancer ; les réglages regroupent tout sauf les effets ; la banque contient effets et générateurs, à glisser ; toutes les séparations se tirent et les tailles sont gardées.

## Décision

- **Volets** dessinés chacun dans une zone fixe (`new_child`, contenu coupé au bord) : un contenu trop large ne déplace plus la mise en page (voir ticket 106). Tailles en parts de la fenêtre dans `Preferences::layout` (`Layout` : hauteur des calques, quatre colonnes, part de la sortie), enregistrées à la fin d'un glisser. Les séparations suivent le pointeur depuis la pression (`follow_drag`).
- **Programme** : l'image du programme. **Aperçu** : le moteur dessine à part la case sélectionnée (`Command::PreviewSlot`), avec son propre flux du clip (une lecture de plus pour un fichier), son générateur et ses effets, à la moitié de la taille de la composition.
- **Réglages** : groupe, couche, clip (lecture, générateur et ses réglages, tableau de bord). **Effets de la sélection** : chaînes du clip, de la couche, du groupe et de la composition.
- **Banque** : onglets Effets et Générateurs, recherche, familles (`EFFECT_CATEGORIES` en plus de celles des générateurs), aperçus animés dessinés par le moteur (les effets sur un plasma en mouvement), à mi-cadence. Glisser un effet sur une case (son clip), sur l'en-tête d'une couche ou sur une chaîne ; un générateur sur une case vide ou générée ; double-cliquer : vers la sélection.

## Conséquences

- Les aperçus de la banque tournent en permanence (environ 72 vignettes de 192 × 108, une image sur deux).
- La fenêtre « Générateurs » reste, ouverte par les boutons « Générateur… ».
