# 2026-10-18 – Ticket 66 : API réseau

- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Transcription : `2026-10-18-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `b461f0113f6ef009488daf3945ecbe72fc6b83429bda1969cba126e4dca20fbf`

## Prompt

« reprends le developpement du logiciel. » Phase 5 validée ; phase 6 commencée par l'API réseau (choix de l'utilisateur), protégée par un code d'accès (choix de l'utilisateur).

## Plan

- Serveur `axum` dans `prism-control`, sur son propre thread, qui reprend les adresses fixes de l'OSC.
- Routes `GET /api/state`, `POST /api/control`, WebSocket `/api/ws`, page de télécommande `/`.
- Code d'accès de 6 caractères, réglages dans Contrôle > API réseau.

## Résultat

- `crates/control/src/api.rs` et `api_page.html` ; ADR 0035.
- Tests : code d'accès, comparaison, commandes (appui et relâchement, fader borné, adresse refusée), état, routes réelles sur un port libre (401 sans code, 200 avec, commande transmise).
- Essai avec Prism lancé : `curl` (401, état, crossfader et opacité modifiés) puis page dans le navigateur intégré (saisie du code, grille, bypass de la couche 1 aller-retour). Préférences de l'utilisateur sauvegardées avant l'essai puis restaurées.

## Problèmes

- `dataset` ne s'affecte pas par `Object.assign` : la page ne s'affichait pas.
- La grille était reconstruite à chaque message : un bouton pouvait être remplacé sous le doigt ; elle n'est plus reconstruite que si elle change.
- `WebSocket::close` n'existe pas sans `futures` : envoi d'un message `Close`.
- Fenêtre de Prism en arrière-plan : les commandes s'appliquent avec un peu de retard (réveil de la boucle ralenti par macOS).
