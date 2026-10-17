# 2026-10-18 – Ticket 64 : projet sorti d'iCloud

- Branche : `fix/64-hors-icloud`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur

## Prompts donnés

1. « règles le probleme icloud et le deplacement du fichier »

## Constat

- Le projet vivait dans `~/Documents`, synchronisé par iCloud. Disque plein à 97 % : iCloud avait retiré du Mac 2 970 fichiers du projet (copies dans le nuage seulement, retéléchargées à chaque lecture). D'où des commandes git de plusieurs minutes, une écriture coupée (« Operation timed out ») et une fusion échouée (« stash failed »).

## Fait

- Nouvelle copie par clonage depuis GitHub dans `~/Developer/prism` (aucune branche locale non fusionnée ; signature des commits par la configuration globale) ; 310 tests passent.
- Cache de compilation déplacé (30 Go, simple renommage) et redevenu le `target/` habituel : plus besoin de `target.nosync` ni de `.cargo/config.toml` local.
- Fichier `UI3_ Figma's UI Kit (Community).fig` sorti du projet, rangé dans le dossier parent (`~/Documents/Claude/Projects/`).
- `scripts/archive-session.sh` retrouve une session commencée dans l'ancien dossier.
- Mémoire de Claude Code recopiée pour le nouveau dossier.

## Points ouverts

- L'ancien dossier `~/Documents/Claude/Projects/prism` reste en place (copies iCloud, le PDF de référence ignoré par git) : à supprimer par l'observateur une fois le nouveau dossier essayé.
- Disque presque plein : à libérer pour que les compilations continuent.
