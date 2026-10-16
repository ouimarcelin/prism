# 2026-10-16 – Ticket 19 : tableau de bord et opacité animée

- Branche : `feat/19-dashboard`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : code du projet ; aucune nouvelle source

## Prompts donnés

1. « continues » (suite de la phase 2)
2. « j'ai lancé la commande dans le terminal. » (clé de signature chargée)

## Plan

1. Modèle : 8 curseurs par clip, liaison d'un réglage d'effet à un curseur, opacité de couche animée.
2. Rendu : curseurs du clip dans sa chaîne, opacité animée au mixage.
3. Interface : section « Tableau de bord », menu de liaison par réglage, « ~ » sur l'opacité.

## Résultat

- `prism-core` : `Clip::dashboard`, `Effect::links`, `Layer::opacity_animation`, `Composition::effective_opacity_at`.
- `prism-fx` : `FrameTime::dashboard` ; `values_at` applique l'animation, sinon le curseur, sinon la valeur réglée.
- `prism-engine` : curseurs du clip dans sa chaîne (générateur compris), opacité animée au mixage ; test.
- `prism-ui` : tableau de bord, menu « — / C1…C8 » par réglage (clip, couche, générateur), animation de l'opacité ; vérifié avec `ui-snapshot`.

## Problèmes et points ouverts

- Curseurs sans nom et non animables eux-mêmes ; à revoir avec le MIDI (phase 3).
- Enveloppes (déclenchées au lancement d'un clip) : pas encore.
- Disque presque plein (5,5 Go libres) et dossier `target/` de 20 Go dans un dossier synchronisé par iCloud : un commit a échoué une fois sur une écriture trop lente.
- Un message de commit a été mal formé par une erreur de script, puis corrigé avant tout push.

## Archive

- Transcription : à archiver en fin de session.
