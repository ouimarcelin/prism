# Cahier des charges – Clone de Resolume Arena

Oct 13, 2026 · @marcelin

Objectif : un logiciel de VJing et de projection mapping open source, écrit en Rust, qui reproduit le flux de travail de Resolume Arena sans réutiliser son code, son nom ni ses visuels. Nom de travail : **Prism** (à changer si besoin).

## Règles clean room

Le projet s'appuie uniquement sur ce qui est public : le comportement observé du logiciel, son [manuel en ligne](https://resolume.com/support/avenue-arena) et sa [page produit](https://resolume.com/software/avenue-arena). Ces règles protègent le projet juridiquement.

| Autorisé | Interdit |
| --- | --- |
| Observer Resolume en usage normal (version d'essai ou licence) | Décompiler, désassembler ou analyser les binaires de Resolume |
| Lire le manuel et les tutoriels publics | Réutiliser le nom Resolume, Arena, Avenue, les logos, icônes ou captures |
| Reprendre la logique de travail (grille de clips, couches, compositions) | Copier pixel pour pixel l'interface, les polices ou les couleurs de marque |
| Lire et écrire des formats ouverts (HAP, ProRes, H.264, PNG) | Implémenter le codec propriétaire DXV |
| Proposer des raccourcis proches, documentés comme les vôtres | Importer les fichiers de projet .avc de Resolume par rétro-ingénierie |

Les brevets restent un risque même en clean room. Avant une diffusion publique ou commerciale, faites vérifier le projet par un avocat en propriété intellectuelle.

## Fonctions à reproduire

Le socle (phases 1 et 2) suffit pour un set de VJing ; les fonctions Arena (mapping, DMX, timecode) arrivent ensuite. P1 = indispensable, P2 = important, P3 = plus tard.

| Module | Fonctions | Priorité | Phase |
| --- | --- | --- | --- |
| Composition | Grille de clips (couches × colonnes), déclenchement par clic ou clavier, colonnes lancées d'un coup, résolution et fréquence de la composition | P1 | 1 |
| Lecture vidéo | Lecture fluide en avant, en arrière, aléatoire ; vitesse, boucle, ping-pong ; scratch ; points de repère (cue points) | P1 | 1 |
| Formats | HAP, HAP Alpha, HAP Q en priorité ; H.264, ProRes, images fixes, séquences PNG ; canal alpha | P1 | 1 |
| Couches | Opacité, modes de fusion (add, multiply, screen, lighten…), solo, bypass, transitions entre clips avec durée | P1 | 1 |
| Crossfader | Groupes A/B, courbes de fondu, transition au tempo | P1 | 1 |
| Effets | Effets GPU par clip, couche, groupe et composition : flou, couleur, kaléido, distorsion, glitch, transformations ; chaînes réordonnables | P1 | 2 |
| Générateurs | Couleur unie, dégradés, lignes, bruit, texte, shaders personnalisés (format ISF) | P2 | 2 |
| Animation de paramètres | Chaque paramètre animable : LFO, enveloppe, synchro BPM, dashboard de 8 curseurs par clip | P1 | 2 |
| Tempo | BPM manuel, tap tempo, resync, synchro Ableton Link | P1 | 2 |
| Audio | Lecture audio des clips, entrée audio, analyse FFT par bande pour piloter les paramètres | P2 | 2 |
| Contrôle | Mapping MIDI par apprentissage (MIDI learn), OSC entrant et sortant, raccourcis clavier assignables | P1 | 3 |
| Entrées live | Caméras USB, cartes de capture (Blackmagic, AJA), NDI, Spout (Windows), Syphon (macOS) | P2 | 3 |
| Sorties | Plein écran multi-écrans, sortie NDI, Spout, Syphon, aperçu et sortie séparés | P1 | 3 |
| Sortie avancée (mapping) | Écrans virtuels, découpes (slices) depuis la composition, déformation par grille et Bézier, masques, transformations d'entrée et de sortie | P1 | 4 |
| Edge blending | Fondu doux entre plusieurs projecteurs, réglage gamma et largeur de zone | P1 | 4 |
| Pixel mapping DMX | Envoi des couleurs vers des luminaires DMX via Art-Net et sACN, gestion des univers et des fixtures | P2 | 5 |
| Pilotage par console | Réception DMX pour déclencher clips et paramètres depuis une console lumière | P2 | 5 |
| Timecode | Lecture SMPTE / LTC et MIDI timecode, clips calés sur le timecode | P2 | 5 |
| API réseau | API REST et WebSocket pour piloter l'application depuis un autre poste ou une tablette | P3 | 6 |
| Plugins | Format de plugin ouvert (FFGL ou interface Rust) pour ajouter des effets | P3 | 6 |
| Projet | Sauvegarde en format ouvert (JSON), presets d'effets et de couches, préférences, gestion des médias manquants | P1 | 1 |

## Architecture technique

Un projet Rust découpé en crates (bibliothèques internes), avec le rendu entièrement sur GPU. Règle d'or : le moteur de rendu tourne à 60 images/s sur son propre thread, et l'interface ne doit jamais le bloquer.

| Crate | Rôle | Bibliothèques open source |
| --- | --- | --- |
| `core` | Modèle de données : composition, couches, clips, paramètres, sauvegarde JSON | `serde`, `serde_json` |
| `media` | Décodage vidéo et images sur threads dédiés, cache de frames | `ffmpeg-next`, décodeur HAP (Snappy + textures DXT/BC) |
| `render` | Mixage des couches, modes de fusion, effets en shaders WGSL | `wgpu` |
| `fx` | Bibliothèque d'effets et de générateurs, chargement de shaders ISF | `wgpu`, `naga` |
| `output` | Fenêtres plein écran, sortie avancée (slices, warping, edge blending) | `winit`, `wgpu` |
| `clock` | BPM, tap tempo, Ableton Link, timecode | `rusty_link`, décodeur LTC |
| `audio` | Entrée et lecture audio, FFT | `cpal`, `rustfft` |
| `control` | MIDI, OSC, raccourcis, API réseau | `midir`, `rosc`, `axum` |
| `dmx` | Art-Net et sACN, pixel mapping | `artnet_protocol`, `sacn` |
| `io-video` | NDI, Spout, Syphon, cartes de capture | SDK NDI (licence gratuite), bindings Spout/Syphon |
| `ui` | Interface : grille de clips, panneaux, éditeur de sortie avancée | `egui` |

Cibles : macOS 13+ d'abord (Apple Silicon et Intel, rendu Metal via wgpu), Windows 10+ ensuite, Linux en dernier. Le code reste multiplateforme dès le départ : tout ce qui est propre à macOS (Syphon, fenêtres, capture) est isolé derrière une interface commune. Matériel de référence pour les tests : 4 couches HAP 1080p à 60 images/s sur un GPU milieu de gamme.

## Plan de développement

Six phases, chacune livrée seulement quand son critère de validation passe. Ne lancez pas la suivante avant.

| Phase | Contenu | Critère de validation |
| --- | --- | --- |
| 1. Moteur de base | Composition, 2 à 4 couches, lecture HAP et H.264, modes de fusion, crossfader, sortie plein écran, sauvegarde JSON | 4 clips HAP 1080p mixés à 60 images/s sans saccade pendant 30 min |
| 2. Effets et tempo | 15 à 20 effets GPU, générateurs, animation des paramètres, BPM et tap tempo, FFT | Un effet cadencé sur le BPM reste synchrone après 10 min |
| 3. Contrôle et flux | MIDI learn, OSC, raccourcis, entrées caméra, NDI, Syphon | Un set complet joué au contrôleur MIDI sans souris |
| 4. Mapping | Sortie avancée : écrans, slices, warping, masques, edge blending | Projection sur 2 projecteurs avec zone de recouvrement invisible |
| 5. Lumière et timecode | Pixel mapping DMX, réception DMX, timecode SMPTE | Une bande LED Art-Net suit l'image ; un clip démarre au bon timecode |
| 6. Finition | Portage Windows (Spout, cartes de capture, DirectX 12 via wgpu), API réseau, plugins, presets, performance, installeurs signés | Test en conditions réelles sur un vrai événement |

À chaque phase : Claude Code écrit le code et les tests automatiques, vous testez à la main avec vos propres médias, puis vous lui décrivez précisément ce qui ne va pas.

## Diffusion publique

Le logiciel sera publié en open source sur GitHub, d'abord pour macOS. Points à respecter dès la première version :

- Licence double MIT / Apache 2.0, fichiers LICENSE et NOTICE à la racine du dépôt
- Mention visible « projet indépendant, non affilié à Resolume » dans le README et la fenêtre « À propos », sans utiliser la marque dans le nom ni le logo
- Application macOS signée et notarisée par Apple, sinon Gatekeeper bloque l'ouverture
- Aucune vidéo, capture ou preset issu de Resolume dans le dépôt, la doc ou les démos : uniquement vos propres médias ou des médias libres
- Contributions extérieures acceptées avec une signature DCO (Developer Certificate of Origin), pour garantir que personne n'injecte de code copié
- Historique clean room conservé : le cahier des charges et les prompts versionnés dans le dépôt servent de preuve de développement indépendant

## Méthode de travail

Un seul principe : chaque morceau de travail suit le même cycle, du ticket à la version publiée, et rien n'existe s'il n'est pas dans GitHub. Les phases du plan deviennent des jalons, les fonctions du tableau deviennent des tickets.

### Outils et organisation du dépôt

| Élément | Rôle | Mise en place |
| --- | --- | --- |
| Dépôt GitHub | Code, historique, preuve clean room | Branche `main` protégée : aucun ajout sans pull request |
| GitHub Issues | Un ticket par fonction ou bug, avec critère d'acceptation | Étiquettes : `phase-1`…`phase-6`, `bug`, `feature`, `P1`/`P2`/`P3` |
| GitHub Milestones | Une étape par phase du plan | Chaque ticket rattaché à sa phase |
| GitHub Projects | Tableau Kanban : À faire · Cette semaine · En cours · À tester · Terminé | Vue unique pour savoir où on en est |
| GitHub Actions | Vérification automatique à chaque modification | Sur un runner macOS : `cargo fmt --check`, `cargo clippy`, `cargo test`, compilation release |
| `CLAUDE.md` | Règles permanentes pour Claude Code | Règles clean room, conventions de code, commandes de test, structure des crates |
| `docs/SPEC.md` | Ce cahier des charges | Mis à jour quand une décision change |
| `docs/decisions/` | Un fichier court par choix technique (ADR) | Exemple : « 0003 – egui plutôt que Slint pour l'interface » |
| `docs/journal/` | Journal daté de chaque session Claude Code | Prompt donné, résultat, problèmes rencontrés |
| `CHANGELOG.md` | Liste des changements par version | Mis à jour à chaque pull request |

### Le cycle d'un ticket

1. **Choisir** un ticket dans la colonne « Cette semaine » et le passer « En cours ».
2. **Créer une branche** au nom du ticket, par exemple `feat/12-crossfader`.
3. **Lancer une session Claude Code** dédiée à ce seul ticket : coller le ticket et son critère d'acceptation, demander un plan avant le code.
4. **Valider le plan**, puis laisser Claude coder par petites étapes, avec un commit à chaque étape qui compile et passe les tests.
5. **Tester à la main** avec vos propres médias, en suivant le critère d'acceptation du ticket.
6. **Ouvrir une pull request** : la CI doit être verte, relire le diff (ou demander à Claude une revue critique dans une nouvelle session).
7. **Fusionner** dans `main`, fermer le ticket, noter la session dans `docs/journal/`.

### Rythme hebdomadaire

| Moment | Durée | Contenu |
| --- | --- | --- |
| Lundi : planification | 30 min | Choisir 3 à 5 tickets pour la semaine, vérifier qu'ils ont un critère d'acceptation clair |
| Chaque session | 1 à 3 h | Un ticket, une branche, une session Claude Code, une note de journal |
| Vendredi : démo | 30 min | Lancer la dernière version de `main` avec un vrai set, noter les bugs en tickets |
| Vendredi : bilan | 15 min | Ce qui a marché, ce qui a bloqué, ce qu'on change la semaine suivante |
| Fin de phase | 1 demi-journée | Vérifier le critère de validation de la phase, publier une version |

### Définition de « terminé »

Un ticket n'est terminé que si toutes ces cases sont vraies :

- [ ] Le critère d'acceptation du ticket est vérifié à la main
- [ ] La CI est verte (format, clippy sans avertissement, tests)
- [ ] Des tests automatiques couvrent la nouvelle logique
- [ ] Aucune baisse de performance sur le benchmark 4 couches 1080p
- [ ] Le `CHANGELOG.md` et, si besoin, la doc utilisateur sont à jour
- [ ] La session est notée dans le journal

### Règles pour travailler avec Claude Code

- **Un ticket par session.** Repartir d'une session propre évite que Claude mélange les sujets.
- **Plan d'abord, code ensuite.** Demander systématiquement un plan, le corriger, puis lancer l'écriture.
- **Commits fréquents.** Si une étape casse tout, on revient au commit précédent au lieu de réparer à l'aveugle.
- **Toujours relire le diff** avant de fusionner : vous restez responsable du code publié.
- **Bugs précis.** Décrire ce que vous avez fait, ce que vous attendiez, ce qui s'est passé, avec une capture ou une vidéo.
- **Rien de Resolume dans les prompts** : pas de capture d'interface ni d'extrait de fichier, seulement des descriptions de comportement.

### Versions

Numérotation `0.x` pendant le développement : `0.1` à la fin de la phase 1, `0.2` à la fin de la phase 2, et ainsi de suite. La `1.0` sort quand la phase 6 est validée en conditions réelles. Chaque version est publiée dans GitHub Releases avec une application macOS signée et notarisée. Dès la `0.3`, ouvrez un petit groupe de testeurs VJ pour avoir des retours de terrain.

## Prompt de démarrage pour Claude Code

Copiez ce texte dans Claude Code, dans un dossier vide, avec ce cahier des charges enregistré à côté sous le nom `SPEC.md`.

```markdown
Tu développes Prism, un logiciel open source de VJing et de projection mapping en Rust (licence MIT/Apache 2.0).
Le cahier des charges complet est dans SPEC.md : lis-le entièrement avant d'écrire du code.

Règles absolues (clean room) :
- Tu ne consultes ni ne reproduis aucun code, binaire, fichier ou visuel de Resolume.
- Tu travailles uniquement à partir de SPEC.md et de documentations publiques.
- Aucun nom, logo ou élément graphique de Resolume dans le code, l'interface ou la doc.

Ce que je veux maintenant : la PHASE 1 uniquement.
1. Crée un workspace Cargo avec les crates core, media, render, output, ui.
2. Implémente une composition de 4 couches, la lecture de fichiers HAP et H.264,
   les modes de fusion add, multiply, screen et normal, un crossfader A/B,
   une sortie plein écran sur un second écran, et la sauvegarde du projet en JSON.
   Cible : macOS (backend Metal de wgpu), mais garde le code multiplateforme pour un portage Windows ultérieur.
3. Écris des tests automatiques pour le modèle de données et le décodage.
4. Ajoute un benchmark qui mesure le nombre d'images/s avec 4 couches 1080p.
5. Rédige un README avec les commandes pour compiler et lancer.

Avance par petites étapes, compile et teste après chacune, et montre-moi
le résultat avant de passer à la suivante. Si un choix technique est ambigu,
propose deux options et demande-moi.
```

Pour les phases suivantes, reprenez le même prompt en remplaçant le bloc « PHASE 1 » par le contenu de la phase visée.

## Points ouverts

- [ ] Nom définitif du logiciel (vérifier qu'il n'est pas déjà déposé à l'INPI ou à l'EUIPO)
- [x] Diffusion publique décidée : faire vérifier le nom, l'interface et les brevets par un avocat en propriété intellectuelle avant la première version publique
- [x] Plateforme décidée : macOS d'abord, Windows à long terme. Reste à ouvrir un compte Apple Developer (99 $ par an) pour signer et notariser l'application
- [ ] Matériel de test disponible : nombre d'écrans et de projecteurs, contrôleur MIDI, bande LED Art-Net
- [ ] Budget d'abonnement Claude pour les sessions de code (le projet demandera de nombreuses heures d'agent)

## Sources

- [Resolume Avenue & Arena – page produit](https://resolume.com/software/avenue-arena)
- [Resolume – manuel et articles d'aide](https://resolume.com/support/avenue-arena)
