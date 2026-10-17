# Changelog

Toutes les modifications notables de Prism sont listées ici.
Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/). Numérotation `0.x` pendant le développement (voir `docs/SPEC.md`, section « Versions »).

## [Non publié]

### Modifié

- Flou : 7 lectures par passe au lieu de 13 (filtrage linéaire), de 2,16 à 0,88 ms en 1080p sur M1, même courbe.
- README : état des phases 1 et 2 et mode d'emploi des effets, générateurs, animations et du son.
- Le crossfader occupe toute sa longueur dans la barre du bas.
- L'application garde les actions de toutes les passes d'egui dans une image : un raccourci ou un clic ne se perd plus quand egui refait la mise en page.
- La ligne d'état (cadence, saccades, images sautées, images en retard) a sa propre ligne sous le crossfader.
- Thread de rendu en ordonnancement temps réel sur macOS : réveillé à l'heure même quand d'autres processus occupent les cœurs ; threads de décodage en classe « initiée par l'utilisateur » (ADR 0012).
- Trames envoyées au GPU par des tampons réutilisés d'une image à l'autre : plus d'allocation de mémoire GPU pendant la lecture ; travail moyen par image de 3,5 à 2,3 ms avec 4 couches HAP 1080p (ADR 0012).
- Au plus 3 images en attente sur le GPU : au-delà, le moteur saute un tour au lieu d'empiler des images et leurs tampons ; images sautées et latence du GPU dans l'état du moteur (ADR 0012).
- Mixage de quatre couches par passe, écrit directement dans l'image programme : temps GPU d'une image de 3,7 à 2,2 ms avec 4 couches HAP 1080p, empreinte mémoire réduite de 40 Mo (ADR 0013).
- Barre d'état de la fenêtre de contrôle : saccades, images sautées (GPU occupé) et images en retard (décodage), avec une infobulle.
- Interface allégée au repos (30 images/s) : de 67 % à 50 % d'un cœur avec 4 couches HAP 1080p.
- Plus de menu macOS standard (Masquer, Cmd+H) : Cmd+Q passe par l'interface.
- Le thread de décodage ne décode plus deux fois une image déjà livrée.
- Noms par défaut des couches et des colonnes en français.
- `cargo-deny` ne vérifie que les plateformes livrées (macOS, Windows).
- wgpu 30 au lieu de 26, Rust 1.95 minimum, en vue de l'interface egui 0.36 (ADR 0005).
- CI : seules les exécutions de pull request remplacées sont annulées ; chaque push sur `main` garde sa propre exécution complète.

### Ajouté

- Raccourcis clavier assignables : une touche s'apprend comme une commande MIDI et agit aussi depuis la sortie plein écran (ADR 0021).
- OSC : écoute UDP (port réglable) avec adresses fixes `/prism/…` et apprentissage comme le MIDI ; envoi des valeurs (crossfader, tempo, couches) vers une tablette ; menu Contrôle (ADR 0022).
- Contrôle MIDI : apprentissage (menu Contrôle > Apprentissage MIDI, cliquer une commande puis bouger le contrôleur), fenêtre des correspondances, entrées MIDI à écouter ; clips, colonnes, couches (opacité, bypass, solo, arrêt), crossfader, tap, resync, tout arrêter, curseurs du tableau de bord, réglages et activation des effets ; correspondances enregistrées dans le projet (crate `prism-control`, ADR 0021).
- Outil `fx-bench` : temps GPU de chaque effet et générateur en 1080p ; rapport `docs/benchmarks/2026-10-17-phase-2-effets.md`.
- L'opacité d'une couche peut suivre une bande du son (menu à côté de « ~ »), aussi enregistrée dans les préréglages de couche.
- Son des clips : la piste son des vidéos est jouée sur la sortie du système, calée sur la tête de lecture (vitesse, sens, boucles, scratch), au volume du clip multiplié par l'opacité de sa couche ; fondu pendant les transitions ; « Son des clips » et volume général dans le menu Audio, volume par clip dans l'inspecteur (ADR 0020).
- Préréglages : une chaîne d'effets ou les réglages d'une couche (opacité, fusion, crossfader, transition, effets) enregistrés sous un nom et réutilisés dans tout projet (menus « Préréglages… »), fichiers JSON dans le dossier `Préréglages` à côté des préférences.
- Entrée audio (menu Audio : appareil, sensibilité) mesurée en quatre bandes (niveau, graves, médiums, aigus), vumètres à côté du tempo ; tout réglage d'effet peut suivre une bande (« Son : graves »…) sur toute sa plage (crate `prism-audio`, ADR 0019). Exemple `audio-meter`.
- Shaders ISF : un fichier `.fs` ajouté comme effet ou comme générateur (« Shader ISF… »), ses entrées devenues réglages animables ; fichier relu dès qu'il est enregistré ; deux exemples dans `examples/isf` (ADR 0018). Jusqu'à 16 réglages par effet.
- Générateur de texte : texte sur plusieurs lignes saisi dans l'inspecteur, police Ubuntu Light ou Hack, alignement, taille, position et couleur réglables et animables (ADR 0017).
- Enveloppes : une animation réglée « une fois » joue un seul cycle à chaque lancement du clip, puis garde sa dernière valeur (montée avec la rampe, aller-retour avec le triangle ou le sinus).
- Tableau de bord de 8 curseurs par clip ; un réglage d'effet du clip ou de sa couche (et d'un générateur) peut suivre un curseur sur toute sa plage. Opacité des couches animable (« ~ »).
- Générateurs : clips dessinés par le GPU sans fichier (couleur unie, dégradé, lignes, cercles, damier, bruit), créés depuis une case vide de l'inspecteur, réglables et animables comme les effets ; lignes, cercles, damier, bruit et dégradé peuvent bouger au tempo ; un réglage modifié ne relance pas le clip.
- Animation des réglages d'effet (bouton « ~ ») : sinus, triangle, rampe, carré ou aléatoire, période en temps au tempo ou en secondes, entre deux valeurs ; calculée depuis la position en temps, sans dérive (ADR 0016).
- Douze effets de plus, 18 en tout : pixellisation, postérisation, seuil, contours, décalage RVB, glitch, vague, tourbillon, mosaïque, vignette, stroboscope, colorisation ; glitch, vague et stroboscope suivent le tempo. Exemple `gallery` : planche de tous les effets sur une mire.
- Réglage des effets dans l'inspecteur : sections repliables pour la composition, la couche et le clip ; activer, monter, descendre, retirer, ajouter un effet du catalogue ; curseurs et listes de réglages, clic droit pour la valeur par défaut.
- Outil `ui-snapshot` : la fenêtre de contrôle dessinée dans une image PNG, pour vérifier la mise en page.
- Moteur d'effets (crate `prism-fx`) : chaînes d'effets GPU sur le clip, la couche et la composition, enregistrées dans le projet ; six premiers effets : couleur, inversion, transformation, miroir, kaléidoscope, flou (ADR 0015). Réglage dans l'interface au ticket suivant.
- Tempo de la composition (20 à 999 BPM, enregistré dans le projet), tap tempo (bouton et touche T), resync (bouton et touche R), voyants des temps de la mesure ; horloge de tempo sans dérive dans le moteur (crate `prism-clock`) ; fondu automatique du crossfader en temps, au tempo (ADR 0014).
- Outil `gpu-bench` : temps GPU d'une image du moteur, sans fenêtre.
- Benchmark et test d'endurance de la phase 1 (`docs/benchmarks/`) : `engine-run` tient la durée voulue, note chaque incident (saccade, images en retard) avec la pagination de la machine, relève l'empreinte mémoire et le nombre de tampons GPU, et écrit un rapport Markdown ; état du moteur enrichi (pire écart entre deux images, saccades, durée de chaque étape d'une image) ; clips de bruit HAP et HAP Q, le contenu le plus lourd, dans `make-test-clip`.
- Cmd+Q avec confirmation des modifications, écrans maintenus allumés pendant la sortie, préférences (projets récents, écran de sortie, durée du fondu), crossfader automatique vers A ou B (ADR 0011).
- Lecture H.264, ProRes et des autres codecs de macOS par AVFoundation, décodage matériel, conversion YUV sur le GPU ; 4 couches H.264 1080p60 tenues sur Apple M1 (ADR 0010). Outil `decode-speed`.
- Images fixes (PNG, JPEG) et séquences PNG (dossier joué dans l'ordre des numéros, fréquence réglable) : import par dialogue, glisser-déposer ou ligne de commande ; tous les formats passent par une même interface de source d'images (ADR 0009).
- Six modes de fusion : éclaircir, assombrir, différence, soustraction, incrustation, exclusion.
- Interface de contrôle egui (crate `prism-ui`) : grille de clips, déclenchement à la souris et au clavier, réglages des couches, crossfader, propriétés du clip (sens, vitesse, fin, entrée et sortie, scratch, points de repère), aperçu du programme, réglages de la composition, choix de l'écran de sortie, fichiers de projet, import par dialogue ou glisser-déposer, médias manquants (ADR 0008).
- Application `prism` : fenêtre de contrôle avec aperçu, sortie plein écran sur un second écran, vidéos ou projet passés en ligne de commande, raccourcis clavier (colonnes 1 à 9, Espace, F, Échap) ; sorties présentées par le thread de rendu avec synchronisation verticale (crate `prism-output`, ADR 0007).
- Moteur de lecture (crate `prism-engine`) : thread de rendu à la fréquence de la composition, lecture avant, arrière, aléatoire, pause, vitesse, boucle, aller-retour, une fois, points d'entrée et de sortie, scratch, points de repère ; un thread de décodage par clip ; fondu entre clips ; état publié pour l'interface (ADR 0006).
- Outil `engine-run` : lecture de 1 à 4 fichiers HAP en 1080p60 sans fenêtre, avec verdict de fluidité.
- Lecture des fichiers HAP, HAP Alpha, HAP Q et HAP Q Alpha (crate `prism-media`) : lecteur QuickTime/MP4 avec accès direct à toute image, décodeur de trames selon la spécification publique HAP.
- Envoi des trames HAP au GPU sans décompression sur le processeur (textures BC), conversion en texture de couche ; lecture d'un fichier HAP jusqu'au compositeur.
- Outils de test manuel : `make-test-clip` (génère des mires HAP) et `hap-check` (propriétés, vitesse, capture PNG).
- Crate `prism-render` : mixage des couches sur le GPU (wgpu, Metal), opacité, modes normal, add, multiply et screen, crossfader appliqué via la composition ; images intermédiaires en 16 bits flottants ; tests GPU comparés à une référence CPU (ADR 0004).
- Workspace Cargo et crate `prism-core` : composition (résolution, fréquence), grille couches × colonnes, crossfader A/B à trois courbes, réglages de couche (opacité, mode de fusion, solo, bypass, transition), réglages de lecture des clips, déclenchement d'un clip ou d'une colonne.
- Fichier projet `.prism` en JSON versionné : chemins relatifs au dossier du projet, écriture atomique, détection des médias manquants (ADR 0002).
- ADR 0003 : lecture vidéo native (HAP en Rust, VideoToolbox pour H.264), sans FFmpeg.
- Mise en place du dépôt : cahier des charges (`docs/SPEC.md`), règles pour Claude Code (`CLAUDE.md`), double licence MIT / Apache 2.0, `NOTICE`, `README.md`.
- Politique et attestation clean room (`docs/CLEAN_ROOM.md`), registre des sources (`docs/SOURCES.md`), dossiers `docs/observations/`, `docs/decisions/` et `docs/journal/`.
- Guide de contribution avec DCO, modèles de pull request et de tickets.
- CI GitHub Actions : contrôle clean room, vérification DCO, format, clippy, tests et compilation release sur macOS, `cargo-deny`.
