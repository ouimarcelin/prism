# 2026-10-16 – Ticket 18 : générateurs

- Branche : `feat/18-generators`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : code du projet ; bruit de valeurs lissé et motifs géométriques courants, connus de mémoire (ligne générale déjà inscrite dans `docs/SOURCES.md`)

## Prompts donnés

1. « continues » : suite de la phase 2 après les tickets 13 à 17.

## Plan

1. Modèle : clip dessiné par un générateur, sans fichier.
2. `prism-fx` : catalogue de générateurs, effets sans image d'entrée.
3. Moteur : générateur en tête de la chaîne du clip, à la taille de la composition ; réglages relus à chaque image.
4. Interface : créer un générateur dans une case vide, le régler.

## Résultat

- `prism-core` : `MediaSource::Generator`, `Clip::from_generator`, `MediaSource::same_media` (même générateur malgré des réglages différents).
- `prism-fx` : six générateurs (couleur unie, dégradé, lignes, cercles, damier, bruit), dont cinq peuvent bouger au tempo ; planche `gallery` complétée.
- `prism-engine` : flux de clip « généré » sans thread de décodage ; un réglage modifié s'applique à l'image suivante, sans relancer le clip ni déclencher de fondu.
- `prism-ui` : menu « Générateur… » dans une case vide, réglages et animations du générateur dans l'inspecteur ; `ui-snapshot generator`.
- Tests : générateurs sur GPU, moteur (réglage changé sans relance), modèle.

## Problèmes et points ouverts

- Texte et shaders personnalisés ISF : pas encore (police à rasteriser, GLSL à traduire en WGSL).
- Commits bloqués pendant la session : la clé SSH de signature n'était plus chargée dans l'agent ; il faut `ssh-add --apple-use-keychain ~/.ssh/id_ed25519`.

## Archive

- Transcription : à archiver en fin de session.
