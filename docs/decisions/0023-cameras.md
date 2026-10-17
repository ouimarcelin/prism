# 0023 – Caméras en direct

- Date : 2026-10-17
- Statut : Acceptée (prise en mode autonome, à revoir sur demande)

## Contexte

La phase 3 demande les entrées live : caméras USB, cartes de capture, NDI, Syphon. Sur macOS, les caméras USB, la caméra intégrée et beaucoup de cartes de capture (pilotes compatibles) apparaissent comme des périphériques AVFoundation.

## Décisions

- **Source de clip `Camera`** (identifiant AVFoundation stable, nom au moment du choix), créée depuis une case vide (« Caméra… »). Le clip a ses effets et son tableau de bord ; pas de position, de durée ni de réglages de lecture.
- **Capture AVFoundation** : session, entrée du périphérique, sortie vidéo en 4:2:0 « video range » (comme le décodage H.264, ADR 0010) sur une file de dispatch série, images en retard abandonnées. Le délégué copie chaque image dans une trame qui remplace celle pas encore prise ; le moteur prend la plus récente à chaque image et rend l'ancienne.
- **Ouverture hors du thread de rendu** : démarrer une session prend des centaines de millisecondes. Le moteur l'ouvre sur un thread, montre la couche vide en attendant, puis l'image.
- **Autorisation** : le périphérique est cherché d'abord (une caméra absente est signalée sans demander l'accès). Au premier usage, macOS demande l'accès ; le clip affiche « accès demandé » et doit être relancé une fois l'accès accordé. Refusé : message vers les Réglages Système.
- **Taille** : celle que la caméra livre (son format par défaut) ; la texture suit si elle change.

## Options écartées

- **Choix du format (résolution, cadence) de la caméra** : plus tard, sur demande.
- **Caméra comme décodeur à la demande** (comme un fichier) : une source en direct n'a pas d'image n.

## Conséquences

- L'application empaquetée devra déclarer `NSCameraUsageDescription`.
- NDI et Syphon : tickets à part (SDK et framework tiers à intégrer).
