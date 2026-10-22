# 0047 – Sélecteur de générateurs avec aperçus animés

- Date : 2026-10-22
- Statut : Acceptée

## Contexte

Avec 54 générateurs, la liste déroulante rendait le choix difficile. @ouimarcelin demande des catégories et un sélecteur avec un panneau de visualisation.

## Décision

- **Familles** dans le catalogue (`GENERATOR_CATEGORIES`, `generator_category`) : simples, bruits, vagues, fluides et matières, motifs, fractales, signal, particules. Un test vérifie que chaque générateur est dans une famille et une seule.
- **Aperçus animés, dessinés par le moteur** plutôt que des vignettes fixes livrées avec l'application : ils montrent le mouvement, restent justes quand un shader change et ne pèsent rien dans le dépôt. Le moteur les dessine seulement pendant que le sélecteur est ouvert (`Command::ShowGeneratorPreviews`) : 192 × 108 pixels chacun, réglages par défaut, une chaîne par générateur (les générateurs à mémoire gardent la leur), dans une seule image publiée comme le programme (`Engine::generator_previews`, emplacement par `preview_rect`). Le reste du temps, rien.
- **Fenêtre « Générateurs »** : recherche, familles avec leur nombre, grande vue du générateur survolé (ou choisi), grille des aperçus ; un clic choisit, un double clic ou « Choisir » applique. Ouverte par « Générateur… » d'une case vide ou par le bouton du générateur dans l'inspecteur. Les shaders ISF et les plugins restent dans le menu « Shader… ».

## Conséquences

- Coût GPU pendant que le sélecteur est ouvert : 54 petites images par image, environ 1,1 million de pixels.
- Les aperçus montrent les réglages par défaut, en gris quand la palette par défaut est le gris.
