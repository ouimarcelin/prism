# 0024 – Syphon

- Date : 2026-10-17
- Statut : Acceptée (choix de l'intégration par l'utilisateur : code source dans le dépôt)

## Contexte

La phase 3 demande Syphon (macOS), pour partager l'image avec d'autres applications (mapping, diffusion, enregistrement), en sortie puis en entrée.

## Décisions

- **Code de Syphon dans le dépôt** (`third_party/syphon`, BSD 3 clauses, choix de l'utilisateur) : serveur et client de base, messagerie, client Metal. Ni OpenGL ni le serveur Metal de Syphon : ses shaders demandent Xcode complet pour être compilés.
- **Crate `prism-io-video`** : `build.rs` compile ces sources et la glue de Prism avec `cc` (ARC, en-tête préfixe de Syphon).
- **Serveur** : une sous-classe de `SyphonServerBase` (glue de Prism) copie une texture BGRA 8 bits dans la surface IOSurface du serveur par une copie GPU (le chemin rapide du serveur Metal de Syphon), puis publie quand la copie est faite.
- **Avec wgpu** : la sortie Syphon est une sortie du moteur (comme une fenêtre plein écran). Après chaque image, elle convertit l'image programme (16 bits flottants, prémultipliée) en BGRA 8 bits par une passe wgpu, puis passe à la glue le périphérique, la file de commandes et la texture Metal de wgpu (`as_hal`) : la copie suit la conversion sur la même file.
- **Nom** : « Prism ». Menu Sortie > Publier sur Syphon (préférences), désactivé par défaut.

## Options écartées

- **Framework Syphon installé à part** : une étape d'installation de plus (choix de l'utilisateur).
- **Protocole réécrit** : pas de spécification publique en dehors du code ; reprendre le code sous licence est plus sûr.

## Conséquences

- La compilation sur macOS demande les Command Line Tools (déjà nécessaires).
- Couleurs prémultipliées dans l'image publiée ; l'image programme est opaque dans l'usage courant.
- Entrée Syphon (client) : ticket suivant, avec un test aller-retour dans le même processus.
- **Ajout du 2026-10-17 (ticket 34) : entrée Syphon.** Clip de source `Syphon` (application et nom du serveur, les identifiants changeant à chaque lancement). Le client Metal de Syphon donne la texture du serveur ; la glue la copie dans une texture BGRA de wgpu sur la file Metal de wgpu, avant le travail de l'image. Le moteur essaie de se connecter chaque seconde tant que le serveur n'est pas publié (la couche affiche « en attente »), et se reconnecte s'il disparaît. L'annuaire des serveurs reçoit leurs annonces sur la boucle du fil principal : l'application le démarre au lancement. `prism-io-video` ne dépend plus du moteur ; la sortie Syphon est un adaptateur de l'application.

