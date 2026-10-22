# 2026-10-20 – Tickets 86 à 89 : démarrage, curseurs, groupes de couches

- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Transcription : `2026-10-20-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `17e80530fd5b97a4d1ae663c1fa78b1b51beb528604c0f7d9f41a4d118b2b2bc`

## Prompts

- « augmente grave la valeur du ease-in, supprimes l'animation scale du logo »
- Maquettes de curseur (deux images), « intègre cette nouvelle ui de parametre glissière », puis « tu n'as pas compris, voici le parametre glissière à intégrer… plusieurs états de la glissière »
- « les calques doivent être organisable en drag & drop. les groupes doivent pouvoir se créer en drag & drop. » (ticket 90)
- « je veux pouvoir renommer les calques et les groupes quand je double clic sur leur nom. » (ticket 91)
- « fais : Déformation par Bézier, avec des poignées réglables à la main » (ticket 93)
- « fais en sorte que les led puissent être déplacés avec des courbes de béziers » (ticket 94)
- « je veux intégrer une banque de vidéo généré proceduralement… créé moi un tableau de toutes les vidéos à coder et par catégories. », puis « oui » (ticket 95)
- « listes moi les futures implémentations de features pour le logiciel. », puis « vas y » (groupes de couches)

## Résultat

- Ticket 86 : démarrage en 2 s, ease-in marqué (sinusoïde de t³), logo fixe.
- Tickets 87 et 88 : curseurs à piste arrondie et poignée en capsule blanche logée au bout du remplissage (ADR 0041 révisée).
- Ticket 89 : groupes de couches en sous-mixage (ADR 0043) : modèle, rendu, contrôle, interface, tests du modèle et test GPU.

- Ticket 90 : glisser-déposer des couches et des groupes par une poignée, zones de dépôt (au-dessus, en dessous, grouper), test egui_kittest qui glisse deux couches, capture « glisser ».
- Ticket 91 : renommage sur place par double clic (couches et groupes), test egui_kittest (Entrée garde, Échap annule).
- Ticket 93 : déformation par Bézier à poignées (ADR 0044), tests du modèle, captures « bezier-clair ».
- Ticket 94 : bandes LED en courbe de Bézier, pixels espacés selon la longueur, test du modèle, capture « lumiere-courbe ».
- Ticket 95 : feuille de route `docs/generateurs.md`, socle commun de la banque (ADR 0045), trois premiers générateurs, test de validation naga de tous les shaders, variantes dans `gallery`.

## Problèmes

- Ticket 87 : maquette mal lue (capsule prise pour une marque d'origine) ; corrigé au ticket 88 avec la maquette des états.
- Ticket 88 : un commit parti sans `cargo fmt` (chaîne de commandes avec `;`) ; corrigé dans un second commit.
- Disque plein (1 Gio libre) : l'édition de liens a écrit un binaire tronqué de 3 Ko, installé dans Prism.app, qui ne s'ouvrait plus. `cargo clean --profile dev` (43 Gio), nouvelle édition de liens forcée.
- Ticket 91 : le test du double clic échouait, les images du harnais étant à 0,25 s d'écart (plus que le délai du double clic, 0,3 s, une fois deux passes comptées) : pas de 0,05 s pour ce harnais.
- Ticket 93 : au redimensionnement, les poignées étaient d'abord calculées sur une grille invalide (nouvelles dimensions, anciens points), qui rendait l'identité : calcul sur l'ancienne forme, au pas de la nouvelle grille.
- Ticket 95 : `common` et `shared` sont des mots réservés de WGSL (refusés par naga) ; fonction renommée `bank_setting`.

## Ticket 96 : effets repliables et ordre par glisser-déposer

Prompt : « chaque effet doit pouvoir se replier ; les effets peuvent changer d'ordre par drag&drop ».

Résultat : champ `collapsed` dans `Effect` (absent du fichier quand faux, comme le repli des groupes) ; ligne de titre de l'effet avec poignée, flèche de repli, case d'activation et ×. Le glisser-déposer reste dans une même chaîne (même propriétaire) et montre un trait de dépôt au-dessus ou au-dessous du bloc visé ; il remplace les boutons ⏶ ⏷ (`ChainEdit::Move { from, to }`). Test unitaire du déplacement ; capture `clip` vérifiée.

## Ticket 97 : générateurs de vagues (2026-10-21)

Prompt : « implémentes les effets dont on vient de parler », compris comme la catégorie « Vagues » proposée à la fin du ticket 95.

Résultat : sept générateurs (n° 16, 17, 18, 19, 20, 23, 27 de `docs/generateurs.md`) et la fonction commune `wave_shape`. Vérifiés sur la planche de `gallery` (14 variantes) et par le test naga.

Problème : sur la planche, l'onde carrée en palette néon paraissait unie. Les palettes cosinus reviennent à leur couleur de départ au bout d'un cycle ; corrigé en ne lisant que 80 % du cycle, ce qui change un peu les couleurs des bruits du ticket 95.

## Ticket 98 : changer le générateur d'un clip

Prompt : « fais en sorte qu'une fois un générateur choisi, on ai la possibilité de la modifier ».

Résultat : le titre du générateur dans l'inspecteur devient un menu « Générateur » listant tous les générateurs intégrés. `switch_generator` remplace le générateur ; entre deux générateurs de la banque, il garde les 16 réglages communs avec leurs animations, liens au tableau de bord et bandes du son ; les réglages propres reprennent leur valeur par défaut. Le nom du clip suit s'il était encore celui de l'ancien générateur. Test unitaire ; capture `generateur` vérifiée.

## Ticket 99 : programme séparé des réglages

Prompt : « scindes la fenêtre programme en deux… c'est moi qui décide de la taille de la fenetre avec un drag entre les fenetres ».

Résultat : panneau haut redimensionnable (`egui::Panel::top("program")`, 80 px au minimum, 120 px laissés aux réglages) dans l'inspecteur ; l'image du programme y tient en entier, centrée, à son rapport largeur sur hauteur ; les réglages ont leur propre défilement dessous. Capture vérifiée (sans image du programme dans le harnais de test).

## Ticket 100 : séparation du programme, correctifs

Prompt : « la fenetre du visualisateur est à 100% lorsque l'app se lance. supprimes l'animation bizarre… quand je bouge la taille des fenetres ».

Cause : le panneau `egui::Panel::top` prend la taille de son contenu ; l'image, qui remplissait la hauteur disponible, le faisait grandir jusqu'au maximum, et la boucle taille du panneau / taille de l'image faisait trembler le redimensionnement. Correctif : séparation faite à la main, hauteur gardée dans `ControlPanel::program_height` (240 px au lancement), poignée de 8 px tirée verticalement, sans animation.

## Ticket 101 : tailles de l'inspecteur

Prompt : capture de la taille voulue au lancement ; « le redimensionnement entre les fenetres gauches et droites bug ».

Cause : l'image du programme était dessinée dans un `scope_builder` à `max_rect`, qui ramenait le curseur du parent sous le contenu réel au lieu du bas du volet ; la hauteur du volet ne tenait pas et la mise en page sautait. Correctif : volet dessiné dans un `new_child`, qui ne touche pas au curseur. Au lancement, l'inspecteur prend 42 % de la fenêtre et le volet du programme la hauteur de l'image à cette largeur, jusqu'à ce qu'on tire une séparation. La bordure grille / inspecteur est faite à la main (`exact_size`, bande de 8 px tirée de côté, curseur de redimensionnement, 260 px au minimum, 360 px laissés à la grille). Test kittest du glisser de la bordure.

## Ticket 102 : la bordure suit la souris

Prompt : « le redimensionnement gauche-droite déconne. il bouge plus vite que la sourie ».

Cause : la largeur était mise à jour en ajoutant le déplacement de la souris à chaque passage ; l'interface peut être disposée plus d'une fois par image, le même déplacement comptait alors plusieurs fois. Correctif : au début du glisser, la largeur de départ est gardée ; ensuite largeur = départ + (point de pression − pointeur). Même chose pour la séparation du programme. Le test kittest vérifie qu'un glisser de 100 points déplace la bordure de 100 points.

## Ticket 103 : bordure, départ du glisser gardé par pression

Prompt : « si je bouge de 50 px vers la gauche, la fenêtre bouge de 70 px » (vidéo jointe, illisible : dossier temporaire de macOS refusé ; accès à Prism refusé).

Hypothèse : le départ du glisser relu une seconde fois après un premier déplacement (deux passes d'egui), d'où un décalage égal au premier mouvement. Non reproduite dans les tests, même à 2 pixels par point, avec une image de programme et deux passes par image. Correctif défensif : `follow_drag` garde la valeur de départ avec le point de pression et ne la reprend qu'à une nouvelle pression ; `drag_offset` donne le chemin du pointeur depuis la pression. Le test de la bordure passe aussi avec deux passes et un premier pas de 20 points.

## Ticket 104 : générateurs sans mémoire d'image (2026-10-21)

Prompt : « créé tous les effets qui ne demande pas de memoire d'image, ne fais pas de pause ».

Résultat : 33 générateurs (n° 12 à 53 de `docs/generateurs.md`, hors ◆), chacun un fichier WGSL dans `crates/fx/src/shaders/bank/` et une entrée du catalogue ; `bank_pixel()` ajouté au socle. Tous validés par naga, vérifiés sur la planche de `gallery` (une variante chacun, deux pour marbre / bois).

Corrections après la planche : Lissajous en pointillés (240 échantillons, passés à 600) ; hexagones rayés (identifiant de tuile arrondi au lieu de tronqué) ; Julia réduite à de la poussière (c par défaut à 170°, l'ensemble connexe classique) ; Mandelbrot centré sur l'ensemble entier ; pli fractal replié avant la rotation.

Limites : océan simple (pas de vraies vagues de Gerstner, pas de reflets) ; mire en valeurs, colorée seulement par la palette.

## Ticket 105 : mémoire d'image et générateurs à mémoire

Prompt : « fais la mémoire d'image et les 4 générateurs restants ».

Résultat : ADR 0046. `EffectChain` garde, pour un générateur marqué `memory`, deux textures 16 bits en alternance ; `fs_step` tourne « Pas par image » fois, `fs_main` affiche. Réaction-diffusion (Gray-Scott, laplacien 3 × 3, graines et injection), fluide 2D (advection semi-lagrangienne, viscosité, remous par rotationnel de bruit, couleur versée en trois points), particules dans un flux (image transportée le long d'un rotationnel, étincelles semées, traînées), feux d'artifice (gerbes au tempo dessinées, traînées qui retombent). Test GPU `generators_with_memory_build_on_their_last_image` ; planche de `gallery` après 240 images pour ces quatre.

Problème trouvé en route : `bank_point` avait l'axe y vers le bas ; feu, fumée, aurore, océan, vagues de lignes en perspective et Sierpinski étaient à l'envers. Corrigé dans le socle (y vers le haut).

Premier essai du test de réaction-diffusion raté : à 32 × 18 texels, les graines (1,5 texel) mouraient ; le test tourne en 192 × 108, pleine résolution.

## Ticket 106 : sélecteur de générateurs (2026-10-22)

Prompt : « créé des categories de générateurs et fais en sorte que l'interface de selection des générateurs comporte un panel de visualisateur ».

Résultat : ADR 0047. Familles dans le catalogue ; aperçus animés dessinés par le moteur pendant que le sélecteur est ouvert ; fenêtre « Générateurs » (recherche, familles, grande vue, grille). Test moteur `generator_previews_are_drawn_while_asked_for` (chaque aperçu dessine quelque chose) ; capture `selecteur` avec les vrais aperçus lus du moteur.

Bug trouvé en route, sans doute celui de la bordure « 50 px → 70 px » : avec une case vide sélectionnée (comme au lancement), la rangée des sources était plus large que l'inspecteur ; le volet egui élargissait alors toute la zone de la fenêtre, l'inspecteur sortait à droite et la bordure, placée d'après lui, se décalait. La grille et l'inspecteur sont maintenant dessinés chacun dans une zone fixe (`new_child`, contenu coupé au bord), la bordure placée d'après la largeur demandée ; les sources d'une case vide passent sur deux lignes.
