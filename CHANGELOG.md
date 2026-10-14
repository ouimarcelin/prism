# Changelog

Toutes les modifications notables de Prism sont listées ici.
Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/). Numérotation `0.x` pendant le développement (voir `docs/SPEC.md`, section « Versions »).

## [Non publié]

### Modifié

- Noms par défaut des couches et des colonnes en français.
- `cargo-deny` ne vérifie que les plateformes livrées (macOS, Windows).
- wgpu 30 au lieu de 26, Rust 1.95 minimum, en vue de l'interface egui 0.36 (ADR 0005).
- CI : seules les exécutions de pull request remplacées sont annulées ; chaque push sur `main` garde sa propre exécution complète.

### Ajouté

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
