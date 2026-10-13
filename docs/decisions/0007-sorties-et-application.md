# 0007 – Sorties présentées par le thread de rendu

- Date : 2026-10-14
- Statut : Acceptée

## Contexte

La phase 1 demande une sortie plein écran sur un second écran, sans saccade. L'interface (egui, ticket suivant) et les fenêtres vivent sur le thread principal, imposé par macOS pour les événements de fenêtres. Il faut décider qui présente l'image dans les fenêtres de sortie.

## Options

1. **Le thread principal présente les sorties**, comme le ferait une application egui classique (eframe et ses « viewports »). Simple, mais une interface occupée (boîte de dialogue, liste de fichiers longue) ferait saccader la sortie, ce que la règle d'or interdit.
2. **Le thread de rendu présente les sorties** juste après chaque image. Le thread principal crée les fenêtres et ne transmet que leur taille et leur visibilité. Il faut une boucle d'événements à nous (winit) plutôt qu'eframe.

## Décision

Option 2 :

- le moteur expose un trait `Output` ; `OutputSurface` (crate `prism-output`) l'implémente pour une fenêtre ; le thread principal l'ajoute ou la retire par commande ;
- présentation avec synchronisation verticale (`AutoVsync`) pour éviter les déchirures ; une fenêtre cachée ou une surface pas prête fait sauter l'image au lieu de bloquer le moteur ;
- format de surface non sRGB : l'image programme est déjà en espace d'affichage (ADR 0004) ; valeurs écrêtées à l'affichage, image ajustée avec bandes noires ;
- sortie plein écran en mode « sans bordure » de winit ; sur macOS elle occupe son propre espace sur l'écran de sortie, la fenêtre de contrôle restant sur l'écran principal ;
- l'application (`prism`, crate `prism-app`) arrête le moteur avant de fermer les fenêtres, pour que le thread de rendu ne libère jamais la dernière référence à une fenêtre ;
- `cargo-deny` ne vérifie que les plateformes livrées (macOS, Windows) : une dépendance propre à Linux signalée non maintenue (`ttf-parser`, via les décorations Wayland de winit) n'entre pas dans l'application macOS.

## Conséquences

- L'interface egui sera intégrée à la main (egui-winit et egui-wgpu) dans la fenêtre de contrôle, et non via eframe.
- Le rythme du moteur s'aligne sur l'écran quand la file de présentation est pleine ; un pilotage par la synchronisation de l'écran (CVDisplayLink) pourra remplacer le minuteur.
- À faire : empêcher la mise en veille de l'écran pendant un spectacle.
