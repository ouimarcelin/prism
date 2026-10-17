# 2026-10-17 – Ticket 34 : entrée Syphon

- Branche : `feat/34-syphon-input`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : framework Syphon (déjà repris, ticket 33)

## Prompts donnés

1. « parfait, commence les intégrations » (suite de la phase 3)

## Plan

1. Glue : annuaire, liste des serveurs, client Metal, copie vers une texture de wgpu.
2. `prism-io-video` sans dépendance au moteur ; test aller-retour.
3. Modèle et moteur : source `Syphon`, connexion et reconnexion.
4. Interface : « Syphon… » ; application : annuaire au lancement, adaptateur de sortie.

## Résultat

- `prism-io-video` : `SyphonServer`, `SyphonClient`, `syphon_servers`, `start_syphon_directory` ; test : image orange publiée puis relue.
- `prism-core` : `MediaSource::Syphon`, `Clip::from_syphon`.
- `prism-engine` : phase Syphon des flux, `syphon_servers()` ; test d'un serveur absent attendu.
- `prism-ui`, `prism-app`.
- ADR 0024 complété.

## Problèmes et points ouverts

- La découverte des serveurs d'autres applications (annonces sur la boucle principale) n'est pas testée automatiquement : à vérifier avec une vraie application qui publie.

## Archive

- Transcription : `2026-10-17-094428-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `860b332998b62d546ba9aee54dc3c673e16a5dc99c0da4da9e83931da50b0863` (même session, archivée après le ticket 34 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
