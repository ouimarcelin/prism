# 0025 – Sortie plein écran sur plusieurs écrans

- Date : 2026-10-17
- Statut : Acceptée

## Contexte

Le cahier des charges (module Sorties, phase 3) demande le plein écran multi-écrans. Jusqu'ici, l'application n'ouvrait qu'une fenêtre de sortie : choisir un autre écran fermait la première. La sortie avancée (écrans virtuels, slices, phase 4) n'est pas encore là : chaque écran reçoit donc l'image programme entière.

## Décision

- L'application garde une liste de fenêtres de sortie, au plus une par écran. Chacune est une sortie du moteur (`Command::AddOutput`), présentée par le thread de rendu comme avant (ADR 0007).
- Menu Sortie : une case à cocher par écran ouvre ou ferme la sortie sur cet écran. « Fermer la sortie » (ou « Fermer les sorties ») et `Échap` les ferment toutes.
- `F` ouvre les sorties sur les écrans utilisés la dernière fois et encore branchés, sinon sur le premier écran qui ne montre pas la fenêtre de contrôle ; si une sortie est ouverte, `F` les ferme toutes.
- Préférences : `output_screens`, liste de noms d'écrans. L'ancien champ `output_screen` est repris au chargement, puis n'est plus écrit.
- Un seul processus `caffeinate` garde les écrans allumés, transmis à une sortie restante quand celle qui le porte se ferme.

## Conséquences

- Chaque sortie est présentée avec synchronisation verticale. Des écrans de fréquences différentes (60 et 50 Hz) peuvent faire attendre le thread de rendu sur le plus lent ; à mesurer avec le matériel de l'utilisateur.
- La phase 4 remplacera « image entière sur chaque écran » par des écrans virtuels et des slices ; la liste de fenêtres restera.
