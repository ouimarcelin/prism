# 0035 – API réseau

- Date : 2026-10-18
- Statut : Acceptée

## Contexte

La phase 6 demande une « API REST et WebSocket pour piloter l'application depuis un autre poste ou une tablette » (`docs/SPEC.md`, crate `control`, bibliothèque prévue : `axum`). Une fois activée, l'API est visible de tout le réseau local, souvent partagé lors d'un événement.

## Décision

- **Adresses** : l'API reprend les adresses fixes de l'OSC (`/prism/layer/1/clip/2`, `/prism/column/3`, `/prism/layer/1/opacity`, `/prism/crossfader`, `/prism/tap`, `/prism/resync`, `/prism/stop`…). Une commande devient un `ControlMessage` de source OSC : mêmes règles, mêmes affectations apprises, rien de dupliqué. Sans valeur, la commande est un appui puis un relâchement de bouton ; avec une valeur (0 à 1), c'est un fader.
- **Serveur** : `axum` sur un environnement `tokio` à un seul thread, sur son propre thread ; les commandes passent par le canal des contrôleurs puis réveillent la boucle de l'interface, qui n'est jamais bloquée.
- **Routes** :
  - `GET /api/state` : état en JSON (colonnes, couches avec nom, opacité, bypass, solo, colonne active, noms des clips ; crossfader ; tempo) ;
  - `POST /api/control` avec `{"address": "...", "value": 0.5}` : réponse 204, ou 400 hors de `/prism/` ;
  - `GET /api/ws` : WebSocket, reçoit les mêmes commandes et envoie l'état à la connexion puis à chaque changement ;
  - `GET /` : page de télécommande pour tablette (grille des clips, colonnes, opacités, bypass, solo, crossfader, tap), intégrée au binaire, sans ressource externe.
- **Sécurité** : choix de l'utilisateur, un code d'accès. Prism tire au hasard un code de 6 caractères (sans 0, O, 1, I, L) à la première activation ; il s'affiche dans Contrôle > API réseau, avec « Nouveau code ». Le code passe en en-tête `Authorization: Bearer CODE` ou en paramètre `?code=` (WebSocket, navigateurs) ; comparaison sans tenir compte de la casse, en temps constant. La page elle-même est servie sans code et le demande.
- **Réglages** : désactivée par défaut, port 8080.

## Conséquences

- HTTP en clair : le code circule sans chiffrement sur le réseau local. Suffisant contre un appareil curieux sur le Wi-Fi d'une salle, pas contre une écoute du réseau. HTTPS serait un ajout ultérieur.
- L'état est publié à chaque image de l'interface, mais envoyé aux clients seulement quand il change.
- Les commandes reçues en arrière-plan sont appliquées au prochain réveil de la boucle ; macOS peut ralentir une fenêtre cachée.
